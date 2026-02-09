use crate::analyzer::report::{Finding, FindingCategory, Severity};
use crate::parser::dag::PipelineDag;
use crate::runner_sizing::{profile_pipeline, RunnerSizeClass};

/// Convert inferred runner-sizing recommendations into analysis findings.
pub fn detect_runner_right_sizing(dag: &PipelineDag) -> Vec<Finding> {
    let report = profile_pipeline(dag);
    let mut findings = Vec::new();

    for recommendation in report.jobs {
        if recommendation.current_class == recommendation.recommended_class {
            continue;
        }

        let (severity, title, recommendation_text, savings) = match (
            recommendation.current_class,
            recommendation.recommended_class,
        ) {
            (RunnerSizeClass::Small, RunnerSizeClass::Medium)
            | (RunnerSizeClass::Small, RunnerSizeClass::Large)
            | (RunnerSizeClass::Small, RunnerSizeClass::XLarge)
            | (RunnerSizeClass::Medium, RunnerSizeClass::Large)
            | (RunnerSizeClass::Medium, RunnerSizeClass::XLarge)
            | (RunnerSizeClass::Large, RunnerSizeClass::XLarge) => (
                Severity::Medium,
                format!(
                    "Job '{}' appears under-provisioned ({} -> {})",
                    recommendation.job_id,
                    recommendation.current_class.as_str(),
                    recommendation.recommended_class.as_str()
                ),
                format!(
                    "Increase runner size for '{}' to '{}' and compare p90 duration \
                        before/after over at least 30 runs.",
                    recommendation.job_id,
                    recommendation.recommended_class.as_str()
                ),
                Some((recommendation.duration_secs * 0.18).max(30.0)),
            ),
            _ => (
                Severity::Low,
                format!(
                    "Job '{}' may be over-provisioned ({} -> {})",
                    recommendation.job_id,
                    recommendation.current_class.as_str(),
                    recommendation.recommended_class.as_str()
                ),
                format!(
                    "Consider downsizing runner for '{}' to '{}' to reduce cost while \
                        validating no p90 regression.",
                    recommendation.job_id,
                    recommendation.recommended_class.as_str()
                ),
                Some((recommendation.duration_secs * 0.03).max(10.0)),
            ),
        };

        findings.push(Finding {
            severity,
            category: FindingCategory::RunnerSizing,
            title,
            description: format!(
                "Inferred resource profile for '{}' indicates cpu={}, memory={}, io={} \
                using command-level heuristics. Signals: {}.",
                recommendation.job_id,
                recommendation.cpu_pressure,
                recommendation.memory_pressure,
                recommendation.io_pressure,
                recommendation.rationale.join("; "),
            ),
            affected_jobs: vec![recommendation.job_id.clone()],
            recommendation: recommendation_text,
            fix_command: None,
            estimated_savings_secs: savings,
            confidence: recommendation.confidence,
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
    fn emits_runner_sizing_findings_when_resize_needed() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-small
    steps:
      - run: cargo build --release
      - run: cargo test --all
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let findings = detect_runner_right_sizing(&dag);
        assert!(!findings.is_empty());
        assert!(findings
            .iter()
            .any(|f| matches!(f.category, FindingCategory::RunnerSizing)));
    }
}
