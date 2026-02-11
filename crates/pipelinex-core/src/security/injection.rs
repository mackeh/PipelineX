use crate::analyzer::report::{Finding, FindingCategory, Severity};
use crate::parser::dag::PipelineDag;

/// Dangerous GitHub Actions expression contexts that can be attacker-controlled.
const DANGEROUS_CONTEXTS: &[&str] = &[
    "github.event.issue.title",
    "github.event.issue.body",
    "github.event.pull_request.title",
    "github.event.pull_request.body",
    "github.event.comment.body",
    "github.event.review.body",
    "github.event.head_commit.message",
    "github.head_ref",
    "github.event.workflow_run.head_branch",
    "github.event.discussion.title",
    "github.event.discussion.body",
];

/// Detect expression injection vulnerabilities in GitHub Actions workflows.
pub fn detect_injection(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Primary check is for GitHub Actions
    if dag.provider != "github-actions" {
        return findings;
    }

    for node in dag.graph.node_weights() {
        for step in &node.steps {
            if let Some(run) = &step.run {
                for ctx in DANGEROUS_CONTEXTS {
                    let expression = format!("${{{{ {} }}}}", ctx);
                    if run.contains(&expression) {
                        findings.push(Finding {
                            severity: Severity::Critical,
                            category: FindingCategory::CustomPlugin,
                            title: format!("Expression injection via {}", ctx),
                            description: format!(
                                "Job '{}', step '{}' uses `{}` directly in a `run:` step. \
                                 This is attacker-controlled input and can lead to arbitrary code execution.",
                                node.id, step.name, ctx
                            ),
                            affected_jobs: vec![node.id.clone()],
                            recommendation: format!(
                                "Assign to an environment variable first:\n  \
                                 env:\n    SAFE_VALUE: ${{{{ {} }}}}\n  \
                                 Then use $SAFE_VALUE in the run step.",
                                ctx
                            ),
                            fix_command: None,
                            estimated_savings_secs: None,
                            confidence: 0.95,
                            auto_fixable: false,
                        });
                    }
                }
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dag::{JobNode, PipelineDag, StepInfo};

    #[test]
    fn test_detect_title_injection() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let mut job = JobNode::new("greet".into(), "Greet".into());
        job.steps.push(StepInfo {
            name: "Echo title".into(),
            uses: None,
            run: Some("echo \"${{ github.event.issue.title }}\"".into()),
            estimated_duration_secs: None,
        });
        dag.add_job(job);

        let findings = detect_injection(&dag);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Critical);
        assert!(findings[0].title.contains("injection"));
    }

    #[test]
    fn test_safe_context_not_flagged() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "Use safe context".into(),
            uses: None,
            run: Some("echo ${{ github.sha }}".into()),
            estimated_duration_secs: None,
        });
        dag.add_job(job);

        let findings = detect_injection(&dag);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_non_github_skipped() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "gitlab-ci".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "test".into(),
            uses: None,
            run: Some("echo \"${{ github.event.issue.title }}\"".into()),
            estimated_duration_secs: None,
        });
        dag.add_job(job);

        let findings = detect_injection(&dag);
        assert!(findings.is_empty());
    }
}
