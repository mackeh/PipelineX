use crate::analyzer::report::{Finding, FindingCategory, Severity};
use crate::parser::dag::PipelineDag;

/// Audit workflow permissions for overly broad access.
pub fn audit_permissions(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Only applicable to GitHub Actions
    if dag.provider != "github-actions" {
        return findings;
    }

    // Check for write-all or broad permissions in workflow-level env
    // Since we don't parse permissions block directly yet, check steps for
    // indicators of missing or overly broad permissions
    let has_permissions_indicator = dag.graph.node_weights().any(|job| {
        job.env
            .keys()
            .any(|k| k.to_lowercase().contains("permissions"))
    });

    if !has_permissions_indicator {
        // Check what actions are used to suggest minimal permissions
        let mut needs_contents_write = false;
        let mut needs_packages_write = false;
        let mut needs_security_events_write = false;
        let mut uses_third_party_with_token = false;

        for node in dag.graph.node_weights() {
            for step in &node.steps {
                if let Some(uses) = &step.uses {
                    if uses.contains("create-release")
                        || uses.contains("upload-release-asset")
                        || uses.contains("push")
                    {
                        needs_contents_write = true;
                    }
                    if uses.contains("docker/build-push-action")
                        || uses.contains("publish-packages")
                    {
                        needs_packages_write = true;
                    }
                    if uses.contains("codeql-action/upload-sarif") {
                        needs_security_events_write = true;
                    }
                    // Third-party actions that receive GITHUB_TOKEN
                    if !uses.starts_with("actions/") && !uses.starts_with("github/") {
                        uses_third_party_with_token = true;
                    }
                }
            }
        }

        let mut suggested_perms = vec!["contents: read".to_string()];
        if needs_contents_write {
            suggested_perms[0] = "contents: write".to_string();
        }
        if needs_packages_write {
            suggested_perms.push("packages: write".to_string());
        }
        if needs_security_events_write {
            suggested_perms.push("security-events: write".to_string());
        }

        findings.push(Finding {
            severity: Severity::Medium,
            category: FindingCategory::CustomPlugin,
            title: "Missing explicit permissions block".to_string(),
            description: "Workflow does not declare a permissions block. Without explicit permissions, the GITHUB_TOKEN may have broader access than needed.".to_string(),
            affected_jobs: dag.job_ids(),
            recommendation: format!(
                "Add a permissions block to your workflow:\n  permissions:\n    {}",
                suggested_perms.join("\n    ")
            ),
            fix_command: None,
            estimated_savings_secs: None,
            confidence: 0.70,
            auto_fixable: true,
        });

        if uses_third_party_with_token {
            findings.push(Finding {
                severity: Severity::Medium,
                category: FindingCategory::CustomPlugin,
                title: "GITHUB_TOKEN exposed to third-party actions".to_string(),
                description: "Third-party actions have access to the GITHUB_TOKEN. Consider restricting token permissions to minimize risk.".to_string(),
                affected_jobs: dag.job_ids(),
                recommendation: "Pin third-party actions to full SHA commits and restrict permissions to the minimum required.".to_string(),
                fix_command: None,
                estimated_savings_secs: None,
                confidence: 0.65,
                auto_fixable: false,
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dag::{JobNode, PipelineDag, StepInfo};

    #[test]
    fn test_missing_permissions_detected() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "Checkout".into(),
            uses: Some("actions/checkout@v4".into()),
            run: None,
            estimated_duration_secs: None,
        });
        dag.add_job(job);

        let findings = audit_permissions(&dag);
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.title.contains("permissions")));
    }

    #[test]
    fn test_non_github_skipped() {
        let dag = PipelineDag::new("ci".into(), "ci.yml".into(), "gitlab-ci".into());
        let findings = audit_permissions(&dag);
        assert!(findings.is_empty());
    }
}
