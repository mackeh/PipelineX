use crate::parser::dag::PipelineDag;
use crate::analyzer::report::format_duration;
use petgraph::Direction;

/// Generate a Mermaid flowchart diagram from a Pipeline DAG.
pub fn to_mermaid(dag: &PipelineDag) -> String {
    let mut lines = Vec::new();
    lines.push("graph LR".to_string());

    // Add nodes with timing labels
    for idx in dag.graph.node_indices() {
        let job = &dag.graph[idx];
        let duration = format_duration(job.estimated_duration_secs);
        let label = format!("{}\\n{}", job.name, duration);
        lines.push(format!("    {}[\"{}\"]\n", job.id, label));
    }

    // Add edges
    for edge in dag.graph.edge_indices() {
        let (source, target) = dag.graph.edge_endpoints(edge).unwrap();
        let source_id = &dag.graph[source].id;
        let target_id = &dag.graph[target].id;
        lines.push(format!("    {} --> {}", source_id, target_id));
    }

    // Style root nodes green, leaf nodes blue
    let roots = dag.root_jobs();
    let leaves = dag.leaf_jobs();

    if !roots.is_empty() {
        let root_ids: Vec<String> = roots.iter().map(|&idx| dag.graph[idx].id.clone()).collect();
        lines.push(format!(
            "    style {} fill:#22c55e,color:#fff",
            root_ids.join(",")
        ));
    }
    if !leaves.is_empty() {
        let leaf_ids: Vec<String> = leaves.iter().map(|&idx| dag.graph[idx].id.clone()).collect();
        lines.push(format!(
            "    style {} fill:#3b82f6,color:#fff",
            leaf_ids.join(",")
        ));
    }

    lines.join("\n")
}

/// Generate a DOT (Graphviz) representation of the Pipeline DAG.
pub fn to_dot(dag: &PipelineDag) -> String {
    let mut lines = Vec::new();
    lines.push(format!("digraph \"{}\" {{", dag.name));
    lines.push("    rankdir=LR;".to_string());
    lines.push("    node [shape=box, style=\"rounded,filled\", fontname=\"Helvetica\"];".to_string());
    lines.push("    edge [color=\"#666666\"];".to_string());
    lines.push(String::new());

    let roots = dag.root_jobs();
    let leaves = dag.leaf_jobs();

    for idx in dag.graph.node_indices() {
        let job = &dag.graph[idx];
        let duration = format_duration(job.estimated_duration_secs);
        let label = format!("{}\\n{}", job.name, duration);

        let color = if roots.contains(&idx) {
            "#22c55e"
        } else if leaves.contains(&idx) {
            "#3b82f6"
        } else {
            "#f59e0b"
        };

        let font_color = "#ffffff";
        lines.push(format!(
            "    {} [label=\"{}\", fillcolor=\"{}\", fontcolor=\"{}\"];",
            job.id, label, color, font_color
        ));
    }

    lines.push(String::new());

    for edge in dag.graph.edge_indices() {
        let (source, target) = dag.graph.edge_endpoints(edge).unwrap();
        lines.push(format!(
            "    {} -> {};",
            dag.graph[source].id, dag.graph[target].id
        ));
    }

    lines.push("}".to_string());
    lines.join("\n")
}

/// Generate an ASCII art representation of the Pipeline DAG.
pub fn to_ascii(dag: &PipelineDag) -> String {
    let mut lines = Vec::new();
    let topo = match petgraph::algo::toposort(&dag.graph, None) {
        Ok(t) => t,
        Err(_) => return "Error: cycle detected in DAG".to_string(),
    };

    // Compute levels
    let mut levels: std::collections::HashMap<petgraph::graph::NodeIndex, usize> = std::collections::HashMap::new();
    for &node in &topo {
        let deps: Vec<_> = dag.graph.neighbors_directed(node, Direction::Incoming).collect();
        let level = deps.iter()
            .map(|d| levels.get(d).copied().unwrap_or(0) + 1)
            .max()
            .unwrap_or(0);
        levels.insert(node, level);
    }

    // Group by level
    let max_level = levels.values().copied().max().unwrap_or(0);
    let mut level_jobs: Vec<Vec<petgraph::graph::NodeIndex>> = vec![Vec::new(); max_level + 1];
    for (&node, &level) in &levels {
        level_jobs[level].push(node);
    }

    lines.push(format!(
        "Pipeline: {} ({} jobs, {} levels)",
        dag.name,
        dag.job_count(),
        max_level + 1
    ));
    lines.push("=".repeat(60));
    lines.push(String::new());

    for (level, jobs) in level_jobs.iter().enumerate() {
        let prefix = if level == 0 { "START" } else { &format!("L{}", level) };
        let job_strs: Vec<String> = jobs.iter().map(|&idx| {
            let job = &dag.graph[idx];
            let duration = format_duration(job.estimated_duration_secs);
            format!("[{} ({})]", job.id, duration)
        }).collect();

        if jobs.len() > 1 {
            lines.push(format!("  {:>5} ─┬─ {}", prefix, job_strs[0]));
            for (i, js) in job_strs[1..].iter().enumerate() {
                if i == job_strs.len() - 2 {
                    lines.push(format!("         └─ {}", js));
                } else {
                    lines.push(format!("         ├─ {}", js));
                }
            }
        } else if let Some(js) = job_strs.first() {
            lines.push(format!("  {:>5} ── {}", prefix, js));
        }

        if level < max_level {
            lines.push("         │".to_string());
        }
    }

    lines.push(String::new());

    // Critical path summary
    let total: f64 = dag.graph.node_weights()
        .map(|j| j.estimated_duration_secs)
        .sum();
    lines.push(format!("Total job time: {}", format_duration(total)));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::github::GitHubActionsParser;

    #[test]
    fn test_mermaid_output() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: npm run build
  test:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - run: npm test
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let mermaid = to_mermaid(&dag);
        assert!(mermaid.contains("graph LR"));
        assert!(mermaid.contains("build"));
        assert!(mermaid.contains("test"));
        assert!(mermaid.contains("-->"));
    }

    #[test]
    fn test_dot_output() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: npm run build
  deploy:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - run: ./deploy.sh
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let dot = to_dot(&dag);
        assert!(dot.contains("digraph"));
        assert!(dot.contains("build -> deploy"));
    }
}
