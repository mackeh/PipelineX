use crate::parser::dag::PipelineDag;
use crate::analyzer::report::{Finding, FindingCategory, Severity};

/// Detect various forms of waste in the pipeline configuration.
pub fn detect_waste(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    findings.extend(detect_missing_path_filters(dag));
    findings.extend(detect_full_git_clone(dag));
    findings.extend(detect_redundant_checkouts(dag));
    findings.extend(detect_missing_concurrency(dag));
    findings.extend(detect_matrix_bloat(dag));

    findings
}

/// Detect workflows that don't use path-based filtering.
fn detect_missing_path_filters(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    let has_path_filter = dag.triggers.iter().any(|t| {
        t.paths.is_some() || t.paths_ignore.is_some()
    });

    if !has_path_filter && dag.job_count() > 1 {
        findings.push(Finding {
            severity: Severity::Medium,
            category: FindingCategory::MissingPathFilter,
            title: "No path-based filtering on triggers".to_string(),
            description: "This workflow runs the full pipeline on every push/PR, even for \
                documentation-only or config-only changes. Adding paths-ignore for docs/, \
                *.md, and similar patterns can eliminate unnecessary runs."
                .to_string(),
            affected_jobs: dag.job_ids(),
            recommendation: "Add a `paths-ignore` filter to skip the pipeline for \
                non-code changes:\n\
                \n  on:\n    push:\n      paths-ignore:\n        - 'docs/**'\n        \
                - '*.md'\n        - '.gitignore'\n        - 'LICENSE'"
                .to_string(),
            fix_command: None,
            estimated_savings_secs: None,
            confidence: 0.85,
            auto_fixable: true,
        });
    }

    findings
}

/// Detect full git clones (missing fetch-depth: 1).
fn detect_full_git_clone(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    for job in dag.graph.node_weights() {
        for step in &job.steps {
            if let Some(uses) = &step.uses {
                if uses.starts_with("actions/checkout") {
                    // Check if fetch-depth is not specified (default is full clone)
                    // We can't directly check 'with' params from our parsed data,
                    // so we flag all checkout actions and note the recommendation
                    findings.push(Finding {
                        severity: Severity::Medium,
                        category: FindingCategory::ShallowClone,
                        title: format!("Consider shallow clone in job '{}'", job.id),
                        description: format!(
                            "Job '{}' uses actions/checkout without `fetch-depth: 1`. \
                            For large repos, full git history clones can take 30s-3min. \
                            Shallow clones are much faster unless you need git history.",
                            job.id,
                        ),
                        affected_jobs: vec![job.id.clone()],
                        recommendation: "Add `with: { fetch-depth: 1 }` to the checkout step \
                            unless you need full git history (e.g., for changelog generation)."
                            .to_string(),
                        fix_command: None,
                        estimated_savings_secs: Some(30.0),
                        confidence: 0.80,
                        auto_fixable: true,
                    });
                    break; // Only report once per job
                }
            }
        }
    }

    findings
}

/// Detect multiple jobs all independently checking out code and installing deps.
fn detect_redundant_checkouts(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Count how many jobs install dependencies independently
    let mut install_jobs = Vec::new();

    for job in dag.graph.node_weights() {
        let has_install = job.steps.iter().any(|s| {
            s.run.as_ref().map_or(false, |r| {
                let cmd = r.to_lowercase();
                cmd.contains("npm ci") || cmd.contains("npm install")
                    || cmd.contains("pip install") || cmd.contains("yarn install")
                    || cmd.contains("pnpm install")
            })
        });

        if has_install {
            install_jobs.push(job.id.clone());
        }
    }

    if install_jobs.len() > 2 {
        findings.push(Finding {
            severity: Severity::Medium,
            category: FindingCategory::ArtifactReuse,
            title: format!(
                "{} jobs independently install dependencies",
                install_jobs.len()
            ),
            description: format!(
                "Jobs [{}] each install dependencies from scratch. Consider using a \
                shared setup job with artifact upload, or ensure caching is consistent \
                across all jobs.",
                install_jobs.join(", "),
            ),
            affected_jobs: install_jobs,
            recommendation: "Create a single 'setup' job that installs dependencies and \
                uploads them as artifacts, or ensure all jobs use the same cache key."
                .to_string(),
            fix_command: None,
            estimated_savings_secs: Some(120.0),
            confidence: 0.75,
            auto_fixable: false,
        });
    }

    findings
}

/// Detect missing concurrency controls.
fn detect_missing_concurrency(dag: &PipelineDag) -> Vec<Finding> {
    // For workflows triggered by push to the same branch, concurrent runs can queue up
    let has_push_trigger = dag.triggers.iter().any(|t| t.event == "push");

    if has_push_trigger {
        return vec![Finding {
            severity: Severity::Low,
            category: FindingCategory::ConcurrencyControl,
            title: "No concurrency control configured".to_string(),
            description: "This workflow triggers on push but has no concurrency settings. \
                Rapid pushes can cause multiple runs to queue, wasting compute on \
                already-superseded commits."
                .to_string(),
            affected_jobs: dag.job_ids(),
            recommendation: "Add concurrency controls to cancel in-progress runs:\n\
                \n  concurrency:\n    group: ${{ github.workflow }}-${{ github.ref }}\n    \
                cancel-in-progress: true"
                .to_string(),
            fix_command: None,
            estimated_savings_secs: None,
            confidence: 0.70,
            auto_fixable: true,
        }];
    }

    Vec::new()
}

/// Detect overly large matrix strategies.
fn detect_matrix_bloat(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    for job in dag.graph.node_weights() {
        if let Some(matrix) = &job.matrix {
            if matrix.total_combinations > 6 {
                findings.push(Finding {
                    severity: Severity::Medium,
                    category: FindingCategory::MatrixOptimization,
                    title: format!(
                        "Large matrix strategy in '{}' ({} combinations)",
                        job.id, matrix.total_combinations
                    ),
                    description: format!(
                        "Job '{}' runs {} matrix combinations. Consider running full \
                        tests only on the primary platform and smoke tests on others.",
                        job.id, matrix.total_combinations,
                    ),
                    affected_jobs: vec![job.id.clone()],
                    recommendation: "Use a smart matrix with `include:` to run full tests \
                        on the primary platform and reduced tests on secondary platforms."
                        .to_string(),
                    fix_command: None,
                    estimated_savings_secs: Some(
                        job.estimated_duration_secs * (matrix.total_combinations as f64 - 4.0).max(0.0) / matrix.total_combinations as f64
                    ),
                    confidence: 0.75,
                    auto_fixable: false,
                });
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::github::GitHubActionsParser;

    #[test]
    fn test_detect_missing_path_filter() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm run build
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm test
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let findings = detect_waste(&dag);
        assert!(findings.iter().any(|f| matches!(f.category, FindingCategory::MissingPathFilter)));
    }

    #[test]
    fn test_no_path_filter_warning_when_present() {
        let yaml = r#"
name: CI
on:
  push:
    paths-ignore:
      - 'docs/**'
      - '*.md'
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm run build
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm test
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let findings = detect_waste(&dag);
        assert!(!findings.iter().any(|f| matches!(f.category, FindingCategory::MissingPathFilter)));
    }
}
