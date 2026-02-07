use crate::parser::dag::*;
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::Path;

/// Parser for GitHub Actions workflow YAML files.
pub struct GitHubActionsParser;

impl GitHubActionsParser {
    /// Parse a GitHub Actions workflow file into a Pipeline DAG.
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read workflow file: {}", path.display()))?;
        Self::parse(&content, path.to_string_lossy().to_string())
    }

    /// Parse GitHub Actions YAML content into a Pipeline DAG.
    pub fn parse(content: &str, source_file: String) -> Result<PipelineDag> {
        let yaml: Value = serde_yaml::from_str(content)
            .context("Failed to parse YAML")?;

        let name = yaml.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed Workflow")
            .to_string();

        let mut dag = PipelineDag::new(name, source_file, "github-actions".to_string());

        // Parse triggers
        dag.triggers = Self::parse_triggers(&yaml);

        // Parse top-level env
        if let Some(env) = yaml.get("env") {
            dag.env = Self::parse_env(env);
        }

        // Parse jobs
        let jobs = yaml.get("jobs")
            .and_then(|v| v.as_mapping())
            .context("No 'jobs' section found in workflow")?;

        // First pass: create all job nodes
        for (job_id, job_config) in jobs {
            let job_id = job_id.as_str().unwrap_or("unknown").to_string();
            let job = Self::parse_job(&job_id, job_config)?;
            dag.add_job(job);
        }

        // Second pass: add dependency edges (needs: [job_a, job_b])
        // The edge direction is from dependency TO dependent (dependency must finish first)
        for (job_id, job_config) in jobs {
            let job_id = job_id.as_str().unwrap_or("unknown").to_string();
            if let Some(needs) = job_config.get("needs") {
                let deps = Self::parse_needs(needs);
                for dep in deps {
                    dag.add_dependency(&dep, &job_id)
                        .with_context(|| format!("Failed to add dependency {dep} -> {job_id}"))?;
                }
            }
        }

        Ok(dag)
    }

    fn parse_triggers(yaml: &Value) -> Vec<WorkflowTrigger> {
        let mut triggers = Vec::new();

        let on = match yaml.get("on") {
            Some(v) => v,
            None => return triggers,
        };

        match on {
            Value::String(event) => {
                triggers.push(WorkflowTrigger {
                    event: event.clone(),
                    branches: None,
                    paths: None,
                    paths_ignore: None,
                });
            }
            Value::Sequence(events) => {
                for event in events {
                    if let Some(e) = event.as_str() {
                        triggers.push(WorkflowTrigger {
                            event: e.to_string(),
                            branches: None,
                            paths: None,
                            paths_ignore: None,
                        });
                    }
                }
            }
            Value::Mapping(map) => {
                for (event, config) in map {
                    let event_name = match event.as_str() {
                        Some(e) => e.to_string(),
                        None => continue,
                    };
                    let branches = config.get("branches")
                        .and_then(|v| v.as_sequence())
                        .map(|seq| seq.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                    let paths = config.get("paths")
                        .and_then(|v| v.as_sequence())
                        .map(|seq| seq.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                    let paths_ignore = config.get("paths-ignore")
                        .and_then(|v| v.as_sequence())
                        .map(|seq| seq.iter().filter_map(|v| v.as_str().map(String::from)).collect());

                    triggers.push(WorkflowTrigger {
                        event: event_name,
                        branches,
                        paths,
                        paths_ignore,
                    });
                }
            }
            _ => {}
        }

        triggers
    }

    fn parse_job(job_id: &str, config: &Value) -> Result<JobNode> {
        let name = config.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(job_id)
            .to_string();

        let mut job = JobNode::new(job_id.to_string(), name);

        // runs-on
        if let Some(runs_on) = config.get("runs-on").and_then(|v| v.as_str()) {
            job.runs_on = runs_on.to_string();
        }

        // needs
        if let Some(needs) = config.get("needs") {
            job.needs = Self::parse_needs(needs);
        }

        // condition
        if let Some(cond) = config.get("if").and_then(|v| v.as_str()) {
            job.condition = Some(cond.to_string());
        }

        // env
        if let Some(env) = config.get("env") {
            job.env = Self::parse_env(env);
        }

        // matrix strategy
        if let Some(strategy) = config.get("strategy") {
            job.matrix = Self::parse_matrix(strategy);
        }

        // steps
        if let Some(steps) = config.get("steps").and_then(|v| v.as_sequence()) {
            for step in steps {
                let step_info = Self::parse_step(step);
                job.steps.push(step_info);
            }
        }

        // Detect caches in steps
        job.caches = Self::detect_caches(&job.steps);

        // Estimate duration based on step analysis
        job.estimated_duration_secs = Self::estimate_job_duration(&job);

        Ok(job)
    }

    fn parse_needs(needs: &Value) -> Vec<String> {
        match needs {
            Value::String(s) => vec![s.clone()],
            Value::Sequence(seq) => {
                seq.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn parse_step(step: &Value) -> StepInfo {
        let name = step.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed step")
            .to_string();

        let uses = step.get("uses")
            .and_then(|v| v.as_str())
            .map(String::from);

        let run = step.get("run")
            .and_then(|v| v.as_str())
            .map(String::from);

        let estimated_duration = Self::estimate_step_duration(&uses, &run);

        StepInfo {
            name,
            uses,
            run,
            estimated_duration_secs: Some(estimated_duration),
        }
    }

    fn parse_env(env: &Value) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Some(mapping) = env.as_mapping() {
            for (k, v) in mapping {
                if let (Some(key), Some(val)) = (k.as_str(), v.as_str()) {
                    map.insert(key.to_string(), val.to_string());
                }
            }
        }
        map
    }

    fn parse_matrix(strategy: &Value) -> Option<MatrixStrategy> {
        let matrix = strategy.get("matrix")?;
        let mapping = matrix.as_mapping()?;

        let mut variables = HashMap::new();
        let mut total = 1usize;

        for (key, value) in mapping {
            let key = key.as_str()?;
            // Skip special keys like 'include' and 'exclude'
            if key == "include" || key == "exclude" {
                continue;
            }
            if let Some(seq) = value.as_sequence() {
                let values: Vec<String> = seq.iter()
                    .filter_map(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        Value::Number(n) => Some(n.to_string()),
                        Value::Bool(b) => Some(b.to_string()),
                        _ => None,
                    })
                    .collect();
                total *= values.len();
                variables.insert(key.to_string(), values);
            }
        }

        Some(MatrixStrategy {
            variables,
            total_combinations: total,
        })
    }

    fn detect_caches(steps: &[StepInfo]) -> Vec<CacheConfig> {
        let mut caches = Vec::new();
        for step in steps {
            if let Some(uses) = &step.uses {
                if uses.starts_with("actions/cache") {
                    caches.push(CacheConfig {
                        path: "detected".to_string(),
                        key_pattern: "detected".to_string(),
                        restore_keys: Vec::new(),
                    });
                }
            }
        }
        caches
    }

    /// Estimate step duration in seconds based on heuristics.
    fn estimate_step_duration(uses: &Option<String>, run: &Option<String>) -> f64 {
        if let Some(uses) = uses {
            if uses.starts_with("actions/checkout") {
                return 12.0;
            }
            if uses.starts_with("actions/setup-node")
                || uses.starts_with("actions/setup-python")
                || uses.starts_with("actions/setup-java")
                || uses.starts_with("actions/setup-go")
            {
                return 15.0;
            }
            if uses.starts_with("actions/cache") {
                return 10.0;
            }
            if uses.starts_with("docker/build-push-action") {
                return 300.0;
            }
            if uses.starts_with("actions/upload-artifact") || uses.starts_with("actions/download-artifact") {
                return 15.0;
            }
            return 20.0; // Generic action
        }

        if let Some(run) = run {
            let cmd = run.to_lowercase();
            if cmd.contains("npm install") || cmd.contains("npm ci") || cmd.contains("yarn install") || cmd.contains("pnpm install") {
                return 180.0;
            }
            if cmd.contains("pip install") {
                return 120.0;
            }
            if cmd.contains("cargo build") {
                return 300.0;
            }
            if cmd.contains("npm run build") || cmd.contains("yarn build") || cmd.contains("pnpm build") {
                return 240.0;
            }
            if cmd.contains("npm test") || cmd.contains("pytest") || cmd.contains("cargo test") || cmd.contains("jest") {
                return 300.0;
            }
            if cmd.contains("npm run lint") || cmd.contains("eslint") || cmd.contains("clippy") {
                return 60.0;
            }
            if cmd.contains("docker build") {
                return 300.0;
            }
            if cmd.contains("docker push") {
                return 60.0;
            }
            if cmd.contains("deploy") || cmd.contains("kubectl") || cmd.contains("terraform") {
                return 120.0;
            }
            return 30.0; // Generic command
        }

        10.0 // Unknown step
    }

    fn estimate_job_duration(job: &JobNode) -> f64 {
        job.steps.iter()
            .filter_map(|s| s.estimated_duration_secs)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_workflow() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build
        run: npm run build
  test:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Test
        run: npm test
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 2);
        assert_eq!(dag.name, "CI");
        assert!(dag.get_job("build").is_some());
        assert!(dag.get_job("test").is_some());
        assert_eq!(dag.get_job("test").unwrap().needs, vec!["build"]);
    }

    #[test]
    fn test_parse_parallel_jobs() {
        let yaml = r#"
name: CI
on: [push, pull_request]
jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm run lint
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm test
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm run build
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 3);
        assert_eq!(dag.root_jobs().len(), 3);
        assert_eq!(dag.max_parallelism(), 3);
    }

    #[test]
    fn test_parse_matrix_strategy() {
        let yaml = r#"
name: Matrix CI
on: push
jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        node: [18, 20, 22]
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - run: npm test
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let test_job = dag.get_job("test").unwrap();
        let matrix = test_job.matrix.as_ref().unwrap();
        assert_eq!(matrix.total_combinations, 6);
    }
}
