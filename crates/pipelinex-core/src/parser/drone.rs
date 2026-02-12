use crate::parser::dag::*;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_yaml::Value;
use std::path::Path;

/// Parser for Drone CI / Woodpecker CI (.drone.yml / .woodpecker.yml).
pub struct DroneParser;

impl DroneParser {
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read Drone CI file: {}", path.display()))?;
        Self::parse(&content, path.to_string_lossy().to_string())
    }

    pub fn parse_content(content: &str, source_name: &str) -> Result<PipelineDag> {
        Self::parse(content, source_name.to_string())
    }

    pub fn parse(content: &str, source_file: String) -> Result<PipelineDag> {
        // Drone configs can contain multiple YAML documents (multi-pipeline)
        // Try parsing as multi-doc first
        let docs: Vec<Value> = serde_yaml::Deserializer::from_str(content)
            .map(Value::deserialize)
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to parse YAML")?;

        if docs.is_empty() {
            anyhow::bail!("Empty Drone CI configuration");
        }

        // If multiple pipeline documents, parse each as a job in the DAG
        if docs.len() > 1 {
            return Self::parse_multi_pipeline(&docs, source_file);
        }

        Self::parse_single(&docs[0], source_file)
    }

    fn parse_single(yaml: &Value, source_file: String) -> Result<PipelineDag> {
        let kind = yaml
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("pipeline");

        if kind != "pipeline" {
            // Skip non-pipeline documents (secrets, signatures, etc.)
            let name = yaml
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed")
                .to_string();
            return Ok(PipelineDag::new(name, source_file, "drone".to_string()));
        }

        let name = yaml
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed Pipeline")
            .to_string();

        let mut dag = PipelineDag::new(name, source_file, "drone".to_string());

        // Parse trigger
        if let Some(trigger) = yaml.get("trigger") {
            dag.triggers = Self::parse_trigger(trigger);
        }

        // Parse platform
        let platform = yaml
            .get("platform")
            .and_then(|v| v.get("os"))
            .and_then(|v| v.as_str())
            .unwrap_or("linux");

        // Parse steps
        let steps = yaml
            .get("steps")
            .and_then(|v| v.as_sequence())
            .context("No 'steps' section found in Drone pipeline")?;

        // In Drone, steps within a pipeline run sequentially by default
        // unless depends_on is used to create explicit dependencies
        let has_depends_on = steps.iter().any(|s| s.get("depends_on").is_some());

        // First pass: create all step nodes
        for step in steps {
            let job = Self::parse_step(step, platform)?;
            dag.add_job(job);
        }

        // Second pass: add dependency edges
        if has_depends_on {
            // Explicit dependency mode
            for step in steps {
                let step_name = step
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unnamed")
                    .to_string();

                if let Some(deps) = step.get("depends_on").and_then(|v| v.as_sequence()) {
                    for dep in deps {
                        if let Some(dep_name) = dep.as_str() {
                            let _ = dag.add_dependency(dep_name, &step_name);
                        }
                    }
                }
            }
        } else {
            // Sequential mode: each step depends on the previous
            let step_names: Vec<String> = steps
                .iter()
                .filter_map(|s| s.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect();

            for i in 1..step_names.len() {
                let _ = dag.add_dependency(&step_names[i - 1], &step_names[i]);
            }
        }

        Ok(dag)
    }

    fn parse_multi_pipeline(docs: &[Value], source_file: String) -> Result<PipelineDag> {
        let mut dag = PipelineDag::new(
            "Multi-Pipeline".to_string(),
            source_file,
            "drone".to_string(),
        );

        let mut pipeline_names = Vec::new();

        for doc in docs {
            let kind = doc
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("pipeline");

            if kind != "pipeline" {
                continue;
            }

            let pipeline_name = doc
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();

            // Each pipeline becomes a single aggregated job
            let steps = doc
                .get("steps")
                .and_then(|v| v.as_sequence())
                .unwrap_or(&Vec::new())
                .clone();

            let mut job = JobNode::new(pipeline_name.clone(), pipeline_name.clone());
            let mut total_duration = 0.0;

            for step in &steps {
                let step_info = Self::parse_step_info(step);
                total_duration += step_info.estimated_duration_secs.unwrap_or(30.0);
                job.steps.push(step_info);
            }

            job.estimated_duration_secs = total_duration;

            // depends_on at pipeline level
            if let Some(deps) = doc.get("depends_on").and_then(|v| v.as_sequence()) {
                job.needs = deps
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
            }

            dag.add_job(job);
            pipeline_names.push(pipeline_name);
        }

        // Add cross-pipeline dependency edges
        for doc in docs {
            let kind = doc.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if kind != "pipeline" {
                continue;
            }

            let pipeline_name = doc
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();

            if let Some(deps) = doc.get("depends_on").and_then(|v| v.as_sequence()) {
                for dep in deps {
                    if let Some(dep_name) = dep.as_str() {
                        let _ = dag.add_dependency(dep_name, &pipeline_name);
                    }
                }
            }
        }

        if dag.name == "Multi-Pipeline" && !pipeline_names.is_empty() {
            dag.name = pipeline_names.join(" + ");
        }

        Ok(dag)
    }

    fn parse_step(step: &Value, platform: &str) -> Result<JobNode> {
        let name = step
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed-step")
            .to_string();

        let mut job = JobNode::new(name.clone(), name);

        let image = step.get("image").and_then(|v| v.as_str()).unwrap_or("");
        job.runs_on = format!("{} ({})", platform, image);

        // Commands
        let commands = step
            .get("commands")
            .and_then(|v| v.as_sequence())
            .map(|seq| seq.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>());

        if let Some(cmds) = &commands {
            for cmd in cmds {
                job.steps.push(StepInfo {
                    name: cmd.to_string(),
                    uses: Some(image.to_string()),
                    run: Some(cmd.to_string()),
                    estimated_duration_secs: Some(Self::estimate_command_duration(cmd)),
                });
            }
        } else {
            // Single step with just an image (plugin)
            job.steps.push(StepInfo {
                name: format!("plugin: {}", image),
                uses: Some(image.to_string()),
                run: None,
                estimated_duration_secs: Some(Self::estimate_plugin_duration(image)),
            });
        }

        // Settings (Drone plugin settings)
        if let Some(settings) = step.get("settings").and_then(|v| v.as_mapping()) {
            for (k, v) in settings {
                if let (Some(key), Some(val)) = (k.as_str(), v.as_str()) {
                    job.env.insert(key.to_string(), val.to_string());
                }
            }
        }

        // Environment variables
        if let Some(env) = step.get("environment").and_then(|v| v.as_mapping()) {
            for (k, v) in env {
                if let (Some(key), Some(val)) = (k.as_str(), v.as_str()) {
                    job.env.insert(key.to_string(), val.to_string());
                }
            }
        }

        // Conditions
        if let Some(when) = step.get("when") {
            let cond_str = serde_yaml::to_string(when).unwrap_or_default();
            job.condition = Some(cond_str.trim().to_string());
        }

        // depends_on -> needs
        if let Some(deps) = step.get("depends_on").and_then(|v| v.as_sequence()) {
            job.needs = deps
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }

        job.estimated_duration_secs = job
            .steps
            .iter()
            .filter_map(|s| s.estimated_duration_secs)
            .sum();

        if job.estimated_duration_secs == 0.0 {
            job.estimated_duration_secs = 30.0;
        }

        Ok(job)
    }

    fn parse_step_info(step: &Value) -> StepInfo {
        let name = step
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed")
            .to_string();

        let image = step.get("image").and_then(|v| v.as_str()).map(String::from);

        let commands = step
            .get("commands")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(" && ")
            });

        let duration = if let Some(ref cmds) = commands {
            Self::estimate_command_duration(cmds)
        } else if let Some(ref img) = image {
            Self::estimate_plugin_duration(img)
        } else {
            30.0
        };

        StepInfo {
            name,
            uses: image,
            run: commands,
            estimated_duration_secs: Some(duration),
        }
    }

    fn parse_trigger(trigger: &Value) -> Vec<WorkflowTrigger> {
        let mut triggers = Vec::new();

        if let Some(event) = trigger.get("event") {
            let events = match event {
                Value::String(s) => vec![s.clone()],
                Value::Sequence(seq) => seq
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                _ => Vec::new(),
            };

            let branches = trigger.get("branch").and_then(|v| match v {
                Value::String(s) => Some(vec![s.clone()]),
                Value::Sequence(seq) => Some(
                    seq.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                ),
                Value::Mapping(m) => m.get(Value::String("include".into())).and_then(|v| {
                    v.as_sequence().map(|seq| {
                        seq.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                }),
                _ => None,
            });

            for event_name in events {
                triggers.push(WorkflowTrigger {
                    event: event_name,
                    branches: branches.clone(),
                    paths: None,
                    paths_ignore: None,
                });
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

    fn estimate_command_duration(cmd: &str) -> f64 {
        let lower = cmd.to_lowercase();
        if lower.contains("npm install")
            || lower.contains("npm ci")
            || lower.contains("yarn install")
            || lower.contains("pip install")
        {
            180.0
        } else if lower.contains("cargo build") || lower.contains("go build") {
            300.0
        } else if lower.contains("npm run build") || lower.contains("yarn build") {
            240.0
        } else if lower.contains("npm test")
            || lower.contains("pytest")
            || lower.contains("cargo test")
            || lower.contains("go test")
        {
            300.0
        } else if lower.contains("lint") || lower.contains("eslint") || lower.contains("clippy") {
            60.0
        } else if lower.contains("docker build") {
            300.0
        } else if lower.contains("deploy") || lower.contains("kubectl") {
            120.0
        } else {
            30.0
        }
    }

    fn estimate_plugin_duration(image: &str) -> f64 {
        let lower = image.to_lowercase();
        if lower.contains("docker") || lower.contains("ecr") || lower.contains("gcr") {
            300.0
        } else if lower.contains("s3") || lower.contains("gcs") || lower.contains("artifact") {
            60.0
        } else if lower.contains("slack") || lower.contains("email") || lower.contains("notify") {
            5.0
        } else if lower.contains("terraform") || lower.contains("ansible") {
            120.0
        } else if lower.contains("cache") {
            10.0
        } else {
            30.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_drone_simple() {
        let yaml = r#"
kind: pipeline
name: default
steps:
  - name: test
    image: golang:1.21
    commands:
      - go test ./...
  - name: build
    image: golang:1.21
    commands:
      - go build -o app
"#;
        let dag = DroneParser::parse(yaml, ".drone.yml".into()).unwrap();
        assert_eq!(dag.provider, "drone");
        assert_eq!(dag.job_count(), 2);
        assert_eq!(dag.name, "default");

        // Sequential: build depends on test
        // In sequential mode we don't set needs on JobNode but add edges
        assert_eq!(dag.root_jobs().len(), 1);
    }

    #[test]
    fn test_parse_drone_with_depends_on() {
        let yaml = r#"
kind: pipeline
name: ci
steps:
  - name: clone
    image: alpine/git
    commands:
      - git clone https://github.com/test/repo
  - name: test
    image: node:18
    commands:
      - npm test
    depends_on:
      - clone
  - name: lint
    image: node:18
    commands:
      - npm run lint
    depends_on:
      - clone
  - name: deploy
    image: plugins/docker
    depends_on:
      - test
      - lint
"#;
        let dag = DroneParser::parse(yaml, ".drone.yml".into()).unwrap();
        assert_eq!(dag.job_count(), 4);
        // clone is root, test and lint depend on clone, deploy depends on both
        assert_eq!(dag.root_jobs().len(), 1);
        let deploy = dag.get_job("deploy").unwrap();
        assert_eq!(deploy.needs.len(), 2);
    }

    #[test]
    fn test_parse_drone_multi_pipeline() {
        let yaml = "kind: pipeline\nname: test\nsteps:\n  - name: test\n    image: node:18\n    commands:\n      - npm test\n---\nkind: pipeline\nname: deploy\ndepends_on:\n  - test\nsteps:\n  - name: deploy\n    image: plugins/docker\n";
        let dag = DroneParser::parse(yaml, ".drone.yml".into()).unwrap();
        assert_eq!(dag.job_count(), 2);
        assert!(dag.get_job("test").is_some());
        assert!(dag.get_job("deploy").is_some());
    }

    #[test]
    fn test_parse_drone_with_trigger() {
        let yaml = r#"
kind: pipeline
name: default
trigger:
  event:
    - push
    - pull_request
  branch:
    - main
    - develop
steps:
  - name: test
    image: node:18
    commands:
      - npm test
"#;
        let dag = DroneParser::parse(yaml, ".drone.yml".into()).unwrap();
        assert_eq!(dag.triggers.len(), 2);
        assert_eq!(dag.triggers[0].event, "push");
        assert!(dag.triggers[0].branches.is_some());
    }
}
