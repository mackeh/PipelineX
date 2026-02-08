use crate::analyzer::report::{AnalysisReport, Finding, Severity};
use serde_json::json;

/// Generate a SARIF 2.1.0 report from an analysis report.
/// SARIF (Static Analysis Results Interchange Format) is consumed by
/// GitHub Code Scanning, VS Code, and other tools.
pub fn to_sarif(report: &AnalysisReport) -> serde_json::Value {
    let rules: Vec<serde_json::Value> = report
        .findings
        .iter()
        .enumerate()
        .map(|(i, f)| sarif_rule(i, f))
        .collect();

    let results: Vec<serde_json::Value> = report
        .findings
        .iter()
        .enumerate()
        .map(|(i, f)| sarif_result(i, f, &report.source_file))
        .collect();

    json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "PipelineX",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/mackeh/PipelineX",
                    "rules": rules,
                }
            },
            "results": results,
            "invocations": [{
                "executionSuccessful": true,
                "toolExecutionNotifications": [],
            }]
        }]
    })
}

fn sarif_rule(index: usize, finding: &Finding) -> serde_json::Value {
    let level = match finding.severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    };

    json!({
        "id": format!("PX{:03}", index + 1),
        "name": finding.category.label(),
        "shortDescription": {
            "text": finding.title.clone(),
        },
        "fullDescription": {
            "text": finding.description.clone(),
        },
        "helpUri": "https://github.com/mackeh/PipelineX#what-it-detects",
        "defaultConfiguration": {
            "level": level,
        },
        "properties": {
            "category": finding.category.label(),
            "confidence": finding.confidence,
            "autoFixable": finding.auto_fixable,
            "estimatedSavingsSeconds": finding.estimated_savings_secs,
        }
    })
}

fn sarif_result(index: usize, finding: &Finding, source_file: &str) -> serde_json::Value {
    let level = match finding.severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    };

    let mut result = json!({
        "ruleId": format!("PX{:03}", index + 1),
        "level": level,
        "message": {
            "text": format!("{}\n\nRecommendation: {}", finding.description, finding.recommendation),
        },
        "locations": [{
            "physicalLocation": {
                "artifactLocation": {
                    "uri": source_file,
                },
                "region": {
                    "startLine": 1,
                }
            }
        }],
    });

    // Add fix information if available
    if let Some(cmd) = &finding.fix_command {
        result["fixes"] = json!([{
            "description": {
                "text": format!("Run: {}", cmd),
            }
        }]);
    }

    if !finding.affected_jobs.is_empty() {
        result["relatedLocations"] = serde_json::Value::Array(
            finding
                .affected_jobs
                .iter()
                .enumerate()
                .map(|(i, job)| {
                    json!({
                        "id": i,
                        "message": {
                            "text": format!("Affected job: {}", job),
                        },
                        "physicalLocation": {
                            "artifactLocation": {
                                "uri": source_file,
                            }
                        }
                    })
                })
                .collect(),
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer;
    use crate::parser::github::GitHubActionsParser;

    #[test]
    fn test_sarif_output_is_valid() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm ci
      - run: npm run build
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let report = analyzer::analyze(&dag);
        let sarif = to_sarif(&report);

        assert_eq!(sarif["version"], "2.1.0");
        assert!(sarif["runs"].is_array());
        let runs = sarif["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0]["tool"]["driver"]["name"], "PipelineX");
        assert!(runs[0]["results"].is_array());
    }
}
