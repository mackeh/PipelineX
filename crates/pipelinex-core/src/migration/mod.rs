use crate::parser::dag::{MatrixStrategy, PipelineDag, WorkflowTrigger};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;

/// Output of a provider migration operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub source_provider: String,
    pub target_provider: String,
    pub converted_jobs: usize,
    pub warnings: Vec<String>,
    pub yaml: String,
}

/// Convert a GitHub Actions DAG into a GitLab CI YAML file.
pub fn github_actions_to_gitlab_ci(dag: &PipelineDag) -> Result<MigrationResult> {
    if dag.provider != "github-actions" {
        bail!(
            "GitHub Actions migration expects provider 'github-actions', got '{}'",
            dag.provider
        );
    }

    let mut warnings = Vec::new();
    let yaml = render_gitlab_yaml(dag, &mut warnings)?;

    Ok(MigrationResult {
        source_provider: dag.provider.clone(),
        target_provider: "gitlab-ci".to_string(),
        converted_jobs: dag.job_count(),
        warnings,
        yaml,
    })
}

fn render_gitlab_yaml(dag: &PipelineDag, warnings: &mut Vec<String>) -> Result<String> {
    let stage_by_job = compute_stage_indexes(dag);
    let max_stage = stage_by_job.values().copied().max().unwrap_or(0);

    let mut root = Mapping::new();
    root.insert(
        Value::String("stages".to_string()),
        Value::Sequence(
            (0..=max_stage)
                .map(|idx| Value::String(format!("stage_{}", idx + 1)))
                .collect(),
        ),
    );

    if !dag.env.is_empty() {
        root.insert(
            Value::String("variables".to_string()),
            to_string_map_value(&dag.env),
        );
    }

    if let Some(workflow) = convert_workflow_triggers(&dag.triggers, warnings) {
        root.insert(Value::String("workflow".to_string()), workflow);
    }

    let mut jobs: Vec<_> = dag.graph.node_weights().collect();
    jobs.sort_by(|a, b| {
        let stage_a = stage_by_job.get(&a.id).copied().unwrap_or(0);
        let stage_b = stage_by_job.get(&b.id).copied().unwrap_or(0);
        stage_a.cmp(&stage_b).then(a.id.cmp(&b.id))
    });

    let mut default_image: Option<String> = None;
    for job in jobs {
        let mut job_map = Mapping::new();
        let stage_idx = stage_by_job.get(&job.id).copied().unwrap_or(0);
        job_map.insert(
            Value::String("stage".to_string()),
            Value::String(format!("stage_{}", stage_idx + 1)),
        );

        if let Some(image) = infer_gitlab_image(&job.runs_on) {
            default_image = default_image.or_else(|| Some(image.to_string()));
        } else if job.runs_on.contains("windows") || job.runs_on.contains("macos") {
            warnings.push(format!(
                "Job '{}' runs on '{}' which has no direct GitLab shared-runner image mapping",
                job.id, job.runs_on
            ));
        }

        let script_lines = convert_steps_to_script(&job.id, &job.steps, warnings);
        job_map.insert(
            Value::String("script".to_string()),
            Value::Sequence(script_lines.into_iter().map(Value::String).collect()),
        );

        if !job.needs.is_empty() {
            job_map.insert(
                Value::String("needs".to_string()),
                Value::Sequence(
                    job.needs
                        .iter()
                        .map(|dep| Value::String(dep.clone()))
                        .collect(),
                ),
            );
        }

        if !job.env.is_empty() {
            job_map.insert(
                Value::String("variables".to_string()),
                to_string_map_value(&job.env),
            );
        }

        if let Some(matrix) = &job.matrix {
            if let Some(parallel_matrix) = convert_matrix(matrix) {
                job_map.insert(Value::String("parallel".to_string()), parallel_matrix);
            }
        }

        if let Some(condition) = &job.condition {
            warnings.push(format!(
                "Job '{}' uses GitHub condition '{}'; review and translate to GitLab 'rules:' manually",
                job.id, condition
            ));
        }

        root.insert(Value::String(job.id.clone()), Value::Mapping(job_map));
    }

    if let Some(image) = default_image {
        let mut default_map = Mapping::new();
        default_map.insert(Value::String("image".to_string()), Value::String(image));
        root.insert(
            Value::String("default".to_string()),
            Value::Mapping(default_map),
        );
    }

    let yaml = serde_yaml::to_string(&root)?;
    Ok(yaml)
}

fn compute_stage_indexes(dag: &PipelineDag) -> HashMap<String, usize> {
    fn visit(job_id: &str, dag: &PipelineDag, memo: &mut HashMap<String, usize>) -> usize {
        if let Some(depth) = memo.get(job_id) {
            return *depth;
        }

        let Some(job) = dag.get_job(job_id) else {
            return 0;
        };

        if job.needs.is_empty() {
            memo.insert(job_id.to_string(), 0);
            return 0;
        }

        let parent_depth = job
            .needs
            .iter()
            .map(|dep| visit(dep, dag, memo))
            .max()
            .unwrap_or(0);
        let depth = parent_depth + 1;
        memo.insert(job_id.to_string(), depth);
        depth
    }

    let mut memo = HashMap::new();
    for job_id in dag.job_ids() {
        let _ = visit(&job_id, dag, &mut memo);
    }
    memo
}

fn convert_steps_to_script(
    job_id: &str,
    steps: &[crate::parser::dag::StepInfo],
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let mut script = Vec::new();
    for step in steps {
        if let Some(run) = &step.run {
            for line in run.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    script.push(trimmed.to_string());
                }
            }
            continue;
        }

        if let Some(uses) = &step.uses {
            if uses.starts_with("actions/checkout@") {
                script.push("echo \"Repository checkout is built into GitLab CI\"".to_string());
            } else {
                warnings.push(format!(
                    "Job '{}' step '{}' uses action '{}' and needs manual porting",
                    job_id, step.name, uses
                ));
                script.push(format!(
                    "echo \"TODO: port GitHub Action {} ({})\"",
                    uses, step.name
                ));
            }
        }
    }

    if script.is_empty() {
        script
            .push("echo \"No executable shell steps were detected in the source workflow\"".into());
    }

    script
}

fn convert_matrix(matrix: &MatrixStrategy) -> Option<Value> {
    if matrix.variables.is_empty() {
        return None;
    }

    let mut entry = Mapping::new();
    for (key, values) in &matrix.variables {
        let variable_name = key
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect::<String>();

        if values.is_empty() {
            continue;
        }

        entry.insert(
            Value::String(variable_name),
            Value::Sequence(values.iter().map(|v| Value::String(v.clone())).collect()),
        );
    }

    if entry.is_empty() {
        return None;
    }

    let mut parallel = Mapping::new();
    parallel.insert(
        Value::String("matrix".to_string()),
        Value::Sequence(vec![Value::Mapping(entry)]),
    );
    Some(Value::Mapping(parallel))
}

fn convert_workflow_triggers(
    triggers: &[WorkflowTrigger],
    warnings: &mut Vec<String>,
) -> Option<Value> {
    if triggers.is_empty() {
        return None;
    }

    let mut rules = Vec::new();
    for trigger in triggers {
        let mut rule = Mapping::new();
        let maybe_if = match trigger.event.as_str() {
            "push" => Some("$CI_PIPELINE_SOURCE == \"push\""),
            "pull_request" => Some("$CI_PIPELINE_SOURCE == \"merge_request_event\""),
            "workflow_dispatch" => Some("$CI_PIPELINE_SOURCE == \"web\""),
            "schedule" => Some("$CI_PIPELINE_SOURCE == \"schedule\""),
            other => {
                warnings.push(format!(
                    "Workflow trigger '{}' has no direct GitLab mapping and was skipped",
                    other
                ));
                None
            }
        };

        if let Some(expr) = maybe_if {
            rule.insert(
                Value::String("if".to_string()),
                Value::String(expr.to_string()),
            );
            rules.push(Value::Mapping(rule));
        }
    }

    if rules.is_empty() {
        return None;
    }

    let mut deny_other = Mapping::new();
    deny_other.insert(
        Value::String("when".to_string()),
        Value::String("never".to_string()),
    );
    rules.push(Value::Mapping(deny_other));

    let mut workflow = Mapping::new();
    workflow.insert(Value::String("rules".to_string()), Value::Sequence(rules));
    Some(Value::Mapping(workflow))
}

fn to_string_map_value(data: &HashMap<String, String>) -> Value {
    let mut map = Mapping::new();
    let mut entries = data.iter().collect::<Vec<_>>();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    for (key, value) in entries {
        map.insert(
            Value::String((*key).clone()),
            Value::String((*value).clone()),
        );
    }
    Value::Mapping(map)
}

fn infer_gitlab_image(runs_on: &str) -> Option<&'static str> {
    let lower = runs_on.to_lowercase();
    if lower.contains("ubuntu") || lower.contains("linux") {
        Some("ubuntu:22.04")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dag::{JobNode, StepInfo};
    use crate::GitHubActionsParser;

    #[test]
    fn migrates_basic_github_actions_workflow() {
        let workflow = r#"
name: CI
on:
  push:
  pull_request:
jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo fmt --all -- --check
  test:
    needs: lint
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, beta]
    steps:
      - run: cargo test --all
"#;

        let dag = GitHubActionsParser::parse(workflow, "ci.yml".to_string()).unwrap();
        let result = github_actions_to_gitlab_ci(&dag).unwrap();

        assert_eq!(result.target_provider, "gitlab-ci");
        assert_eq!(result.converted_jobs, 2);
        assert!(result.yaml.contains("stages:"));
        assert!(result.yaml.contains("lint:"));
        assert!(result.yaml.contains("test:"));

        let parsed: Value = serde_yaml::from_str(&result.yaml).unwrap();
        let test_job = parsed.get("test").unwrap().as_mapping().unwrap();
        assert!(test_job.get(Value::String("needs".to_string())).is_some());
        assert!(test_job
            .get(Value::String("parallel".to_string()))
            .is_some());
    }

    #[test]
    fn fails_for_non_github_provider() {
        let dag = PipelineDag::new(
            "ci".to_string(),
            ".gitlab-ci.yml".to_string(),
            "gitlab-ci".to_string(),
        );
        let err = github_actions_to_gitlab_ci(&dag).unwrap_err();
        assert!(err
            .to_string()
            .contains("expects provider 'github-actions'"));
    }

    #[test]
    fn emits_warning_for_unsupported_uses_steps() {
        let mut dag = PipelineDag::new(
            "ci".to_string(),
            "ci.yml".to_string(),
            "github-actions".to_string(),
        );
        let mut job = JobNode::new("build".to_string(), "build".to_string());
        job.steps.push(StepInfo {
            name: "Setup Node".to_string(),
            uses: Some("actions/setup-node@v4".to_string()),
            run: None,
            estimated_duration_secs: Some(5.0),
        });
        dag.add_job(job);

        let result = github_actions_to_gitlab_ci(&dag).unwrap();
        assert!(!result.warnings.is_empty());
        assert!(result
            .yaml
            .contains("TODO: port GitHub Action actions/setup-node@v4"));
    }
}
