use crate::parser::dag::*;
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// Parser for AWS CodePipeline definitions (JSON or YAML).
///
/// Supported constructs:
/// - stages
/// - actions
/// - runOrder-based dependencies
/// - input/output artifacts
pub struct AwsCodePipelineParser;

impl AwsCodePipelineParser {
    /// Parse an AWS CodePipeline file.
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read AWS CodePipeline file: {}", path.display()))?;
        Self::parse(&content, path.to_string_lossy().to_string())
    }

    /// Parse AWS CodePipeline content into a Pipeline DAG.
    ///
    /// Note: We use `serde_yaml::Value` intentionally because YAML parser can also decode JSON.
    pub fn parse(content: &str, source_file: String) -> Result<PipelineDag> {
        let parsed: Value =
            serde_yaml::from_str(content).context("Failed to parse pipeline data")?;
        let pipeline = parsed.get("pipeline").unwrap_or(&parsed);

        let name = pipeline
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("AWS CodePipeline")
            .to_string();

        let mut dag = PipelineDag::new(name, source_file, "aws-codepipeline".to_string());

        let stages = pipeline
            .get("stages")
            .and_then(|v| v.as_sequence())
            .context("No 'stages' found in AWS CodePipeline definition")?;

        let mut stage_actions: Vec<StageActions> = Vec::new();

        for (stage_idx, stage_value) in stages.iter().enumerate() {
            let stage_name = stage_value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("stage")
                .to_string();

            let actions = stage_value
                .get("actions")
                .and_then(|v| v.as_sequence())
                .context("Stage missing 'actions' sequence")?;

            let mut run_order_groups: BTreeMap<u32, Vec<String>> = BTreeMap::new();
            let mut all_ids = Vec::new();

            for (action_idx, action_value) in actions.iter().enumerate() {
                let action_name = action_value
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("action")
                    .to_string();
                let action_id =
                    format!("{}-{}", sanitize_id(&stage_name), sanitize_id(&action_name));

                let run_order = action_value
                    .get("runOrder")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as u32;
                run_order_groups
                    .entry(run_order)
                    .or_default()
                    .push(action_id.clone());

                let job = Self::parse_action(
                    &action_id,
                    &action_name,
                    &stage_name,
                    action_idx,
                    action_value,
                );
                dag.add_job(job);
                all_ids.push(action_id);
            }

            let _ = stage_idx;
            stage_actions.push(StageActions {
                all_ids,
                run_order_groups,
            });
        }

        // Dependencies:
        // 1) Inside a stage, actions with higher runOrder depend on all lower runOrder actions.
        // 2) Stage N+1 actions depend on all actions in stage N.
        for stage in &stage_actions {
            let sorted_orders: Vec<u32> = stage.run_order_groups.keys().copied().collect();
            for order in &sorted_orders {
                let Some(current_ids) = stage.run_order_groups.get(order) else {
                    continue;
                };
                let lower_orders: Vec<u32> = sorted_orders
                    .iter()
                    .copied()
                    .filter(|candidate| candidate < order)
                    .collect();
                for lower in lower_orders {
                    if let Some(lower_ids) = stage.run_order_groups.get(&lower) {
                        for lower_id in lower_ids {
                            for current_id in current_ids {
                                let _ = dag.add_dependency(lower_id, current_id);
                                if let Some(idx) = dag.node_map.get(current_id).copied() {
                                    if !dag.graph[idx].needs.contains(lower_id) {
                                        dag.graph[idx].needs.push(lower_id.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for idx in 1..stage_actions.len() {
            let prev = &stage_actions[idx - 1];
            let current = &stage_actions[idx];
            for prev_id in &prev.all_ids {
                for current_id in &current.all_ids {
                    let _ = dag.add_dependency(prev_id, current_id);
                    if let Some(node_idx) = dag.node_map.get(current_id).copied() {
                        if !dag.graph[node_idx].needs.contains(prev_id) {
                            dag.graph[node_idx].needs.push(prev_id.clone());
                        }
                    }
                }
            }
        }

        Ok(dag)
    }

    fn parse_action(
        id: &str,
        action_name: &str,
        stage_name: &str,
        action_idx: usize,
        action: &Value,
    ) -> JobNode {
        let action_type = action.get("actionTypeId");
        let category = action_type
            .and_then(|v| v.get("category"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let provider = action_type
            .and_then(|v| v.get("provider"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let owner = action_type
            .and_then(|v| v.get("owner"))
            .and_then(|v| v.as_str())
            .unwrap_or("AWS");

        let input_artifacts = parse_artifacts(action.get("inputArtifacts"));
        let output_artifacts = parse_artifacts(action.get("outputArtifacts"));

        let mut job = JobNode::new(id.to_string(), action_name.to_string());
        job.runs_on = format!("aws:{}:{}", category.to_lowercase(), provider);
        job.env
            .insert("__stage".to_string(), stage_name.to_string());
        job.env
            .insert("__category".to_string(), category.to_string());
        job.env
            .insert("__provider".to_string(), provider.to_string());
        job.env.insert("__owner".to_string(), owner.to_string());
        job.env
            .insert("__action_index".to_string(), (action_idx + 1).to_string());

        if !input_artifacts.is_empty() {
            job.env
                .insert("input_artifacts".to_string(), input_artifacts.join(","));
        }
        if !output_artifacts.is_empty() {
            job.env
                .insert("output_artifacts".to_string(), output_artifacts.join(","));
        }

        if !input_artifacts.is_empty() || !output_artifacts.is_empty() {
            job.caches.push(CacheConfig {
                path: "artifacts".to_string(),
                key_pattern: format!("{}:{}", stage_name, action_name),
                restore_keys: Vec::new(),
            });
        }

        let step_run = format!(
            "{} action via {} (inputs: {}; outputs: {})",
            category,
            provider,
            if input_artifacts.is_empty() {
                "none".to_string()
            } else {
                input_artifacts.join("|")
            },
            if output_artifacts.is_empty() {
                "none".to_string()
            } else {
                output_artifacts.join("|")
            }
        );

        job.steps.push(StepInfo {
            name: format!("{} {}", category, action_name),
            uses: Some(format!("{}::{}", owner, provider)),
            run: Some(step_run),
            estimated_duration_secs: Some(estimate_action_duration(category, provider)),
        });

        job.estimated_duration_secs = job
            .steps
            .iter()
            .filter_map(|step| step.estimated_duration_secs)
            .sum::<f64>()
            .max(20.0);

        job
    }
}

struct StageActions {
    all_ids: Vec<String>,
    run_order_groups: BTreeMap<u32, Vec<String>>,
}

fn parse_artifacts(value: Option<&Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };

    let Some(seq) = value.as_sequence() else {
        return Vec::new();
    };

    seq.iter()
        .filter_map(|artifact| {
            artifact
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| artifact.as_str().map(String::from))
        })
        .collect()
}

fn estimate_action_duration(category: &str, provider: &str) -> f64 {
    let category = category.to_lowercase();
    let provider = provider.to_lowercase();

    if provider.contains("codebuild") {
        return 300.0;
    }
    if provider.contains("codedeploy") {
        return 180.0;
    }
    if provider.contains("lambda") {
        return 80.0;
    }

    match category.as_str() {
        "source" => 90.0,
        "build" => 280.0,
        "test" => 220.0,
        "deploy" => 180.0,
        "approval" => 60.0,
        "invoke" => 100.0,
        _ => 120.0,
    }
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
    fn test_parse_stage_action_dependencies() {
        let config = r#"
{
  "pipeline": {
    "name": "SamplePipeline",
    "stages": [
      {
        "name": "Source",
        "actions": [
          {
            "name": "SourceAction",
            "actionTypeId": { "category": "Source", "owner": "AWS", "provider": "CodeCommit", "version": "1" },
            "outputArtifacts": [{ "name": "SourceOutput" }],
            "runOrder": 1
          }
        ]
      },
      {
        "name": "Build",
        "actions": [
          {
            "name": "BuildA",
            "actionTypeId": { "category": "Build", "owner": "AWS", "provider": "CodeBuild", "version": "1" },
            "inputArtifacts": [{ "name": "SourceOutput" }],
            "outputArtifacts": [{ "name": "BuildOutputA" }],
            "runOrder": 1
          },
          {
            "name": "BuildB",
            "actionTypeId": { "category": "Build", "owner": "AWS", "provider": "CodeBuild", "version": "1" },
            "inputArtifacts": [{ "name": "BuildOutputA" }],
            "runOrder": 2
          }
        ]
      }
    ]
  }
}
"#;

        let dag = AwsCodePipelineParser::parse(config, "pipeline.json".to_string()).unwrap();
        assert_eq!(dag.provider, "aws-codepipeline");
        assert_eq!(dag.job_count(), 3);

        let build_b = dag.get_job("build-buildb").unwrap();
        assert!(build_b.needs.contains(&"build-builda".to_string()));
        assert!(build_b.needs.contains(&"source-sourceaction".to_string()));
    }

    #[test]
    fn test_parse_artifact_metadata() {
        let config = r#"
pipeline:
  name: ArtifactPipe
  stages:
    - name: Source
      actions:
        - name: Fetch
          actionTypeId:
            category: Source
            provider: S3
            owner: AWS
          outputArtifacts:
            - name: Src
"#;

        let dag = AwsCodePipelineParser::parse(config, "codepipeline.yml".to_string()).unwrap();
        let source = dag.get_job("source-fetch").unwrap();
        assert!(source.runs_on.to_lowercase().contains("aws:source:s3"));
        assert!(source
            .env
            .get("output_artifacts")
            .is_some_and(|v| v.contains("Src")));
    }
}
