use crate::parser::dag::*;
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Parser for Azure Pipelines YAML (`azure-pipelines.yml`).
///
/// Supported constructs:
/// - stages -> jobs -> steps
/// - stage/job `dependsOn`
/// - template references (`template:` at stage/job/step level)
pub struct AzurePipelinesParser;

impl AzurePipelinesParser {
    /// Parse an Azure Pipelines file into a Pipeline DAG.
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read Azure Pipelines file: {}", path.display()))?;
        Self::parse(&content, path.to_string_lossy().to_string())
    }

    /// Parse Azure Pipelines YAML content into a Pipeline DAG.
    pub fn parse(content: &str, source_file: String) -> Result<PipelineDag> {
        let yaml: Value = serde_yaml::from_str(content).context("Failed to parse YAML")?;

        let name = yaml
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Azure Pipeline")
            .to_string();

        let mut dag = PipelineDag::new(name, source_file, "azure-pipelines".to_string());
        dag.triggers = Self::parse_triggers(&yaml);
        dag.env = Self::parse_variables(yaml.get("variables"));

        let mut stage_to_jobs: HashMap<String, Vec<String>> = HashMap::new();
        let mut stage_dependencies: HashMap<String, Vec<String>> = HashMap::new();
        let mut job_aliases: HashMap<String, String> = HashMap::new();
        let mut raw_job_needs: HashMap<String, Vec<String>> = HashMap::new();
        let mut synthetic_counter = 0usize;

        // Top-level template extends.
        if let Some(template) = yaml
            .get("extends")
            .and_then(|v| v.get("template"))
            .and_then(|v| v.as_str())
        {
            let id = "pipeline-template".to_string();
            let mut job = JobNode::new(id.clone(), "Pipeline Template".to_string());
            job.steps.push(StepInfo {
                name: "template".to_string(),
                uses: Some(template.to_string()),
                run: None,
                estimated_duration_secs: Some(5.0),
            });
            job.estimated_duration_secs = 5.0;
            dag.add_job(job);
            job_aliases.insert("pipeline-template".to_string(), id);
        }

        if let Some(stages) = yaml.get("stages").and_then(|v| v.as_sequence()) {
            for (stage_idx, stage) in stages.iter().enumerate() {
                let parsed_stage = Self::parse_stage(
                    stage,
                    stage_idx,
                    &mut synthetic_counter,
                    &mut dag,
                    &mut job_aliases,
                    &mut raw_job_needs,
                )?;
                stage_dependencies.insert(parsed_stage.name.clone(), parsed_stage.depends_on);
                stage_to_jobs.insert(parsed_stage.name, parsed_stage.job_ids);
            }
        } else if let Some(jobs) = yaml.get("jobs").and_then(|v| v.as_sequence()) {
            // Azure also supports top-level jobs without stages.
            let stage_name = "default".to_string();
            let mut job_ids = Vec::new();
            for (job_idx, job_value) in jobs.iter().enumerate() {
                let parsed_job = Self::parse_job(
                    &stage_name,
                    job_value,
                    job_idx,
                    &mut synthetic_counter,
                    &mut dag,
                    &mut job_aliases,
                )?;
                raw_job_needs.insert(parsed_job.id.clone(), parsed_job.depends_on_raw.clone());
                job_ids.push(parsed_job.id);
            }
            stage_to_jobs.insert(stage_name, job_ids);
        } else if let Some(template) = yaml.get("template").and_then(|v| v.as_str()) {
            // Template-only pipelines still yield a DAG node.
            let id = "top-level-template".to_string();
            let mut job = JobNode::new(id.clone(), "Top-level Template".to_string());
            job.steps.push(StepInfo {
                name: "template".to_string(),
                uses: Some(template.to_string()),
                run: None,
                estimated_duration_secs: Some(5.0),
            });
            job.estimated_duration_secs = 5.0;
            dag.add_job(job);
        }

        // Resolve job-level dependencies (dependsOn may reference job names or stage names).
        let mut edge_dedup: HashSet<(String, String)> = HashSet::new();
        for (job_id, raw_deps) in raw_job_needs {
            let mut resolved = Vec::new();
            for raw_dep in raw_deps {
                let targets = Self::resolve_dependency(&raw_dep, &job_aliases, &stage_to_jobs);
                for target in targets {
                    if target == job_id {
                        continue;
                    }
                    if edge_dedup.insert((target.clone(), job_id.clone())) {
                        let _ = dag.add_dependency(&target, &job_id);
                    }
                    if !resolved.contains(&target) {
                        resolved.push(target);
                    }
                }
            }
            if let Some(idx) = dag.node_map.get(&job_id).copied() {
                dag.graph[idx].needs = resolved;
            }
        }

        // Resolve stage-level dependencies:
        // every job in stage B depends on every job in each stage listed in B.dependsOn.
        for (stage_name, stage_deps) in stage_dependencies {
            let Some(current_jobs) = stage_to_jobs.get(&stage_name) else {
                continue;
            };
            for dep_stage in stage_deps {
                let Some(dep_jobs) = stage_to_jobs.get(&dep_stage) else {
                    continue;
                };
                for dep_job in dep_jobs {
                    for current_job in current_jobs {
                        if dep_job == current_job {
                            continue;
                        }
                        if edge_dedup.insert((dep_job.clone(), current_job.clone())) {
                            let _ = dag.add_dependency(dep_job, current_job);
                        }
                        if let Some(idx) = dag.node_map.get(current_job).copied() {
                            if !dag.graph[idx].needs.contains(dep_job) {
                                dag.graph[idx].needs.push(dep_job.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(dag)
    }

    fn parse_stage(
        stage_value: &Value,
        stage_idx: usize,
        synthetic_counter: &mut usize,
        dag: &mut PipelineDag,
        job_aliases: &mut HashMap<String, String>,
        raw_job_needs: &mut HashMap<String, Vec<String>>,
    ) -> Result<ParsedStage> {
        if let Some(template_path) = stage_value.get("template").and_then(|v| v.as_str()) {
            let stage_name = format!("template-stage-{}", stage_idx + 1);
            let job_id = format!("{}-template", sanitize_id(&stage_name));
            let mut job = JobNode::new(job_id.clone(), format!("Stage Template {}", stage_idx + 1));
            job.steps.push(StepInfo {
                name: "template".to_string(),
                uses: Some(template_path.to_string()),
                run: None,
                estimated_duration_secs: Some(5.0),
            });
            job.estimated_duration_secs = 5.0;
            dag.add_job(job);
            job_aliases.insert(stage_name.clone(), job_id.clone());
            return Ok(ParsedStage {
                name: stage_name,
                depends_on: Vec::new(),
                job_ids: vec![job_id],
            });
        }

        let stage_name = stage_value
            .get("stage")
            .and_then(|v| v.as_str())
            .or_else(|| stage_value.get("name").and_then(|v| v.as_str()))
            .unwrap_or("stage")
            .to_string();
        let stage_depends = parse_depends_on(stage_value.get("dependsOn"));
        let mut job_ids = Vec::new();

        if let Some(jobs) = stage_value.get("jobs").and_then(|v| v.as_sequence()) {
            for (job_idx, job_value) in jobs.iter().enumerate() {
                let parsed = Self::parse_job(
                    &stage_name,
                    job_value,
                    job_idx,
                    synthetic_counter,
                    dag,
                    job_aliases,
                )?;
                raw_job_needs.insert(parsed.id.clone(), parsed.depends_on_raw.clone());
                job_ids.push(parsed.id);
            }
        }

        if job_ids.is_empty() {
            // Stage without explicit jobs still gets a placeholder node.
            *synthetic_counter += 1;
            let id = format!(
                "{}-stage-node-{}",
                sanitize_id(&stage_name),
                *synthetic_counter
            );
            let mut job = JobNode::new(id.clone(), format!("Stage {}", stage_name));
            job.steps.push(StepInfo {
                name: "stage".to_string(),
                uses: None,
                run: Some(format!("stage: {}", stage_name)),
                estimated_duration_secs: Some(30.0),
            });
            job.estimated_duration_secs = 30.0;
            dag.add_job(job);
            job_ids.push(id.clone());
            job_aliases.insert(stage_name.clone(), id);
        } else {
            // Stage name can be referenced by dependsOn.
            job_aliases.insert(stage_name.clone(), job_ids[0].clone());
        }

        Ok(ParsedStage {
            name: stage_name,
            depends_on: stage_depends,
            job_ids,
        })
    }

    fn parse_job(
        stage_name: &str,
        job_value: &Value,
        job_idx: usize,
        synthetic_counter: &mut usize,
        dag: &mut PipelineDag,
        job_aliases: &mut HashMap<String, String>,
    ) -> Result<ParsedJob> {
        if let Some(template_path) = job_value.get("template").and_then(|v| v.as_str()) {
            *synthetic_counter += 1;
            let id = format!(
                "{}-template-{}",
                sanitize_id(stage_name),
                *synthetic_counter
            );
            let mut job = JobNode::new(id.clone(), format!("Template {}", template_path));
            job.steps.push(StepInfo {
                name: "template".to_string(),
                uses: Some(template_path.to_string()),
                run: None,
                estimated_duration_secs: Some(5.0),
            });
            job.estimated_duration_secs = 5.0;
            dag.add_job(job);
            job_aliases.insert(
                format!("{}.template{}", stage_name, job_idx + 1),
                id.clone(),
            );
            return Ok(ParsedJob {
                id,
                depends_on_raw: Vec::new(),
            });
        }

        let raw_name = job_value
            .get("job")
            .and_then(|v| v.as_str())
            .or_else(|| job_value.get("deployment").and_then(|v| v.as_str()))
            .or_else(|| job_value.get("name").and_then(|v| v.as_str()))
            .or_else(|| job_value.get("displayName").and_then(|v| v.as_str()))
            .map(String::from)
            .unwrap_or_else(|| format!("job-{}", job_idx + 1));

        let id = format!("{}-{}", sanitize_id(stage_name), sanitize_id(&raw_name));
        let mut job = JobNode::new(id.clone(), raw_name.clone());

        job.runs_on =
            parse_pool_name(job_value.get("pool")).unwrap_or_else(|| "azure:vm".to_string());
        job.condition = job_value
            .get("condition")
            .and_then(|v| v.as_str())
            .map(String::from);

        let steps = extract_steps(job_value);
        job.caches = detect_caches(&steps);
        job.estimated_duration_secs = steps
            .iter()
            .filter_map(|s| s.estimated_duration_secs)
            .sum::<f64>()
            .max(10.0);
        job.steps = steps;

        dag.add_job(job);

        job_aliases.insert(raw_name.clone(), id.clone());
        job_aliases.insert(format!("{}.{}", stage_name, raw_name), id.clone());

        Ok(ParsedJob {
            id,
            depends_on_raw: parse_depends_on(job_value.get("dependsOn")),
        })
    }

    fn resolve_dependency(
        raw_dep: &str,
        job_aliases: &HashMap<String, String>,
        stage_to_jobs: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        if let Some(job_id) = job_aliases.get(raw_dep) {
            return vec![job_id.clone()];
        }
        if let Some(stage_jobs) = stage_to_jobs.get(raw_dep) {
            return stage_jobs.clone();
        }
        Vec::new()
    }

    fn parse_triggers(yaml: &Value) -> Vec<WorkflowTrigger> {
        let mut triggers = Vec::new();

        for key in ["trigger", "pr"] {
            let Some(trigger_val) = yaml.get(key) else {
                continue;
            };

            match trigger_val {
                Value::String(s) => {
                    if s != "none" {
                        triggers.push(WorkflowTrigger {
                            event: key.to_string(),
                            branches: Some(vec![s.clone()]),
                            paths: None,
                            paths_ignore: None,
                        });
                    }
                }
                Value::Sequence(seq) => {
                    let branches: Vec<String> = seq
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    triggers.push(WorkflowTrigger {
                        event: key.to_string(),
                        branches: if branches.is_empty() {
                            None
                        } else {
                            Some(branches)
                        },
                        paths: None,
                        paths_ignore: None,
                    });
                }
                Value::Mapping(map) => {
                    let branches = map
                        .get(Value::String("branches".to_string()))
                        .and_then(|v| {
                            v.get("include")
                                .or_else(|| v.as_sequence().map(|_| v))
                                .and_then(|x| x.as_sequence())
                        })
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        });

                    let paths = map
                        .get(Value::String("paths".to_string()))
                        .and_then(|v| v.get("include").or_else(|| v.as_sequence().map(|_| v)))
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        });

                    let paths_ignore = map
                        .get(Value::String("paths".to_string()))
                        .and_then(|v| v.get("exclude"))
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        });

                    triggers.push(WorkflowTrigger {
                        event: key.to_string(),
                        branches,
                        paths,
                        paths_ignore,
                    });
                }
                _ => {}
            }
        }

        triggers
    }

    fn parse_variables(vars: Option<&Value>) -> HashMap<String, String> {
        let mut env = HashMap::new();
        let Some(vars) = vars else {
            return env;
        };

        match vars {
            Value::Mapping(map) => {
                for (key, value) in map {
                    if let (Some(k), Some(v)) = (key.as_str(), value.as_str()) {
                        env.insert(k.to_string(), v.to_string());
                    }
                }
            }
            Value::Sequence(seq) => {
                for item in seq {
                    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                        let value = item
                            .get("value")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        env.insert(name.to_string(), value);
                    }
                }
            }
            _ => {}
        }

        env
    }
}

struct ParsedStage {
    name: String,
    depends_on: Vec<String>,
    job_ids: Vec<String>,
}

struct ParsedJob {
    id: String,
    depends_on_raw: Vec<String>,
}

fn parse_depends_on(depends: Option<&Value>) -> Vec<String> {
    let Some(depends) = depends else {
        return Vec::new();
    };

    match depends {
        Value::String(s) => vec![s.clone()],
        Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_pool_name(pool: Option<&Value>) -> Option<String> {
    let pool = pool?;
    if let Some(name) = pool.as_str() {
        return Some(format!("azure:{}", name));
    }
    if let Some(map) = pool.as_mapping() {
        if let Some(vm) = map
            .get(Value::String("vmImage".to_string()))
            .and_then(|v| v.as_str())
        {
            return Some(format!("azure:{}", vm));
        }
        if let Some(name) = map
            .get(Value::String("name".to_string()))
            .and_then(|v| v.as_str())
        {
            return Some(format!("azure:{}", name));
        }
    }
    None
}

fn extract_steps(job_value: &Value) -> Vec<StepInfo> {
    let direct_steps = job_value.get("steps").and_then(|v| v.as_sequence());
    let deployment_steps = job_value
        .get("strategy")
        .and_then(|v| v.get("runOnce"))
        .and_then(|v| v.get("deploy"))
        .and_then(|v| v.get("steps"))
        .and_then(|v| v.as_sequence());

    let steps = direct_steps.or(deployment_steps);
    let Some(steps) = steps else {
        return vec![StepInfo {
            name: "job".to_string(),
            uses: None,
            run: Some("azure job".to_string()),
            estimated_duration_secs: Some(60.0),
        }];
    };

    let mut parsed = Vec::new();
    for step in steps {
        match step {
            Value::String(cmd) => parsed.push(StepInfo {
                name: "script".to_string(),
                uses: None,
                run: Some(cmd.clone()),
                estimated_duration_secs: Some(estimate_cmd_duration(cmd)),
            }),
            Value::Mapping(_) => {
                if let Some(script) = step.get("script").and_then(|v| v.as_str()) {
                    parsed.push(StepInfo {
                        name: step
                            .get("displayName")
                            .and_then(|v| v.as_str())
                            .unwrap_or("script")
                            .to_string(),
                        uses: None,
                        run: Some(script.to_string()),
                        estimated_duration_secs: Some(estimate_cmd_duration(script)),
                    });
                } else if let Some(bash) = step.get("bash").and_then(|v| v.as_str()) {
                    parsed.push(StepInfo {
                        name: "bash".to_string(),
                        uses: None,
                        run: Some(bash.to_string()),
                        estimated_duration_secs: Some(estimate_cmd_duration(bash)),
                    });
                } else if let Some(pwsh) = step.get("pwsh").and_then(|v| v.as_str()) {
                    parsed.push(StepInfo {
                        name: "pwsh".to_string(),
                        uses: None,
                        run: Some(pwsh.to_string()),
                        estimated_duration_secs: Some(estimate_cmd_duration(pwsh)),
                    });
                } else if let Some(task) = step.get("task").and_then(|v| v.as_str()) {
                    parsed.push(StepInfo {
                        name: step
                            .get("displayName")
                            .and_then(|v| v.as_str())
                            .unwrap_or(task)
                            .to_string(),
                        uses: Some(task.to_string()),
                        run: None,
                        estimated_duration_secs: Some(estimate_task_duration(task)),
                    });
                } else if let Some(template) = step.get("template").and_then(|v| v.as_str()) {
                    parsed.push(StepInfo {
                        name: "template".to_string(),
                        uses: Some(template.to_string()),
                        run: None,
                        estimated_duration_secs: Some(5.0),
                    });
                } else {
                    parsed.push(StepInfo {
                        name: "step".to_string(),
                        uses: None,
                        run: Some("azure step".to_string()),
                        estimated_duration_secs: Some(20.0),
                    });
                }
            }
            _ => {}
        }
    }

    if parsed.is_empty() {
        parsed.push(StepInfo {
            name: "job".to_string(),
            uses: None,
            run: Some("azure job".to_string()),
            estimated_duration_secs: Some(60.0),
        });
    }

    parsed
}

fn detect_caches(steps: &[StepInfo]) -> Vec<CacheConfig> {
    let mut caches = Vec::new();
    for step in steps {
        let uses = step.uses.as_deref().unwrap_or("").to_lowercase();
        let run = step.run.as_deref().unwrap_or("").to_lowercase();
        if uses.contains("cache@")
            || run.contains("cache")
            || run.contains("npm ci")
            || run.contains("pip install")
            || run.contains("cargo build")
        {
            caches.push(CacheConfig {
                path: "detected".to_string(),
                key_pattern: "detected".to_string(),
                restore_keys: Vec::new(),
            });
            break;
        }
    }
    caches
}

fn estimate_task_duration(task: &str) -> f64 {
    let lower = task.to_lowercase();
    if lower.contains("cache@") {
        return 10.0;
    }
    if lower.contains("npm") || lower.contains("node") {
        return 120.0;
    }
    if lower.contains("dotnet") {
        return 180.0;
    }
    if lower.contains("docker") {
        return 240.0;
    }
    30.0
}

fn estimate_cmd_duration(cmd: &str) -> f64 {
    let cmd = cmd.to_lowercase();
    if cmd.contains("npm ci") || cmd.contains("npm install") {
        return 180.0;
    }
    if cmd.contains("yarn install") || cmd.contains("pnpm install") {
        return 170.0;
    }
    if cmd.contains("pip install") {
        return 140.0;
    }
    if cmd.contains("dotnet restore") {
        return 120.0;
    }
    if cmd.contains("dotnet build") || cmd.contains("npm run build") {
        return 240.0;
    }
    if cmd.contains("pytest") || cmd.contains("npm test") || cmd.contains("dotnet test") {
        return 300.0;
    }
    if cmd.contains("docker build") {
        return 260.0;
    }
    60.0
}

fn sanitize_id(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut prev_dash = false;

    for ch in value.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };
        if mapped == '-' {
            if !prev_dash {
                out.push(mapped);
                prev_dash = true;
            }
        } else {
            out.push(mapped);
            prev_dash = false;
        }
    }

    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stages_jobs_dependencies() {
        let config = r#"
name: Sample Azure
trigger:
  branches:
    include: [main]

stages:
  - stage: Build
    jobs:
      - job: BuildApp
        pool:
          vmImage: ubuntu-latest
        steps:
          - script: npm ci
          - script: npm run build
  - stage: Test
    dependsOn: Build
    jobs:
      - job: UnitTests
        dependsOn: BuildApp
        steps:
          - task: Cache@2
          - script: npm test
"#;

        let dag = AzurePipelinesParser::parse(config, "azure-pipelines.yml".to_string()).unwrap();
        assert_eq!(dag.provider, "azure-pipelines");
        assert_eq!(dag.job_count(), 2);

        let build = dag.get_job("build-buildapp").unwrap();
        let test = dag.get_job("test-unittests").unwrap();

        assert_eq!(build.runs_on, "azure:ubuntu-latest");
        assert!(test.needs.contains(&"build-buildapp".to_string()));
    }

    #[test]
    fn test_parse_templates_and_deployment_job() {
        let config = r#"
stages:
  - stage: Deploy
    jobs:
      - template: templates/release-job.yml
      - deployment: DeployProd
        strategy:
          runOnce:
            deploy:
              steps:
                - template: templates/deploy-steps.yml
                - script: ./deploy.sh
"#;

        let dag = AzurePipelinesParser::parse(config, "azure-pipelines.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 2);

        let deploy = dag.get_job("deploy-deployprod").unwrap();
        assert!(
            deploy.steps.iter().any(|s| s.uses.is_some()),
            "deployment should include template step"
        );
    }
}
