use crate::analyzer::report::{Finding, FindingCategory, Severity};
use crate::parser::dag::PipelineDag;
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
enum PinningRisk {
    Sha,    // Pinned to full SHA — minimal risk
    Tag,    // Tag can be moved — medium risk
    Branch, // Branch ref — high risk
    Latest, // No version — critical risk
    Unknown,
}

impl PinningRisk {
    fn severity(&self) -> Severity {
        match self {
            PinningRisk::Sha => Severity::Info,
            PinningRisk::Tag => Severity::Low,
            PinningRisk::Branch => Severity::High,
            PinningRisk::Latest | PinningRisk::Unknown => Severity::High,
        }
    }

    fn label(&self) -> &str {
        match self {
            PinningRisk::Sha => "SHA-pinned",
            PinningRisk::Tag => "tag-pinned",
            PinningRisk::Branch => "branch-pinned",
            PinningRisk::Latest => "unpinned (latest)",
            PinningRisk::Unknown => "unknown version",
        }
    }
}

fn classify_pinning(reference: &str) -> PinningRisk {
    // Check for SHA pinning (40-char hex)
    let sha_re = Regex::new(r"@[0-9a-f]{40}$").unwrap();
    if sha_re.is_match(reference) {
        return PinningRisk::Sha;
    }

    // Check for semver tag (v1, v1.2, v1.2.3)
    let tag_re = Regex::new(r"@v?\d+(\.\d+)*$").unwrap();
    if tag_re.is_match(reference) {
        return PinningRisk::Tag;
    }

    // Check for branch name
    let branch_re = Regex::new(r"@(main|master|develop|dev|release.*)$").unwrap();
    if branch_re.is_match(reference) {
        return PinningRisk::Branch;
    }

    // If there's an @ but nothing after, or no @ at all
    if !reference.contains('@') {
        return PinningRisk::Latest;
    }

    PinningRisk::Unknown
}

/// Known compromised or high-risk actions.
const KNOWN_RISKY_ACTIONS: &[(&str, &str)] = &[
    (
        "tj-actions/changed-files",
        "Previously compromised (CVE-2023-51664). Pin to verified SHA.",
    ),
    (
        "reviewdog/action-setup",
        "Previously targeted in supply chain attack. Verify SHA.",
    ),
];

/// Assess supply chain risk for third-party actions and images.
pub fn assess_supply_chain(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    for node in dag.graph.node_weights() {
        for step in &node.steps {
            if let Some(uses) = &step.uses {
                // Skip built-in actions (actions/*, github/*)
                let is_first_party = uses.starts_with("actions/")
                    || uses.starts_with("github/")
                    || uses.starts_with("./")
                    || uses.starts_with("docker://");

                let pinning = classify_pinning(uses);

                // Check for known risky actions
                for (risky_action, warning) in KNOWN_RISKY_ACTIONS {
                    if uses.contains(risky_action) {
                        findings.push(Finding {
                            severity: Severity::Critical,
                            category: FindingCategory::CustomPlugin,
                            title: format!("Known supply chain risk: {}", risky_action),
                            description: format!("Job '{}' uses '{}'. {}", node.id, uses, warning),
                            affected_jobs: vec![node.id.clone()],
                            recommendation: format!(
                                "Pin '{}' to a verified full SHA commit hash.",
                                risky_action
                            ),
                            fix_command: None,
                            estimated_savings_secs: None,
                            confidence: 0.95,
                            auto_fixable: false,
                        });
                    }
                }

                // Flag non-SHA-pinned third-party actions
                if !is_first_party && pinning != PinningRisk::Sha {
                    findings.push(Finding {
                        severity: pinning.severity(),
                        category: FindingCategory::CustomPlugin,
                        title: format!(
                            "Third-party action {} is {}",
                            extract_action_name(uses),
                            pinning.label()
                        ),
                        description: format!(
                            "Job '{}' uses '{}' which is {}. Tags and branches can be moved by the action maintainer, potentially injecting malicious code.",
                            node.id, uses, pinning.label()
                        ),
                        affected_jobs: vec![node.id.clone()],
                        recommendation: format!(
                            "Pin to a full SHA: `{}@<full-sha-hash>`. Find the SHA for the current tag on the action's releases page.",
                            extract_action_name(uses)
                        ),
                        fix_command: None,
                        estimated_savings_secs: None,
                        confidence: 0.90,
                        auto_fixable: false,
                    });
                }
            }
        }
    }

    findings
}

fn extract_action_name(uses: &str) -> &str {
    uses.split('@').next().unwrap_or(uses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dag::{JobNode, PipelineDag, StepInfo};

    #[test]
    fn test_sha_pinned_ok() {
        let pinning = classify_pinning("actions/checkout@a5ac7e51b41094c92402da3b24376905380afc29");
        assert_eq!(pinning, PinningRisk::Sha);
    }

    #[test]
    fn test_tag_pinned() {
        let pinning = classify_pinning("actions/checkout@v4");
        assert_eq!(pinning, PinningRisk::Tag);
    }

    #[test]
    fn test_branch_pinned() {
        let pinning = classify_pinning("some/action@main");
        assert_eq!(pinning, PinningRisk::Branch);
    }

    #[test]
    fn test_third_party_tag_flagged() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "Third party".into(),
            uses: Some("some-org/some-action@v1".into()),
            run: None,
            estimated_duration_secs: None,
        });
        dag.add_job(job);

        let findings = assess_supply_chain(&dag);
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.title.contains("tag-pinned")));
    }

    #[test]
    fn test_first_party_not_flagged() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "Checkout".into(),
            uses: Some("actions/checkout@v4".into()),
            run: None,
            estimated_duration_secs: None,
        });
        dag.add_job(job);

        let findings = assess_supply_chain(&dag);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_known_risky_action() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "Changed files".into(),
            uses: Some("tj-actions/changed-files@v35".into()),
            run: None,
            estimated_duration_secs: None,
        });
        dag.add_job(job);

        let findings = assess_supply_chain(&dag);
        assert!(findings
            .iter()
            .any(|f| f.severity == Severity::Critical && f.title.contains("supply chain risk")));
    }
}
