use crate::parser::dag::*;
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::Path;

/// Parser for GitLab CI `.gitlab-ci.yml` files.
pub struct GitLabCIParser;

/// Reserved top-level keywords in GitLab CI that are NOT job definitions.
const RESERVED_KEYWORDS: &[&str] = &[
    "image", "services", "stages", "before_script", "after_script",
    "variables", "cache", "default", "include", "workflow", "pages",
];

impl GitLabCIParser {
    /// Parse a GitLab CI file into a Pipeline DAG.
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read GitLab CI file: {}", path.display()))?;
        Self::parse(&content, path.to_string_lossy().to_string())
    }

    /// Parse GitLab CI YAML content into a Pipeline DAG.
    pub fn parse(content: &str, source_file: String) -> Result<PipelineDag> {
        let yaml: Value = serde_yaml::from_str(content)
            .context("Failed to parse YAML")?;

        let mapping = yaml.as_mapping()
            .context("GitLab CI config must be a YAML mapping")?;

        let mut dag = PipelineDag::new(
            source_file.clone(),
            source_file,
            "gitlab-ci".to_string(),
        );

        // Parse stages (defines execution order)
        let stages = Self::parse_stages(&yaml);

        // Parse global variables
        if let Some(vars) = yaml.get("variables") {
            dag.env = Self::parse_variables(vars);
        }

        // Parse global cache config
        let global_cache = yaml.get("cache");

        // Parse global default settings
        let default_image = yaml.get("default")
            .and_then(|d| d.get("image"))
            .or_else(|| yaml.get("image"))
            .and_then(Self::parse_image);

        // Collect all jobs (anything not a reserved keyword and not starting with '.')
        let mut jobs_by_stage: HashMap<String, Vec<String>> = HashMap::new();

        for (key, value) in mapping {
            let key_str = match key.as_str() {
                Some(k) => k,
                None => continue,
            };

            // Skip reserved keywords and hidden jobs (starting with .)
            if RESERVED_KEYWORDS.contains(&key_str) || key_str.starts_with('.') {
                continue;
            }

            // Must be a mapping to be a job
            if !value.is_mapping() {
                continue;
            }

            let job = Self::parse_job(key_str, value, &stages, &default_image, global_cache)?;
            let stage = job.env.get("__stage").cloned().unwrap_or_else(|| "test".to_string());

            jobs_by_stage.entry(stage).or_default().push(job.id.clone());
            dag.add_job(job);
        }

        // Build dependency edges
        // In GitLab, jobs in the same stage run in parallel.
        // Jobs in later stages depend on all jobs in the previous stage (unless `needs:` is specified).
        for (key, value) in mapping {
            let key_str = match key.as_str() {
                Some(k) => k,
                None => continue,
            };
            if RESERVED_KEYWORDS.contains(&key_str) || key_str.starts_with('.') {
                continue;
            }
            if !value.is_mapping() {
                continue;
            }

            // If job has explicit `needs:`, use those
            if let Some(needs) = value.get("needs") {
                let deps = Self::parse_needs(needs);
                for dep in deps {
                    if dag.get_job(&dep).is_some() {
                        let _ = dag.add_dependency(&dep, key_str);
                    }
                }
            } else {
                // Otherwise, depend on all jobs from the previous stage
                let job_stage = value.get("stage")
                    .and_then(|v| v.as_str())
                    .unwrap_or("test")
                    .to_string();

                if let Some(stage_idx) = stages.iter().position(|s| s == &job_stage) {
                    if stage_idx > 0 {
                        let prev_stage = &stages[stage_idx - 1];
                        if let Some(prev_jobs) = jobs_by_stage.get(prev_stage) {
                            for prev_job in prev_jobs {
                                let _ = dag.add_dependency(prev_job, key_str);
                            }
                        }
                    }
                }
            }
        }

        // Parse triggers from workflow:rules or just mark as generic
        dag.triggers = Self::parse_triggers(&yaml);

        Ok(dag)
    }

    fn parse_stages(yaml: &Value) -> Vec<String> {
        yaml.get("stages")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(|| vec![
                "build".to_string(),
                "test".to_string(),
                "deploy".to_string(),
            ])
    }

    fn parse_job(
        job_id: &str,
        config: &Value,
        _stages: &[String],
        default_image: &Option<String>,
        global_cache: Option<&Value>,
    ) -> Result<JobNode> {
        let name = job_id.to_string();
        let mut job = JobNode::new(job_id.to_string(), name);

        // Stage
        let stage = config.get("stage")
            .and_then(|v| v.as_str())
            .unwrap_or("test")
            .to_string();
        job.env.insert("__stage".to_string(), stage);

        // Image (runner)
        let image = config.get("image")
            .and_then(Self::parse_image)
            .or_else(|| default_image.clone())
            .unwrap_or_else(|| "docker".to_string());
        job.runs_on = image;

        // Variables
        if let Some(vars) = config.get("variables") {
            for (k, v) in Self::parse_variables(vars) {
                job.env.insert(k, v);
            }
        }

        // Rules / only / except → condition
        if let Some(rules) = config.get("rules") {
            if let Some(seq) = rules.as_sequence() {
                let rule_strs: Vec<String> = seq.iter()
                    .filter_map(|r| r.get("if").and_then(|v| v.as_str()).map(String::from))
                    .collect();
                if !rule_strs.is_empty() {
                    job.condition = Some(rule_strs.join(" || "));
                }
            }
        }

        // Needs (explicit dependencies)
        if let Some(needs) = config.get("needs") {
            job.needs = Self::parse_needs(needs);
        }

        // Script steps
        let mut steps = Vec::new();

        // before_script
        if let Some(before) = config.get("before_script").and_then(|v| v.as_sequence()) {
            for (i, cmd) in before.iter().enumerate() {
                if let Some(cmd_str) = cmd.as_str() {
                    steps.push(StepInfo {
                        name: format!("before_script[{}]", i),
                        uses: None,
                        run: Some(cmd_str.to_string()),
                        estimated_duration_secs: Some(Self::estimate_cmd_duration(cmd_str)),
                    });
                }
            }
        }

        // Main script
        if let Some(script) = config.get("script").and_then(|v| v.as_sequence()) {
            for (i, cmd) in script.iter().enumerate() {
                if let Some(cmd_str) = cmd.as_str() {
                    steps.push(StepInfo {
                        name: format!("script[{}]", i),
                        uses: None,
                        run: Some(cmd_str.to_string()),
                        estimated_duration_secs: Some(Self::estimate_cmd_duration(cmd_str)),
                    });
                }
            }
        }

        // after_script
        if let Some(after) = config.get("after_script").and_then(|v| v.as_sequence()) {
            for (i, cmd) in after.iter().enumerate() {
                if let Some(cmd_str) = cmd.as_str() {
                    steps.push(StepInfo {
                        name: format!("after_script[{}]", i),
                        uses: None,
                        run: Some(cmd_str.to_string()),
                        estimated_duration_secs: Some(Self::estimate_cmd_duration(cmd_str)),
                    });
                }
            }
        }

        job.steps = steps;

        // Cache detection
        let has_cache = config.get("cache").is_some() || global_cache.is_some();
        if has_cache {
            job.caches.push(CacheConfig {
                path: "detected".to_string(),
                key_pattern: "detected".to_string(),
                restore_keys: Vec::new(),
            });
        }

        // Artifacts
        if let Some(artifacts) = config.get("artifacts") {
            if artifacts.get("paths").is_some() {
                job.caches.push(CacheConfig {
                    path: "artifacts".to_string(),
                    key_pattern: "artifacts".to_string(),
                    restore_keys: Vec::new(),
                });
            }
        }

        // Parallel keyword (GitLab's built-in job parallelism)
        if let Some(parallel) = config.get("parallel").and_then(|v| v.as_u64()) {
            let mut vars = HashMap::new();
            let shards: Vec<String> = (1..=parallel).map(|i| i.to_string()).collect();
            let count = shards.len();
            vars.insert("CI_NODE_INDEX".to_string(), shards);
            job.matrix = Some(MatrixStrategy {
                variables: vars,
                total_combinations: count,
            });
        }

        // Estimate total duration
        job.estimated_duration_secs = job.steps.iter()
            .filter_map(|s| s.estimated_duration_secs)
            .sum();

        Ok(job)
    }

    fn parse_needs(needs: &Value) -> Vec<String> {
        match needs {
            Value::Sequence(seq) => {
                seq.iter().filter_map(|v| {
                    match v {
                        Value::String(s) => Some(s.clone()),
                        Value::Mapping(m) => {
                            m.get(Value::String("job".to_string()))
                                .and_then(|v| v.as_str())
                                .map(String::from)
                        }
                        _ => None,
                    }
                }).collect()
            }
            _ => Vec::new(),
        }
    }

    fn parse_variables(vars: &Value) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Some(mapping) = vars.as_mapping() {
            for (k, v) in mapping {
                if let Some(key) = k.as_str() {
                    let val = match v {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Mapping(m) => {
                            // GitLab expanded variable syntax: { value: "x", description: "..." }
                            m.get(Value::String("value".to_string()))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string()
                        }
                        _ => String::new(),
                    };
                    map.insert(key.to_string(), val);
                }
            }
        }
        map
    }

    fn parse_image(v: &Value) -> Option<String> {
        match v {
            Value::String(s) => Some(s.clone()),
            Value::Mapping(m) => {
                m.get(Value::String("name".to_string()))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            }
            _ => None,
        }
    }

    fn parse_triggers(yaml: &Value) -> Vec<WorkflowTrigger> {
        let mut triggers = Vec::new();

        if let Some(workflow) = yaml.get("workflow") {
            if let Some(rules) = workflow.get("rules").and_then(|v| v.as_sequence()) {
                for rule in rules {
                    let event = rule.get("if")
                        .and_then(|v| v.as_str())
                        .unwrap_or("push")
                        .to_string();
                    triggers.push(WorkflowTrigger {
                        event,
                        branches: None,
                        paths: None,
                        paths_ignore: None,
                    });
                }
            }
        }

        if triggers.is_empty() {
            triggers.push(WorkflowTrigger {
                event: "push".to_string(),
                branches: None,
                paths: None,
                paths_ignore: None,
            });
        }

        triggers
    }

    #[allow(clippy::if_same_then_else)]
    fn estimate_cmd_duration(cmd: &str) -> f64 {
        let cmd_lower = cmd.to_lowercase();
        if cmd_lower.contains("npm ci") || cmd_lower.contains("npm install") || cmd_lower.contains("yarn install") {
            180.0
        } else if cmd_lower.contains("pip install") {
            120.0
        } else if cmd_lower.contains("cargo build") {
            300.0
        } else if cmd_lower.contains("cargo test") {
            300.0
        } else if cmd_lower.contains("npm run build") || cmd_lower.contains("yarn build") {
            240.0
        } else if cmd_lower.contains("npm test") || cmd_lower.contains("pytest") || cmd_lower.contains("jest") || cmd_lower.contains("rspec") {
            300.0
        } else if cmd_lower.contains("eslint") || cmd_lower.contains("rubocop") || cmd_lower.contains("flake8") {
            60.0
        } else if cmd_lower.contains("docker build") {
            300.0
        } else if cmd_lower.contains("deploy") || cmd_lower.contains("kubectl") || cmd_lower.contains("helm") {
            120.0
        } else if cmd_lower.contains("bundle install") {
            150.0
        } else if cmd_lower.contains("composer install") {
            90.0
        } else if cmd_lower.contains("apt-get") || cmd_lower.contains("apk add") {
            45.0
        } else {
            30.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_gitlab_ci() {
        let yaml = r#"
stages:
  - build
  - test
  - deploy

build:
  stage: build
  script:
    - npm ci
    - npm run build

test:
  stage: test
  script:
    - npm test

deploy:
  stage: deploy
  script:
    - ./deploy.sh
  only:
    - main
"#;
        let dag = GitLabCIParser::parse(yaml, ".gitlab-ci.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 3);
        assert_eq!(dag.provider, "gitlab-ci");
        assert!(dag.get_job("build").is_some());
        assert!(dag.get_job("test").is_some());
        assert!(dag.get_job("deploy").is_some());
    }

    #[test]
    fn test_gitlab_stage_dependencies() {
        let yaml = r#"
stages:
  - build
  - test

compile:
  stage: build
  script:
    - make build

lint:
  stage: build
  script:
    - make lint

unit_test:
  stage: test
  script:
    - make test
"#;
        let dag = GitLabCIParser::parse(yaml, ".gitlab-ci.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 3);
        // unit_test (stage: test) should depend on both compile and lint (stage: build)
        let _test_job = dag.get_job("unit_test").unwrap();
        // Root jobs are build-stage jobs
        assert_eq!(dag.root_jobs().len(), 2);
    }

    #[test]
    fn test_gitlab_explicit_needs() {
        let yaml = r#"
stages:
  - build
  - test
  - deploy

build_app:
  stage: build
  script:
    - npm run build

build_docs:
  stage: build
  script:
    - npm run docs

test_app:
  stage: test
  needs: [build_app]
  script:
    - npm test

deploy:
  stage: deploy
  needs:
    - job: build_app
    - job: test_app
  script:
    - ./deploy.sh
"#;
        let dag = GitLabCIParser::parse(yaml, ".gitlab-ci.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 4);
        // test_app only needs build_app, not build_docs
        let test_job = dag.get_job("test_app").unwrap();
        assert_eq!(test_job.needs, vec!["build_app"]);
    }

    #[test]
    fn test_gitlab_parallel_keyword() {
        let yaml = r#"
test:
  stage: test
  parallel: 4
  script:
    - npm test -- --shard=$CI_NODE_INDEX/$CI_NODE_TOTAL
"#;
        let dag = GitLabCIParser::parse(yaml, ".gitlab-ci.yml".to_string()).unwrap();
        let test_job = dag.get_job("test").unwrap();
        assert!(test_job.matrix.is_some());
        assert_eq!(test_job.matrix.as_ref().unwrap().total_combinations, 4);
    }

    #[test]
    fn test_gitlab_skips_hidden_jobs() {
        let yaml = r#"
.template:
  image: node:20
  before_script:
    - npm ci

build:
  stage: build
  script:
    - npm run build
"#;
        let dag = GitLabCIParser::parse(yaml, ".gitlab-ci.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 1);
        assert!(dag.get_job("build").is_some());
        assert!(dag.get_job(".template").is_none());
    }
}
