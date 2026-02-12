use crate::parser::dag::*;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::Path;

/// Parser for Argo Workflows and WorkflowTemplate CRDs.
pub struct ArgoWorkflowsParser;

impl ArgoWorkflowsParser {
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read Argo Workflows file: {}", path.display()))?;
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
            anyhow::bail!("Argo workflow file is empty");
        };

        if Self::document_priority(selected) == 0 {
            anyhow::bail!("No Argo Workflow/Template document found");
        }

        Self::parse_document(selected, source_file)
    }

    fn parse_document(yaml: &Value, source_file: String) -> Result<PipelineDag> {
        let metadata = yaml.get("metadata").unwrap_or(yaml);
        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed Argo Workflow")
            .to_string();

        let mut dag = PipelineDag::new(name, source_file, "argo-workflows".to_string());

        let spec = match yaml.get("spec") {
            Some(s) => s,
            None => return Ok(dag),
        };

        // Collect all templates for reference
        let empty_vec = Vec::new();
        let templates: HashMap<String, &Value> = spec
            .get("templates")
            .and_then(|v| v.as_sequence())
            .unwrap_or(&empty_vec)
            .iter()
            .filter_map(|t| {
                t.get("name")
                    .and_then(|v| v.as_str())
                    .map(|name| (name.to_string(), t))
            })
            .collect();

        // Find the entrypoint template
        let entrypoint = spec
            .get("entrypoint")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if let Some(entry_template) = templates.get(entrypoint) {
            Self::process_template(entry_template, &templates, &mut dag)?;
        } else {
            // If no entrypoint, try to process all templates
            for template in templates.values() {
                Self::process_template(template, &templates, &mut dag)?;
            }
        }

        Ok(dag)
    }

    fn document_priority(yaml: &Value) -> u8 {
        let kind = yaml
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        match kind {
            "Workflow" | "CronWorkflow" => 3,
            "WorkflowTemplate" | "ClusterWorkflowTemplate" => 2,
            _ => {
                let api = yaml
                    .get("apiVersion")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if api.contains("argoproj.io") {
                    1
                } else {
                    0
                }
            }
        }
    }

    fn process_template(
        template: &Value,
        all_templates: &HashMap<String, &Value>,
        dag: &mut PipelineDag,
    ) -> Result<()> {
        // DAG template
        if let Some(dag_spec) = template.get("dag") {
            Self::process_dag_template(dag_spec, all_templates, dag)?;
        }
        // Steps template (sequential groups of parallel steps)
        else if let Some(steps_spec) = template.get("steps") {
            Self::process_steps_template(steps_spec, all_templates, dag)?;
        }
        // Container template (single step)
        else if template.get("container").is_some() || template.get("script").is_some() {
            let name = template
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();

            let job = Self::template_to_job(template, &name)?;
            dag.add_job(job);
        }

        Ok(())
    }

    fn process_dag_template(
        dag_spec: &Value,
        all_templates: &HashMap<String, &Value>,
        dag: &mut PipelineDag,
    ) -> Result<()> {
        let tasks = dag_spec
            .get("tasks")
            .and_then(|v| v.as_sequence())
            .unwrap_or(&Vec::new())
            .clone();

        // First pass: create all task nodes
        for task in &tasks {
            let task_name = task
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();

            let template_ref = task.get("template").and_then(|v| v.as_str()).unwrap_or("");

            let mut job = if let Some(tmpl) = all_templates.get(template_ref) {
                Self::template_to_job(tmpl, &task_name)?
            } else {
                let mut j = JobNode::new(task_name.clone(), task_name.clone());
                j.steps.push(StepInfo {
                    name: format!("template: {}", template_ref),
                    uses: Some(template_ref.to_string()),
                    run: None,
                    estimated_duration_secs: Some(60.0),
                });
                j.estimated_duration_secs = 60.0;
                j
            };

            // Dependencies
            if let Some(deps) = task.get("dependencies").and_then(|v| v.as_sequence()) {
                job.needs = deps
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
            }

            // When expression
            if let Some(when) = task.get("when").and_then(|v| v.as_str()) {
                job.condition = Some(when.to_string());
            }

            // Arguments -> env
            if let Some(args) = task.get("arguments") {
                if let Some(params) = args.get("parameters").and_then(|v| v.as_sequence()) {
                    for param in params {
                        if let (Some(name), Some(value)) = (
                            param.get("name").and_then(|v| v.as_str()),
                            param.get("value").and_then(|v| v.as_str()),
                        ) {
                            job.env.insert(name.to_string(), value.to_string());
                        }
                    }
                }
            }

            dag.add_job(job);
        }

        // Second pass: add dependency edges
        for task in &tasks {
            let task_name = task
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();

            if let Some(deps) = task.get("dependencies").and_then(|v| v.as_sequence()) {
                for dep in deps {
                    if let Some(dep_name) = dep.as_str() {
                        let _ = dag.add_dependency(dep_name, &task_name);
                    }
                }
            }
        }

        Ok(())
    }

    fn process_steps_template(
        steps_spec: &Value,
        all_templates: &HashMap<String, &Value>,
        dag: &mut PipelineDag,
    ) -> Result<()> {
        let step_groups = steps_spec.as_sequence().unwrap_or(&Vec::new()).clone();

        let mut prev_group_ids: Vec<String> = Vec::new();

        for (group_idx, group) in step_groups.iter().enumerate() {
            let steps = group.as_sequence().unwrap_or(&Vec::new()).clone();
            let mut current_group_ids = Vec::new();

            for step in &steps {
                let step_name = step
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unnamed")
                    .to_string();

                let unique_name = format!("step-{}-{}", group_idx, step_name);

                let template_ref = step.get("template").and_then(|v| v.as_str()).unwrap_or("");

                let mut job = if let Some(tmpl) = all_templates.get(template_ref) {
                    let mut j = Self::template_to_job(tmpl, &unique_name)?;
                    j.name = step_name;
                    j
                } else {
                    let mut j = JobNode::new(unique_name.clone(), step_name);
                    j.estimated_duration_secs = 60.0;
                    j
                };

                // Each step group depends on all steps from the previous group
                job.needs = prev_group_ids.clone();

                // When condition
                if let Some(when) = step.get("when").and_then(|v| v.as_str()) {
                    job.condition = Some(when.to_string());
                }

                dag.add_job(job);
                current_group_ids.push(unique_name);
            }

            // Add edges from previous group to current group
            for current_id in &current_group_ids {
                for prev_id in &prev_group_ids {
                    let _ = dag.add_dependency(prev_id, current_id);
                }
            }

            prev_group_ids = current_group_ids;
        }

        Ok(())
    }

    fn template_to_job(template: &Value, job_id: &str) -> Result<JobNode> {
        let template_name = template
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(job_id);

        let mut job = JobNode::new(job_id.to_string(), template_name.to_string());

        // Container spec
        if let Some(container) = template.get("container") {
            let image = container
                .get("image")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            let command = container
                .get("command")
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                });

            let args = container
                .get("args")
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                });

            let run = match (command, args) {
                (Some(cmd), Some(a)) => Some(format!("{} {}", cmd, a)),
                (Some(cmd), None) => Some(cmd),
                (None, Some(a)) => Some(a),
                (None, None) => None,
            };

            job.steps.push(StepInfo {
                name: template_name.to_string(),
                uses: Some(image.to_string()),
                run,
                estimated_duration_secs: Some(Self::estimate_duration(image, template_name)),
            });
        }

        // Script spec
        if let Some(script) = template.get("script") {
            let image = script
                .get("image")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let source = script
                .get("source")
                .and_then(|v| v.as_str())
                .map(String::from);

            job.steps.push(StepInfo {
                name: template_name.to_string(),
                uses: Some(image.to_string()),
                run: source,
                estimated_duration_secs: Some(Self::estimate_duration(image, template_name)),
            });
        }

        // Retry strategy
        if let Some(retry) = template.get("retryStrategy") {
            if let Some(limit) = retry.get("limit").and_then(|v| v.as_u64()) {
                job.env.insert("retry_limit".to_string(), limit.to_string());
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

        Ok(job)
    }

    fn estimate_duration(image: &str, name: &str) -> f64 {
        let lower = format!("{} {}", image, name).to_lowercase();
        if lower.contains("build")
            || lower.contains("kaniko")
            || lower.contains("buildah")
            || lower.contains("test")
            || lower.contains("pytest")
        {
            300.0
        } else if lower.contains("lint") || lower.contains("check") {
            60.0
        } else if lower.contains("deploy") || lower.contains("kubectl") || lower.contains("helm") {
            120.0
        } else if lower.contains("clone") || lower.contains("git") {
            15.0
        } else if lower.contains("install") || lower.contains("setup") {
            120.0
        } else {
            60.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_argo_dag_workflow() {
        let yaml = r#"
apiVersion: argoproj.io/v1alpha1
kind: Workflow
metadata:
  name: ci-pipeline
spec:
  entrypoint: main
  templates:
    - name: main
      dag:
        tasks:
          - name: clone
            template: git-clone
          - name: build
            template: build-app
            dependencies: [clone]
          - name: test
            template: run-tests
            dependencies: [clone]
          - name: deploy
            template: deploy-app
            dependencies: [build, test]
    - name: git-clone
      container:
        image: alpine/git
        command: [git, clone]
    - name: build-app
      container:
        image: golang:1.21
        command: [go, build, ./...]
    - name: run-tests
      container:
        image: golang:1.21
        command: [go, test, ./...]
    - name: deploy-app
      container:
        image: bitnami/kubectl
        command: [kubectl, apply, -f, deploy.yaml]
"#;
        let dag = ArgoWorkflowsParser::parse(yaml, "workflow.yaml".into()).unwrap();
        assert_eq!(dag.provider, "argo-workflows");
        assert_eq!(dag.job_count(), 4);
        assert_eq!(dag.name, "ci-pipeline");

        let deploy = dag.get_job("deploy").unwrap();
        assert_eq!(deploy.needs.len(), 2);
    }

    #[test]
    fn test_parse_argo_steps_workflow() {
        let yaml = r#"
apiVersion: argoproj.io/v1alpha1
kind: Workflow
metadata:
  name: steps-pipeline
spec:
  entrypoint: main
  templates:
    - name: main
      steps:
        - - name: clone
            template: git-clone
        - - name: lint
            template: run-lint
          - name: test
            template: run-test
        - - name: deploy
            template: deploy-app
    - name: git-clone
      container:
        image: alpine/git
        command: [git, clone]
    - name: run-lint
      container:
        image: golangci/golangci-lint
        command: [golangci-lint, run]
    - name: run-test
      container:
        image: golang:1.21
        command: [go, test, ./...]
    - name: deploy-app
      container:
        image: bitnami/kubectl
        command: [kubectl, apply]
"#;
        let dag = ArgoWorkflowsParser::parse(yaml, "workflow.yaml".into()).unwrap();
        assert_eq!(dag.provider, "argo-workflows");
        // 4 tasks: clone, lint, test, deploy
        assert_eq!(dag.job_count(), 4);
    }

    #[test]
    fn test_parse_argo_script_template() {
        let yaml = r#"
apiVersion: argoproj.io/v1alpha1
kind: Workflow
metadata:
  name: script-workflow
spec:
  entrypoint: main
  templates:
    - name: main
      dag:
        tasks:
          - name: hello
            template: hello-script
    - name: hello-script
      script:
        image: python:3.11
        source: |
          print("hello world")
"#;
        let dag = ArgoWorkflowsParser::parse(yaml, "workflow.yaml".into()).unwrap();
        assert_eq!(dag.job_count(), 1);
        let hello = dag.get_job("hello").unwrap();
        assert_eq!(hello.steps.len(), 1);
        assert!(hello.steps[0].run.is_some());
    }

    #[test]
    fn test_parse_argo_with_conditions() {
        let yaml = r#"
apiVersion: argoproj.io/v1alpha1
kind: Workflow
metadata:
  name: conditional-pipeline
spec:
  entrypoint: main
  templates:
    - name: main
      dag:
        tasks:
          - name: build
            template: build-app
          - name: deploy-prod
            template: deploy-app
            dependencies: [build]
            when: "{{workflow.parameters.env}} == prod"
    - name: build-app
      container:
        image: golang:1.21
        command: [go, build]
    - name: deploy-app
      container:
        image: bitnami/kubectl
        command: [kubectl, apply]
"#;
        let dag = ArgoWorkflowsParser::parse(yaml, "workflow.yaml".into()).unwrap();
        assert_eq!(dag.job_count(), 2);
        let deploy = dag.get_job("deploy-prod").unwrap();
        assert!(deploy.condition.is_some());
    }

    #[test]
    fn test_parse_argo_multi_document_prefers_workflow() {
        let yaml = r#"
apiVersion: argoproj.io/v1alpha1
kind: WorkflowTemplate
metadata:
  name: shared-template
spec:
  templates:
    - name: shared
      container:
        image: alpine:3
        command: [sh, -c, "echo shared"]
---
apiVersion: argoproj.io/v1alpha1
kind: Workflow
metadata:
  name: real-workflow
spec:
  entrypoint: main
  templates:
    - name: main
      dag:
        tasks:
          - name: hello
            template: say-hello
    - name: say-hello
      container:
        image: alpine:3
        command: [sh, -c, "echo hello"]
"#;
        let dag = ArgoWorkflowsParser::parse(yaml, "workflow.yaml".into()).unwrap();
        assert_eq!(dag.provider, "argo-workflows");
        assert_eq!(dag.name, "real-workflow");
        assert_eq!(dag.job_count(), 1);
        assert!(dag.get_job("hello").is_some());
    }
}
