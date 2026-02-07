use crate::parser::dag::{JobNode, PipelineDag};
use crate::analyzer::report::{Finding, FindingCategory, Severity};
use petgraph::graph::NodeIndex;
use petgraph::Direction;
use std::collections::HashMap;

/// Find the critical path through the pipeline DAG.
/// Returns the ordered list of jobs on the critical path and the total duration.
pub fn find_critical_path(dag: &PipelineDag) -> (Vec<&JobNode>, f64) {
    let graph = &dag.graph;
    let mut longest_dist: HashMap<NodeIndex, f64> = HashMap::new();
    let mut predecessor: HashMap<NodeIndex, Option<NodeIndex>> = HashMap::new();

    // Initialize all distances to the job's own duration
    for idx in graph.node_indices() {
        longest_dist.insert(idx, 0.0);
        predecessor.insert(idx, None);
    }

    // Topological sort for proper processing order
    let topo = match petgraph::algo::toposort(graph, None) {
        Ok(t) => t,
        Err(_) => return (Vec::new(), 0.0), // Cycle detected
    };

    // Forward pass: compute longest path to each node
    for &node in &topo {
        let node_duration = graph[node].estimated_duration_secs;
        let dist_to_node = longest_dist[&node] + node_duration;

        for neighbor in graph.neighbors_directed(node, Direction::Outgoing) {
            if dist_to_node > longest_dist[&neighbor] {
                longest_dist.insert(neighbor, dist_to_node);
                predecessor.insert(neighbor, Some(node));
            }
        }
    }

    // Find the leaf node with the longest total path
    let leaves = dag.leaf_jobs();
    let end_node = leaves.into_iter()
        .max_by(|a, b| {
            let da = longest_dist[a] + graph[*a].estimated_duration_secs;
            let db = longest_dist[b] + graph[*b].estimated_duration_secs;
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

    let end_node = match end_node {
        Some(n) => n,
        None => return (Vec::new(), 0.0),
    };

    let total_duration = longest_dist[&end_node] + graph[end_node].estimated_duration_secs;

    // Backtrack to build the critical path
    let mut path = vec![end_node];
    let mut current = end_node;
    while let Some(Some(pred)) = predecessor.get(&current) {
        path.push(*pred);
        current = *pred;
    }
    path.reverse();

    let critical_jobs: Vec<&JobNode> = path.iter().map(|&idx| &graph[idx]).collect();

    (critical_jobs, total_duration)
}

/// Generate findings based on critical path analysis.
pub fn analyze_critical_path(
    dag: &PipelineDag,
    critical_path: &[&JobNode],
    total_duration: f64,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    if critical_path.is_empty() {
        return findings;
    }

    // Find the single biggest bottleneck on the critical path
    if let Some(bottleneck) = critical_path.iter()
        .max_by(|a, b| a.estimated_duration_secs.partial_cmp(&b.estimated_duration_secs).unwrap())
    {
        let pct = if total_duration > 0.0 {
            bottleneck.estimated_duration_secs / total_duration * 100.0
        } else {
            0.0
        };

        if pct > 30.0 {
            findings.push(Finding {
                severity: Severity::High,
                category: FindingCategory::CriticalPath,
                title: format!("'{}' dominates the critical path ({:.1}%)", bottleneck.id, pct),
                description: format!(
                    "Job '{}' takes {:.0}s ({:.1}% of the {:.0}s critical path). \
                    This is the single biggest opportunity to reduce pipeline time.",
                    bottleneck.id,
                    bottleneck.estimated_duration_secs,
                    pct,
                    total_duration,
                ),
                affected_jobs: vec![bottleneck.id.clone()],
                recommendation: format!(
                    "Consider sharding '{}' into parallel sub-jobs, enabling caching, \
                    or optimizing the slowest steps within this job.",
                    bottleneck.id
                ),
                fix_command: Some(format!(
                    "pipelinex optimize --apply shard --job {}",
                    bottleneck.id
                )),
                estimated_savings_secs: Some(bottleneck.estimated_duration_secs * 0.5),
                confidence: 0.85,
                auto_fixable: false,
            });
        }
    }

    // Check theoretical parallelism efficiency
    let total_job_time: f64 = dag.graph.node_weights()
        .map(|j| j.estimated_duration_secs)
        .sum();
    let parallelism = dag.max_parallelism();
    let theoretical_min = total_job_time / parallelism as f64;

    if total_duration > theoretical_min * 1.5 && parallelism > 1 {
        findings.push(Finding {
            severity: Severity::Medium,
            category: FindingCategory::CriticalPath,
            title: format!(
                "Parallelism efficiency is {:.0}%",
                theoretical_min / total_duration * 100.0
            ),
            description: format!(
                "With {} parallel slots and {:.0}s of total job time, the theoretical minimum \
                is {:.0}s, but the actual critical path is {:.0}s.",
                parallelism, total_job_time, theoretical_min, total_duration,
            ),
            affected_jobs: critical_path.iter().map(|j| j.id.clone()).collect(),
            recommendation: "Review job dependencies — some may be unnecessary, \
                allowing more parallelism."
                .to_string(),
            fix_command: None,
            estimated_savings_secs: Some((total_duration - theoretical_min) * 0.3),
            confidence: 0.7,
            auto_fixable: false,
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::github::GitHubActionsParser;

    #[test]
    fn test_critical_path_linear() {
        let yaml = r#"
name: CI
on: push
jobs:
  a:
    runs-on: ubuntu-latest
    steps:
      - run: echo a
  b:
    needs: a
    runs-on: ubuntu-latest
    steps:
      - run: echo b
  c:
    needs: b
    runs-on: ubuntu-latest
    steps:
      - run: echo c
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let (path, _duration) = find_critical_path(&dag);
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].id, "a");
        assert_eq!(path[1].id, "b");
        assert_eq!(path[2].id, "c");
    }
}
