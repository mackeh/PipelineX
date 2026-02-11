use super::{LintFinding, LintSeverity};

/// Basic schema validation for CI configs.
pub fn validate_schema(content: &str, provider: &str) -> Vec<LintFinding> {
    let mut findings = Vec::new();

    match provider {
        "github-actions" => findings.extend(validate_github_actions(content)),
        "gitlab-ci" => findings.extend(validate_gitlab_ci(content)),
        _ => {}
    }

    findings
}

fn validate_github_actions(content: &str) -> Vec<LintFinding> {
    let mut findings = Vec::new();

    // Check for required top-level keys
    let yaml: Result<serde_yaml::Value, _> = serde_yaml::from_str(content);
    let yaml = match yaml {
        Ok(v) => v,
        Err(e) => {
            findings.push(LintFinding {
                severity: LintSeverity::Error,
                rule_id: "PLX-LINT-YAML".to_string(),
                message: format!("Invalid YAML: {}", e),
                suggestion: None,
                location: None,
            });
            return findings;
        }
    };

    // Must have 'on' trigger
    // serde_yaml parses bare `on:` as boolean `true`, so also check for Value::Bool(true) key
    let has_on = yaml.get("on").is_some()
        || yaml
            .as_mapping()
            .is_some_and(|m| m.contains_key(serde_yaml::Value::Bool(true)));
    if !has_on {
        findings.push(LintFinding {
            severity: LintSeverity::Error,
            rule_id: "PLX-LINT-SCHEMA-001".to_string(),
            message: "Missing required 'on' trigger block".to_string(),
            suggestion: Some("Add 'on:' with push/pull_request triggers".to_string()),
            location: Some("top-level".to_string()),
        });
    }

    // Must have 'jobs' block
    if yaml.get("jobs").is_none() {
        findings.push(LintFinding {
            severity: LintSeverity::Error,
            rule_id: "PLX-LINT-SCHEMA-002".to_string(),
            message: "Missing required 'jobs' block".to_string(),
            suggestion: Some("Add 'jobs:' block with at least one job".to_string()),
            location: Some("top-level".to_string()),
        });
    }

    // Check jobs have runs-on
    if let Some(jobs) = yaml.get("jobs").and_then(|v| v.as_mapping()) {
        for (job_id, job_config) in jobs {
            let job_name = job_id.as_str().unwrap_or("unknown");
            if job_config.get("runs-on").is_none() && job_config.get("uses").is_none() {
                findings.push(LintFinding {
                    severity: LintSeverity::Error,
                    rule_id: "PLX-LINT-SCHEMA-003".to_string(),
                    message: format!(
                        "Job '{}' missing 'runs-on' or 'uses' (reusable workflow)",
                        job_name
                    ),
                    suggestion: Some("Add 'runs-on: ubuntu-latest' or equivalent".to_string()),
                    location: Some(format!("jobs.{}", job_name)),
                });
            }
        }
    }

    findings
}

fn validate_gitlab_ci(content: &str) -> Vec<LintFinding> {
    let mut findings = Vec::new();

    let yaml: Result<serde_yaml::Value, _> = serde_yaml::from_str(content);
    let yaml = match yaml {
        Ok(v) => v,
        Err(e) => {
            findings.push(LintFinding {
                severity: LintSeverity::Error,
                rule_id: "PLX-LINT-YAML".to_string(),
                message: format!("Invalid YAML: {}", e),
                suggestion: None,
                location: None,
            });
            return findings;
        }
    };

    // Check that stages are defined if referenced
    let has_stages = yaml.get("stages").is_some();
    if let Some(mapping) = yaml.as_mapping() {
        for (key, value) in mapping {
            let key_str = key.as_str().unwrap_or("");
            // Skip known top-level keys
            if matches!(
                key_str,
                "stages"
                    | "variables"
                    | "image"
                    | "services"
                    | "before_script"
                    | "after_script"
                    | "default"
                    | "include"
                    | "workflow"
                    | "pages"
            ) {
                continue;
            }
            // This is likely a job definition
            if let Some(stage) = value.get("stage").and_then(|v| v.as_str()) {
                if !has_stages {
                    findings.push(LintFinding {
                        severity: LintSeverity::Warning,
                        rule_id: "PLX-LINT-SCHEMA-010".to_string(),
                        message: format!(
                            "Job '{}' references stage '{}' but no 'stages:' block is defined",
                            key_str, stage
                        ),
                        suggestion: Some("Add a 'stages:' block listing all stages".to_string()),
                        location: Some(format!("{}.stage", key_str)),
                    });
                }
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_on_trigger() {
        let content = "jobs:\n  build:\n    runs-on: ubuntu-latest\n";
        let findings = validate_github_actions(content);
        assert!(findings.iter().any(|f| f.rule_id == "PLX-LINT-SCHEMA-001"));
    }

    #[test]
    fn test_missing_jobs() {
        let content = "on: push\n";
        let findings = validate_github_actions(content);
        assert!(findings.iter().any(|f| f.rule_id == "PLX-LINT-SCHEMA-002"));
    }

    #[test]
    fn test_missing_runs_on() {
        let content = "on: push\njobs:\n  build:\n    steps:\n      - run: echo hi\n";
        let findings = validate_github_actions(content);
        assert!(findings.iter().any(|f| f.rule_id == "PLX-LINT-SCHEMA-003"));
    }

    #[test]
    fn test_valid_github_actions() {
        let content = "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo hi\n";
        let findings = validate_github_actions(content);
        assert!(findings.is_empty());
    }
}
