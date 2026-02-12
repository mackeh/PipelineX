//! What-if simulator engine for pipeline DAG modifications.
//!
//! Allows users to explore optimization impact by modifying the pipeline:
//! - Remove/add dependency edges
//! - Toggle caching on/off per job
//! - Enable/disable path filters
//! - Change runner types
//!
//! Then recalculate critical path, duration, and cost.

use crate::analyzer;
use crate::parser::dag::*;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A what-if scenario modification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Modification {
    /// Remove a dependency edge between two jobs.
    RemoveDependency { from: String, to: String },
    /// Add a dependency edge between two jobs.
    AddDependency { from: String, to: String },
    /// Add cache to a specific job (reduces estimated duration).
    AddCache { job_id: String, savings_secs: f64 },
    /// Remove cache from a specific job.
    RemoveCache { job_id: String },
    /// Enable path filter for a job (reduces trigger frequency).
    EnablePathFilter { job_id: String, paths: Vec<String> },
    /// Change the runner for a job.
    ChangeRunner { job_id: String, runner: String },
    /// Remove a job entirely.
    RemoveJob { job_id: String },
    /// Set a custom duration estimate for a job.
    SetDuration { job_id: String, duration_secs: f64 },
}

/// Result of a what-if simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatIfResult {
    pub original_duration_secs: f64,
    pub modified_duration_secs: f64,
    pub duration_delta_secs: f64,
    pub improvement_pct: f64,
    pub original_critical_path: Vec<String>,
    pub modified_critical_path: Vec<String>,
    pub original_job_count: usize,
    pub modified_job_count: usize,
    pub original_findings_count: usize,
    pub modified_findings_count: usize,
    pub modifications_applied: Vec<String>,
    pub warnings: Vec<String>,
}

/// Apply a set of modifications to a pipeline DAG and compute the impact.
pub fn simulate(dag: &PipelineDag, modifications: &[Modification]) -> WhatIfResult {
    let original_report = analyzer::analyze(dag);

    // Clone the DAG for modification
    let mut modified_dag = dag.clone();
    let mut applied = Vec::new();
    let mut warnings = Vec::new();

    for modification in modifications {
        match apply_modification(&mut modified_dag, modification) {
            Ok(desc) => applied.push(desc),
            Err(e) => warnings.push(format!("Skipped: {}", e)),
        }
    }

    let modified_report = analyzer::analyze(&modified_dag);

    let duration_delta = modified_report.total_estimated_duration_secs
        - original_report.total_estimated_duration_secs;
    let improvement_pct = if original_report.total_estimated_duration_secs > 0.0 {
        -duration_delta / original_report.total_estimated_duration_secs * 100.0
    } else {
        0.0
    };

    WhatIfResult {
        original_duration_secs: original_report.total_estimated_duration_secs,
        modified_duration_secs: modified_report.total_estimated_duration_secs,
        duration_delta_secs: duration_delta,
        improvement_pct,
        original_critical_path: original_report.critical_path,
        modified_critical_path: modified_report.critical_path,
        original_job_count: original_report.job_count,
        modified_job_count: modified_report.job_count,
        original_findings_count: original_report.findings.len(),
        modified_findings_count: modified_report.findings.len(),
        modifications_applied: applied,
        warnings,
    }
}

fn apply_modification(
    dag: &mut PipelineDag,
    modification: &Modification,
) -> anyhow::Result<String> {
    match modification {
        Modification::RemoveDependency { from, to } => {
            let from_idx = dag
                .node_map
                .get(from)
                .ok_or_else(|| anyhow::anyhow!("Job '{}' not found", from))?;
            let to_idx = dag
                .node_map
                .get(to)
                .ok_or_else(|| anyhow::anyhow!("Job '{}' not found", to))?;

            // Find and remove the edge
            let edge = dag
                .graph
                .find_edge(*from_idx, *to_idx)
                .ok_or_else(|| anyhow::anyhow!("No edge from '{}' to '{}'", from, to))?;

            dag.graph.remove_edge(edge);

            // Update the needs list
            if let Some(to_job) = dag.node_map.get(to).map(|idx| &mut dag.graph[*idx]) {
                to_job.needs.retain(|n| n != from);
            }

            Ok(format!("Removed dependency {} -> {}", from, to))
        }

        Modification::AddDependency { from, to } => {
            dag.add_dependency(from, to)?;

            if let Some(to_idx) = dag.node_map.get(to) {
                dag.graph[*to_idx].needs.push(from.clone());
            }

            Ok(format!("Added dependency {} -> {}", from, to))
        }

        Modification::AddCache {
            job_id,
            savings_secs,
        } => {
            let idx = dag
                .node_map
                .get(job_id)
                .ok_or_else(|| anyhow::anyhow!("Job '{}' not found", job_id))?;

            let job = &mut dag.graph[*idx];
            job.caches.push(CacheConfig {
                path: "node_modules".to_string(),
                key_pattern: "${{ hashFiles('**/package-lock.json') }}".to_string(),
                restore_keys: vec!["deps-".to_string()],
            });

            // Reduce duration by savings amount
            job.estimated_duration_secs = (job.estimated_duration_secs - savings_secs).max(10.0);

            Ok(format!(
                "Added cache to '{}' (saves {:.0}s)",
                job_id, savings_secs
            ))
        }

        Modification::RemoveCache { job_id } => {
            let idx = dag
                .node_map
                .get(job_id)
                .ok_or_else(|| anyhow::anyhow!("Job '{}' not found", job_id))?;

            let job = &mut dag.graph[*idx];
            let cache_count = job.caches.len();
            job.caches.clear();

            // Add back estimated install time
            if cache_count > 0 {
                job.estimated_duration_secs += 120.0;
            }

            Ok(format!("Removed {} caches from '{}'", cache_count, job_id))
        }

        Modification::EnablePathFilter { job_id, paths } => {
            let idx = dag
                .node_map
                .get(job_id)
                .ok_or_else(|| anyhow::anyhow!("Job '{}' not found", job_id))?;

            dag.graph[*idx].paths_filter = Some(paths.clone());

            Ok(format!("Enabled path filter on '{}': {:?}", job_id, paths))
        }

        Modification::ChangeRunner { job_id, runner } => {
            let idx = dag
                .node_map
                .get(job_id)
                .ok_or_else(|| anyhow::anyhow!("Job '{}' not found", job_id))?;

            let old_runner = dag.graph[*idx].runs_on.clone();
            dag.graph[*idx].runs_on = runner.clone();

            // Adjust duration based on runner tier
            let factor = runner_speed_factor(&old_runner, runner);
            dag.graph[*idx].estimated_duration_secs *= factor;

            Ok(format!(
                "Changed '{}' runner: {} -> {} (speed factor: {:.2}x)",
                job_id,
                old_runner,
                runner,
                1.0 / factor
            ))
        }

        Modification::RemoveJob { job_id } => {
            let idx = dag
                .node_map
                .get(job_id)
                .ok_or_else(|| anyhow::anyhow!("Job '{}' not found", job_id))?;
            let idx = *idx;

            // Reconnect: make deps of this job point to its dependents
            let incoming: Vec<_> = dag
                .graph
                .neighbors_directed(idx, Direction::Incoming)
                .collect();
            let outgoing: Vec<_> = dag
                .graph
                .neighbors_directed(idx, Direction::Outgoing)
                .collect();

            for from in &incoming {
                for to in &outgoing {
                    if !dag.graph.contains_edge(*from, *to) {
                        dag.graph.add_edge(*from, *to, DagEdge::Dependency);
                    }
                }
            }

            dag.graph.remove_node(idx);
            dag.node_map.remove(job_id);

            // Rebuild node_map since indices may have shifted
            let mut new_map = HashMap::new();
            for idx in dag.graph.node_indices() {
                new_map.insert(dag.graph[idx].id.clone(), idx);
            }
            dag.node_map = new_map;

            Ok(format!("Removed job '{}'", job_id))
        }

        Modification::SetDuration {
            job_id,
            duration_secs,
        } => {
            let idx = dag
                .node_map
                .get(job_id)
                .ok_or_else(|| anyhow::anyhow!("Job '{}' not found", job_id))?;

            let old = dag.graph[*idx].estimated_duration_secs;
            dag.graph[*idx].estimated_duration_secs = *duration_secs;

            Ok(format!(
                "Set '{}' duration: {:.0}s -> {:.0}s",
                job_id, old, duration_secs
            ))
        }
    }
}

/// Estimate speed factor when changing runners.
/// Returns a multiplier for duration (lower = faster).
fn runner_speed_factor(old: &str, new: &str) -> f64 {
    let tier = |r: &str| -> u8 {
        let lower = r.to_lowercase();
        if lower.contains("xlarge") || lower.contains("x-large") || lower.contains("16-core") {
            4
        } else if lower.contains("large") || lower.contains("8-core") {
            3
        } else if lower.contains("medium") || lower.contains("4-core") {
            2
        } else {
            1 // small / standard / default
        }
    };

    let old_tier = tier(old) as f64;
    let new_tier = tier(new) as f64;

    if new_tier == old_tier {
        1.0
    } else {
        // Approximate: doubling cores ~= 0.6x duration for parallel workloads
        (old_tier / new_tier).powf(0.7)
    }
}

/// Parse a modification from a simple string command.
/// Supports formats like:
///   "remove-dep from->to"
///   "add-cache job 120"
///   "remove-job job_id"
///   "set-duration job 300"
///   "change-runner job ubuntu-latest-16-core"
pub fn parse_modification(input: &str) -> anyhow::Result<Modification> {
    let parts: Vec<&str> = input.trim().splitn(2, ' ').collect();
    if parts.is_empty() {
        anyhow::bail!("Empty modification command");
    }

    let command = parts[0];
    let args = if parts.len() > 1 { parts[1] } else { "" };

    match command {
        "remove-dep" => {
            let edges: Vec<&str> = args.split("->").collect();
            if edges.len() != 2 {
                anyhow::bail!("Expected format: remove-dep from->to");
            }
            Ok(Modification::RemoveDependency {
                from: edges[0].trim().to_string(),
                to: edges[1].trim().to_string(),
            })
        }
        "add-dep" => {
            let edges: Vec<&str> = args.split("->").collect();
            if edges.len() != 2 {
                anyhow::bail!("Expected format: add-dep from->to");
            }
            Ok(Modification::AddDependency {
                from: edges[0].trim().to_string(),
                to: edges[1].trim().to_string(),
            })
        }
        "add-cache" => {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            let job_id = parts
                .first()
                .ok_or_else(|| anyhow::anyhow!("Expected: add-cache <job> [savings_secs]"))?
                .to_string();
            let savings = parts
                .get(1)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(120.0);
            Ok(Modification::AddCache {
                job_id,
                savings_secs: savings,
            })
        }
        "remove-cache" => Ok(Modification::RemoveCache {
            job_id: args.trim().to_string(),
        }),
        "remove-job" => Ok(Modification::RemoveJob {
            job_id: args.trim().to_string(),
        }),
        "set-duration" => {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            if parts.len() != 2 {
                anyhow::bail!("Expected format: set-duration <job> <seconds>");
            }
            Ok(Modification::SetDuration {
                job_id: parts[0].to_string(),
                duration_secs: parts[1]
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid duration"))?,
            })
        }
        "change-runner" => {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            if parts.len() != 2 {
                anyhow::bail!("Expected format: change-runner <job> <runner>");
            }
            Ok(Modification::ChangeRunner {
                job_id: parts[0].to_string(),
                runner: parts[1].to_string(),
            })
        }
        _ => anyhow::bail!("Unknown modification: '{}'. Available: remove-dep, add-dep, add-cache, remove-cache, remove-job, set-duration, change-runner", command),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dag::JobNode;

    fn create_test_dag() -> PipelineDag {
        let mut dag = PipelineDag::new("test".into(), "test.yml".into(), "github-actions".into());

        let mut checkout = JobNode::new("checkout".into(), "Checkout".into());
        checkout.estimated_duration_secs = 15.0;
        checkout.steps.push(StepInfo {
            name: "checkout".into(),
            uses: Some("actions/checkout@v4".into()),
            run: None,
            estimated_duration_secs: Some(15.0),
        });
        dag.add_job(checkout);

        let mut build = JobNode::new("build".into(), "Build".into());
        build.estimated_duration_secs = 300.0;
        build.needs = vec!["checkout".into()];
        build.steps.push(StepInfo {
            name: "build".into(),
            uses: None,
            run: Some("npm run build".into()),
            estimated_duration_secs: Some(300.0),
        });
        dag.add_job(build);

        let mut test = JobNode::new("test".into(), "Test".into());
        test.estimated_duration_secs = 300.0;
        test.needs = vec!["checkout".into()];
        test.steps.push(StepInfo {
            name: "test".into(),
            uses: None,
            run: Some("npm test".into()),
            estimated_duration_secs: Some(300.0),
        });
        dag.add_job(test);

        let mut deploy = JobNode::new("deploy".into(), "Deploy".into());
        deploy.estimated_duration_secs = 120.0;
        deploy.needs = vec!["build".into(), "test".into()];
        deploy.steps.push(StepInfo {
            name: "deploy".into(),
            uses: None,
            run: Some("deploy.sh".into()),
            estimated_duration_secs: Some(120.0),
        });
        dag.add_job(deploy);

        let _ = dag.add_dependency("checkout", "build");
        let _ = dag.add_dependency("checkout", "test");
        let _ = dag.add_dependency("build", "deploy");
        let _ = dag.add_dependency("test", "deploy");

        dag
    }

    #[test]
    fn test_simulate_no_changes() {
        let dag = create_test_dag();
        let result = simulate(&dag, &[]);
        assert_eq!(result.modifications_applied.len(), 0);
        assert_eq!(result.duration_delta_secs, 0.0);
        assert_eq!(result.original_job_count, 4);
    }

    #[test]
    fn test_simulate_add_cache() {
        let dag = create_test_dag();
        // Cache both build and test to ensure the critical path gets shorter
        let mods = vec![
            Modification::AddCache {
                job_id: "build".into(),
                savings_secs: 120.0,
            },
            Modification::AddCache {
                job_id: "test".into(),
                savings_secs: 120.0,
            },
        ];
        let result = simulate(&dag, &mods);
        assert_eq!(result.modifications_applied.len(), 2);
        assert!(result.modified_duration_secs < result.original_duration_secs);
    }

    #[test]
    fn test_simulate_remove_job() {
        let dag = create_test_dag();
        let mods = vec![Modification::RemoveJob {
            job_id: "test".into(),
        }];
        let result = simulate(&dag, &mods);
        assert_eq!(result.modified_job_count, 3);
    }

    #[test]
    fn test_simulate_set_duration() {
        let dag = create_test_dag();
        let mods = vec![Modification::SetDuration {
            job_id: "build".into(),
            duration_secs: 60.0,
        }];
        let result = simulate(&dag, &mods);
        assert!(result.modified_duration_secs <= result.original_duration_secs);
    }

    #[test]
    fn test_parse_modification_commands() {
        let m = parse_modification("remove-dep build->deploy").unwrap();
        assert!(matches!(m, Modification::RemoveDependency { .. }));

        let m = parse_modification("add-cache build 120").unwrap();
        assert!(matches!(m, Modification::AddCache { .. }));

        let m = parse_modification("remove-job test").unwrap();
        assert!(matches!(m, Modification::RemoveJob { .. }));

        let m = parse_modification("set-duration build 60").unwrap();
        assert!(matches!(m, Modification::SetDuration { .. }));

        let m = parse_modification("change-runner build ubuntu-latest-16-core").unwrap();
        assert!(matches!(m, Modification::ChangeRunner { .. }));
    }

    #[test]
    fn test_simulate_invalid_job() {
        let dag = create_test_dag();
        let mods = vec![Modification::RemoveJob {
            job_id: "nonexistent".into(),
        }];
        let result = simulate(&dag, &mods);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("nonexistent"));
    }

    #[test]
    fn test_runner_speed_factor() {
        // Same tier
        assert_eq!(runner_speed_factor("ubuntu-latest", "ubuntu-latest"), 1.0);
        // Upgrade
        let factor = runner_speed_factor("ubuntu-latest", "ubuntu-latest-16-core");
        assert!(factor < 1.0); // Should be faster
                               // Downgrade
        let factor = runner_speed_factor("ubuntu-latest-16-core", "ubuntu-latest");
        assert!(factor > 1.0); // Should be slower
    }
}
