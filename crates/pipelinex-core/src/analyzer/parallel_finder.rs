use crate::parser::dag::PipelineDag;
use crate::analyzer::report::{Finding, FindingCategory, Severity};
use petgraph::Direction;

/// Find jobs that are serialized but could potentially run in parallel.
pub fn find_parallelization_opportunities(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    // For each job, check if its dependencies are truly necessary
    for idx in dag.graph.node_indices() {
        let job = &dag.graph[idx];

        // Get all jobs this job depends on
        let deps: Vec<_> = dag.graph.neighbors_directed(idx, Direction::Incoming).collect();

        for dep_idx in &deps {
            let dep_job = &dag.graph[*dep_idx];

            // Check if the dependency is likely a false dependency
            // Heuristic: if the dependent job doesn't use artifacts from the dependency,
            // and the dependency doesn't produce outputs the dependent needs, it may be false
            if is_likely_false_dependency(dep_job, job) {
                let savings = dep_job.estimated_duration_secs.min(job.estimated_duration_secs);
                findings.push(Finding {
                    severity: Severity::High,
                    category: FindingCategory::SerialBottleneck,
                    title: format!(
                        "'{}' depends on '{}' unnecessarily",
                        job.id, dep_job.id
                    ),
                    description: format!(
                        "Job '{}' has `needs: [{}]` but doesn't appear to use any \
                        artifacts or outputs from '{}'. These jobs could run in parallel.",
                        job.id, dep_job.id, dep_job.id,
                    ),
                    affected_jobs: vec![job.id.clone(), dep_job.id.clone()],
                    recommendation: format!(
                        "Remove '{}' from the `needs` list of '{}'. Both jobs can run \
                        concurrently, reducing total pipeline time.",
                        dep_job.id, job.id,
                    ),
                    fix_command: None,
                    estimated_savings_secs: Some(savings),
                    confidence: 0.80,
                    auto_fixable: true,
                });
            }
        }
    }

    // Check for test jobs that could be sharded
    for job in dag.graph.node_weights() {
        if is_test_job(job) && job.matrix.is_none()
            && job.estimated_duration_secs > 300.0 {
                // Test job takes >5 min and isn't sharded
                let optimal_shards = (job.estimated_duration_secs / 120.0).ceil() as usize;
                let optimal_shards = optimal_shards.clamp(2, 8);
                let savings = job.estimated_duration_secs
                    - (job.estimated_duration_secs / optimal_shards as f64);

                findings.push(Finding {
                    severity: Severity::High,
                    category: FindingCategory::SerialBottleneck,
                    title: format!(
                        "'{}' could be sharded into {} parallel jobs",
                        job.id, optimal_shards
                    ),
                    description: format!(
                        "Test job '{}' takes ~{:.0}s and runs serially. Splitting into {} \
                        parallel shards would reduce wall time significantly.",
                        job.id, job.estimated_duration_secs, optimal_shards,
                    ),
                    affected_jobs: vec![job.id.clone()],
                    recommendation: format!(
                        "Add a matrix strategy to shard tests into {} parallel jobs.",
                        optimal_shards
                    ),
                    fix_command: Some(format!(
                        "pipelinex optimize --apply shard --job {} --count {}",
                        job.id, optimal_shards
                    )),
                    estimated_savings_secs: Some(savings),
                    confidence: 0.85,
                    auto_fixable: true,
                });
            }
    }

    findings
}

/// Heuristic: detect if a dependency is likely unnecessary.
/// A dependency is likely false if:
/// - The dependency job is a lint/format check (produces no artifacts)
/// - The dependent job is a build/test that only needs source code
fn is_likely_false_dependency(dep: &crate::parser::dag::JobNode, dependent: &crate::parser::dag::JobNode) -> bool {
    let dep_type = classify_job(dep);
    let dependent_type = classify_job(dependent);

    // Lint doesn't produce artifacts that tests or builds need
    if dep_type == JobType::Lint && (dependent_type == JobType::Test || dependent_type == JobType::Build) {
        return true;
    }

    // Tests generally don't produce artifacts that builds need
    if dep_type == JobType::Test && dependent_type == JobType::Build {
        return true;
    }

    false
}

#[derive(Debug, PartialEq)]
enum JobType {
    Lint,
    Test,
    Build,
    Deploy,
    Other,
}

fn classify_job(job: &crate::parser::dag::JobNode) -> JobType {
    let name = job.id.to_lowercase();
    let job_name = job.name.to_lowercase();

    let is_lint = name.contains("lint") || name.contains("format") || name.contains("style")
        || job_name.contains("lint") || job_name.contains("format");
    let is_test = name.contains("test") || job_name.contains("test");
    let is_build = name.contains("build") || name.contains("compile") || job_name.contains("build");
    let is_deploy = name.contains("deploy") || name.contains("release") || job_name.contains("deploy");

    // Also check step contents
    let step_text: String = job.steps.iter()
        .filter_map(|s| s.run.as_ref())
        .map(|r| r.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    if is_lint || step_text.contains("eslint") || step_text.contains("clippy") || step_text.contains("prettier") {
        JobType::Lint
    } else if is_test || step_text.contains("pytest") || step_text.contains("jest") || step_text.contains("npm test") {
        JobType::Test
    } else if is_build || step_text.contains("npm run build") || step_text.contains("cargo build") {
        JobType::Build
    } else if is_deploy || step_text.contains("deploy") || step_text.contains("kubectl") {
        JobType::Deploy
    } else {
        JobType::Other
    }
}

fn is_test_job(job: &crate::parser::dag::JobNode) -> bool {
    classify_job(job) == JobType::Test
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::github::GitHubActionsParser;

    #[test]
    fn test_detect_serial_lint_test() {
        let yaml = r#"
name: CI
on: push
jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm run lint
  test:
    needs: lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm test
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let findings = find_parallelization_opportunities(&dag);
        assert!(findings.iter().any(|f| matches!(f.category, FindingCategory::SerialBottleneck)));
    }
}

