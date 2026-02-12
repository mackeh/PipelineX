use crate::parser::dag::*;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_yaml::Value;
use std::path::Path;

/// Parser for Tekton Pipeline, Task, and PipelineRun CRDs.
pub struct TektonParser;

impl TektonParser {
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read Tekton file: {}", path.display()))?;
        Self::parse(&content, path.to_string_lossy().to_string())
    }

    pub fn parse_content(content: &str, source_name: &str) -> Result<PipelineDag> {
        Self::parse(content, source_name.to_string())
    }

    pub fn parse(content: &str, source_file: String) -> Result<PipelineDag> {
        let docs: Vec<Value> = serde_yaml::Deserializer::from_str(content)
            .map(Value::deserialize)
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to parse YAML")?;

        let Some(selected) = docs.iter().max_by_key(|doc| Self::document_priority(doc)) else {
            anyhow::bail!("Tekton file is empty");
        };

        if Self::document_priority(selected) == 0 {
            anyhow::bail!("No Tekton Pipeline/Task/PipelineRun document found");
        }

        let kind = selected
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("Pipeline");

        match kind {
            "Pipeline" => Self::parse_pipeline(selected, source_file),
            "PipelineRun" => Self::parse_pipeline_run(selected, source_file),
            "Task" => Self::parse_task_as_pipeline(selected, source_file),
            _ => Self::parse_pipeline(selected, source_file),
        }
    }

    fn document_priority(yaml: &Value) -> u8 {
        let kind = yaml
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        match kind {
            "Pipeline" => 4,
            "PipelineRun" => 3,
            "Task" => 2,
            _ => {
                let api = yaml
                    .get("apiVersion")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if api.contains("tekton.dev") {
                    1
                } else {
                    0
                }
            }
        }
    }

    fn parse_pipeline(yaml: &Value, source_file: String) -> Result<PipelineDag> {
        let metadata = yaml.get("metadata").unwrap_or(yaml);
        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed Tekton Pipeline")
            .to_string();

        let mut dag = PipelineDag::new(name, source_file, "tekton".to_string());

        let spec = match yaml.get("spec") {
            Some(s) => s,
            None => return Ok(dag),
        };

        // Parse pipeline tasks
        let tasks = spec
            .get("tasks")
            .and_then(|v| v.as_sequence())
            .unwrap_or(&Vec::new())
            .clone();

        let finally_tasks = spec
            .get("finally")
            .and_then(|v| v.as_sequence())
            .unwrap_or(&Vec::new())
            .clone();

        // First pass: create all task nodes
        for task in &tasks {
            let job = Self::parse_pipeline_task(task)?;
            dag.add_job(job);
        }

        for task in &finally_tasks {
            let mut job = Self::parse_pipeline_task(task)?;
            job.condition = Some("finally".to_string());
            dag.add_job(job);
        }

        // Second pass: add dependency edges (runAfter)
        for task in &tasks {
            let task_name = task
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            if let Some(run_after) = task.get("runAfter").and_then(|v| v.as_sequence()) {
                for dep in run_after {
                    if let Some(dep_name) = dep.as_str() {
                        let _ = dag.add_dependency(dep_name, &task_name);
                    }
                }
            }
        }

        // Finally tasks depend on all regular tasks
        let regular_task_ids: Vec<String> = tasks
            .iter()
            .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
            .collect();

        let leaf_tasks: Vec<String> = regular_task_ids
            .iter()
            .filter(|id| {
                // A leaf task is one that no other task depends on
                !tasks.iter().any(|t| {
                    t.get("runAfter")
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .any(|d| d.as_str().map(|s| s == id.as_str()).unwrap_or(false))
                        })
                        .unwrap_or(false)
                })
            })
            .cloned()
            .collect();

        for finally_task in &finally_tasks {
            let task_name = finally_task
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            for leaf in &leaf_tasks {
                let _ = dag.add_dependency(leaf, &task_name);
            }
        }

        Ok(dag)
    }

    fn parse_pipeline_task(task: &Value) -> Result<JobNode> {
        let name = task
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed-task")
            .to_string();

        let mut job = JobNode::new(name.clone(), name);

        // Task reference
        if let Some(task_ref) = task.get("taskRef") {
            let ref_name = task_ref
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            job.steps.push(StepInfo {
                name: format!("taskRef: {}", ref_name),
                uses: Some(ref_name.to_string()),
                run: None,
                estimated_duration_secs: Some(Self::estimate_task_duration(ref_name)),
            });
        }

        // Inline task spec (taskSpec)
        if let Some(task_spec) = task.get("taskSpec") {
            if let Some(steps) = task_spec.get("steps").and_then(|v| v.as_sequence()) {
                for step in steps {
                    job.steps.push(Self::parse_step(step));
                }
            }
        }

        // runAfter -> needs
        if let Some(run_after) = task.get("runAfter").and_then(|v| v.as_sequence()) {
            job.needs = run_after
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }

        // when conditions
        if let Some(when) = task.get("when").and_then(|v| v.as_sequence()) {
            if !when.is_empty() {
                job.condition = Some("when-expression".to_string());
            }
        }

        // params -> env
        if let Some(params) = task.get("params").and_then(|v| v.as_sequence()) {
            for param in params {
                if let (Some(name), Some(value)) = (
                    param.get("name").and_then(|v| v.as_str()),
                    param.get("value").and_then(|v| v.as_str()),
                ) {
                    job.env.insert(name.to_string(), value.to_string());
                }
            }
        }

        job.estimated_duration_secs = job
            .steps
            .iter()
            .filter_map(|s| s.estimated_duration_secs)
            .sum();

        if job.estimated_duration_secs == 0.0 {
            job.estimated_duration_secs = 60.0; // Default estimate
        }

        Ok(job)
    }

    fn parse_step(step: &Value) -> StepInfo {
        let name = step
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed-step")
            .to_string();

        let image = step.get("image").and_then(|v| v.as_str()).map(String::from);

        let script = step
            .get("script")
            .and_then(|v| v.as_str())
            .map(String::from);

        let command = step
            .get("command")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            });

        let run = script.or(command);
        let estimated_duration = Self::estimate_step_duration(&image, &run);

        StepInfo {
            name,
            uses: image,
            run,
            estimated_duration_secs: Some(estimated_duration),
        }
    }

    fn parse_pipeline_run(yaml: &Value, source_file: String) -> Result<PipelineDag> {
        let metadata = yaml.get("metadata").unwrap_or(yaml);
        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed PipelineRun")
            .to_string();

        let spec = yaml.get("spec").unwrap_or(yaml);

        // If pipelineSpec is inlined, parse that
        if let Some(pipeline_spec) = spec.get("pipelineSpec") {
            let mut wrapper = serde_yaml::Mapping::new();
            wrapper.insert(Value::String("spec".into()), pipeline_spec.clone());
            wrapper.insert(Value::String("metadata".into()), {
                let mut m = serde_yaml::Mapping::new();
                m.insert(Value::String("name".into()), Value::String(name));
                Value::Mapping(m)
            });
            return Self::parse_pipeline(&Value::Mapping(wrapper), source_file);
        }

        // Otherwise just create an empty DAG with the pipeline reference
        let dag = PipelineDag::new(name, source_file, "tekton".to_string());
        Ok(dag)
    }

    fn parse_task_as_pipeline(yaml: &Value, source_file: String) -> Result<PipelineDag> {
        let metadata = yaml.get("metadata").unwrap_or(yaml);
        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed Task")
            .to_string();

        let mut dag = PipelineDag::new(name.clone(), source_file, "tekton".to_string());

        let mut job = JobNode::new(name.clone(), name);

        if let Some(spec) = yaml.get("spec") {
            if let Some(steps) = spec.get("steps").and_then(|v| v.as_sequence()) {
                for step in steps {
                    job.steps.push(Self::parse_step(step));
                }
            }
        }

        job.estimated_duration_secs = job
            .steps
            .iter()
            .filter_map(|s| s.estimated_duration_secs)
            .sum();

        if job.estimated_duration_secs == 0.0 {
            job.estimated_duration_secs = 60.0;
        }

        dag.add_job(job);
        Ok(dag)
    }

    fn estimate_task_duration(task_name: &str) -> f64 {
        let lower = task_name.to_lowercase();
        if lower.contains("git-clone") || lower.contains("clone") {
            15.0
        } else if lower.contains("build") || lower.contains("compile") || lower.contains("test") {
            300.0
        } else if lower.contains("lint") || lower.contains("check") {
            60.0
        } else if lower.contains("push") || lower.contains("deploy") {
            120.0
        } else if lower.contains("scan") || lower.contains("security") {
            90.0
        } else {
            60.0
        }
    }

    fn estimate_step_duration(image: &Option<String>, run: &Option<String>) -> f64 {
        if let Some(run) = run {
            let cmd = run.to_lowercase();
            if cmd.contains("build")
                || cmd.contains("compile")
                || cmd.contains("test")
                || cmd.contains("pytest")
            {
                return 300.0;
            }
            if cmd.contains("install") || cmd.contains("npm ci") {
                return 180.0;
            }
            if cmd.contains("deploy") || cmd.contains("kubectl") {
                return 120.0;
            }
            if cmd.contains("lint") || cmd.contains("check") {
                return 60.0;
            }
        }
        if let Some(image) = image {
            if image.contains("kaniko") || image.contains("buildah") {
                return 300.0;
            }
        }
        30.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tekton_pipeline() {
        let yaml = r#"
apiVersion: tekton.dev/v1beta1
kind: Pipeline
metadata:
  name: build-and-test
spec:
  tasks:
    - name: clone
      taskRef:
        name: git-clone
    - name: build
      taskRef:
        name: golang-build
      runAfter:
        - clone
    - name: test
      taskRef:
        name: golang-test
      runAfter:
        - clone
    - name: deploy
      taskRef:
        name: kubectl-deploy
      runAfter:
        - build
        - test
"#;
        let dag = TektonParser::parse(yaml, "pipeline.yaml".into()).unwrap();
        assert_eq!(dag.provider, "tekton");
        assert_eq!(dag.job_count(), 4);
        assert_eq!(dag.name, "build-and-test");

        let deploy = dag.get_job("deploy").unwrap();
        assert_eq!(deploy.needs.len(), 2);
    }

    #[test]
    fn test_parse_tekton_with_finally() {
        let yaml = r#"
apiVersion: tekton.dev/v1beta1
kind: Pipeline
metadata:
  name: ci-with-cleanup
spec:
  tasks:
    - name: build
      taskRef:
        name: build-task
    - name: test
      taskRef:
        name: test-task
      runAfter:
        - build
  finally:
    - name: cleanup
      taskRef:
        name: cleanup-task
"#;
        let dag = TektonParser::parse(yaml, "pipeline.yaml".into()).unwrap();
        assert_eq!(dag.job_count(), 3);
        let cleanup = dag.get_job("cleanup").unwrap();
        assert_eq!(cleanup.condition.as_deref(), Some("finally"));
    }

    #[test]
    fn test_parse_tekton_task() {
        let yaml = r#"
apiVersion: tekton.dev/v1beta1
kind: Task
metadata:
  name: my-build-task
spec:
  steps:
    - name: build
      image: golang:1.21
      script: |
        go build ./...
    - name: test
      image: golang:1.21
      script: |
        go test ./...
"#;
        let dag = TektonParser::parse(yaml, "task.yaml".into()).unwrap();
        assert_eq!(dag.job_count(), 1);
        let job = dag.get_job("my-build-task").unwrap();
        assert_eq!(job.steps.len(), 2);
    }

    #[test]
    fn test_parse_tekton_inline_taskspec() {
        let yaml = r#"
apiVersion: tekton.dev/v1beta1
kind: Pipeline
metadata:
  name: inline-pipeline
spec:
  tasks:
    - name: lint
      taskSpec:
        steps:
          - name: run-lint
            image: golangci/golangci-lint
            command: ["golangci-lint", "run"]
    - name: build
      taskRef:
        name: build-task
      runAfter:
        - lint
"#;
        let dag = TektonParser::parse(yaml, "pipeline.yaml".into()).unwrap();
        assert_eq!(dag.job_count(), 2);
        let lint = dag.get_job("lint").unwrap();
        assert_eq!(lint.steps.len(), 1);
    }

    #[test]
    fn test_parse_tekton_multi_document_prefers_pipeline() {
        let yaml = r#"
apiVersion: tekton.dev/v1beta1
kind: Task
metadata:
  name: unit-tests
spec:
  steps:
    - name: test
      image: golang:1.21
      script: go test ./...
---
apiVersion: tekton.dev/v1beta1
kind: Pipeline
metadata:
  name: app-ci
spec:
  tasks:
    - name: build
      taskRef:
        name: kaniko-build
    - name: test
      taskRef:
        name: unit-tests
      runAfter:
        - build
"#;
        let dag = TektonParser::parse(yaml, "pipeline.yaml".into()).unwrap();
        assert_eq!(dag.provider, "tekton");
        assert_eq!(dag.name, "app-ci");
        assert_eq!(dag.job_count(), 2);
        assert!(dag.get_job("build").is_some());
        assert!(dag.get_job("test").is_some());
    }
}
