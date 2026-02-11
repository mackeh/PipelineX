use super::{LintFinding, LintSeverity};
use crate::parser::dag::PipelineDag;

struct DeprecationRule {
    pattern: &'static str,
    message: &'static str,
    suggestion: &'static str,
    severity: LintSeverity,
}

const GITHUB_DEPRECATIONS: &[DeprecationRule] = &[
    DeprecationRule {
        pattern: "actions/checkout@v2",
        message: "actions/checkout@v2 is deprecated",
        suggestion: "Upgrade to actions/checkout@v4",
        severity: LintSeverity::Warning,
    },
    DeprecationRule {
        pattern: "actions/checkout@v3",
        message: "actions/checkout@v3 is outdated",
        suggestion: "Upgrade to actions/checkout@v4",
        severity: LintSeverity::Info,
    },
    DeprecationRule {
        pattern: "actions/setup-node@v2",
        message: "actions/setup-node@v2 is deprecated",
        suggestion: "Upgrade to actions/setup-node@v4",
        severity: LintSeverity::Warning,
    },
    DeprecationRule {
        pattern: "actions/setup-node@v3",
        message: "actions/setup-node@v3 is outdated",
        suggestion: "Upgrade to actions/setup-node@v4",
        severity: LintSeverity::Info,
    },
    DeprecationRule {
        pattern: "actions/setup-python@v2",
        message: "actions/setup-python@v2 is deprecated",
        suggestion: "Upgrade to actions/setup-python@v5",
        severity: LintSeverity::Warning,
    },
    DeprecationRule {
        pattern: "actions/upload-artifact@v2",
        message: "actions/upload-artifact@v2 is deprecated and uses Node 12",
        suggestion: "Upgrade to actions/upload-artifact@v4",
        severity: LintSeverity::Warning,
    },
    DeprecationRule {
        pattern: "actions/upload-artifact@v3",
        message: "actions/upload-artifact@v3 is outdated",
        suggestion: "Upgrade to actions/upload-artifact@v4",
        severity: LintSeverity::Info,
    },
    DeprecationRule {
        pattern: "actions/download-artifact@v2",
        message: "actions/download-artifact@v2 is deprecated",
        suggestion: "Upgrade to actions/download-artifact@v4",
        severity: LintSeverity::Warning,
    },
    DeprecationRule {
        pattern: "actions/download-artifact@v3",
        message: "actions/download-artifact@v3 is outdated",
        suggestion: "Upgrade to actions/download-artifact@v4",
        severity: LintSeverity::Info,
    },
    DeprecationRule {
        pattern: "actions/cache@v2",
        message: "actions/cache@v2 is deprecated",
        suggestion: "Upgrade to actions/cache@v4",
        severity: LintSeverity::Warning,
    },
];

const GITLAB_DEPRECATIONS: &[DeprecationRule] = &[
    DeprecationRule {
        pattern: "only:",
        message: "The 'only' keyword is deprecated in GitLab CI",
        suggestion: "Use 'rules:' syntax instead",
        severity: LintSeverity::Warning,
    },
    DeprecationRule {
        pattern: "except:",
        message: "The 'except' keyword is deprecated in GitLab CI",
        suggestion: "Use 'rules:' syntax instead",
        severity: LintSeverity::Warning,
    },
];

/// Check for deprecated actions, features, and patterns.
pub fn check_deprecations(dag: &PipelineDag) -> Vec<LintFinding> {
    let mut findings = Vec::new();

    let rules = match dag.provider.as_str() {
        "github-actions" => GITHUB_DEPRECATIONS,
        "gitlab-ci" => GITLAB_DEPRECATIONS,
        _ => return findings,
    };

    for node in dag.graph.node_weights() {
        for step in &node.steps {
            if let Some(uses) = &step.uses {
                for rule in rules {
                    if uses.contains(rule.pattern) {
                        findings.push(LintFinding {
                            severity: rule.severity,
                            rule_id: "PLX-LINT-DEPR".to_string(),
                            message: format!(
                                "{} (job '{}', step '{}')",
                                rule.message, node.id, step.name
                            ),
                            suggestion: Some(rule.suggestion.to_string()),
                            location: Some(format!("jobs.{}.steps", node.id)),
                        });
                    }
                }
            }
        }

        // Check runner deprecation: suggest pinned version instead of -latest
        if dag.provider == "github-actions" && node.runs_on.ends_with("-latest") {
            findings.push(LintFinding {
                severity: LintSeverity::Info,
                rule_id: "PLX-LINT-RUNNER".to_string(),
                message: format!(
                    "Job '{}' uses '{}' which auto-updates and may cause unexpected breaks",
                    node.id, node.runs_on
                ),
                suggestion: Some(format!(
                    "Consider pinning to a specific version (e.g., '{}')",
                    node.runs_on.replace("-latest", "-24.04")
                )),
                location: Some(format!("jobs.{}.runs-on", node.id)),
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
    fn test_detect_deprecated_checkout_v2() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "Checkout".into(),
            uses: Some("actions/checkout@v2".into()),
            run: None,
            estimated_duration_secs: None,
        });
        dag.add_job(job);

        let findings = check_deprecations(&dag);
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("deprecated"));
    }

    #[test]
    fn test_latest_runner_info() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let job = JobNode::new("build".into(), "Build".into());
        dag.add_job(job);

        let findings = check_deprecations(&dag);
        assert!(findings.iter().any(|f| f.rule_id == "PLX-LINT-RUNNER"));
    }
}
