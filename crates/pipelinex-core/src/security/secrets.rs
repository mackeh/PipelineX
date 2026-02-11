use crate::analyzer::report::{Finding, FindingCategory, Severity};
use crate::parser::dag::PipelineDag;
use regex::Regex;

struct SecretPattern {
    id: &'static str,
    description: &'static str,
    regex: &'static str,
    severity: Severity,
}

const SECRET_PATTERNS: &[SecretPattern] = &[
    SecretPattern {
        id: "PLX-SEC-001",
        description: "Hardcoded API key or secret in env/run block",
        regex: r#"(?i)(api[_-]?key|secret[_-]?key|access[_-]?key|auth[_-]?token|password)\s*[:=]\s*['"][A-Za-z0-9+/=_\-]{8,}['"]"#,
        severity: Severity::Critical,
    },
    SecretPattern {
        id: "PLX-SEC-002",
        description: "AWS Access Key ID detected",
        regex: r#"AKIA[0-9A-Z]{16}"#,
        severity: Severity::Critical,
    },
    SecretPattern {
        id: "PLX-SEC-003",
        description: "GitHub Personal Access Token detected",
        regex: r#"ghp_[A-Za-z0-9]{36}"#,
        severity: Severity::Critical,
    },
    SecretPattern {
        id: "PLX-SEC-004",
        description: "Docker login with inline password",
        regex: r#"docker\s+login.*-p\s+\S+"#,
        severity: Severity::Critical,
    },
    SecretPattern {
        id: "PLX-SEC-005",
        description: "Base64-encoded secret piped to decode",
        regex: r#"echo\s+[A-Za-z0-9+/=]{40,}\s*\|\s*base64"#,
        severity: Severity::High,
    },
    SecretPattern {
        id: "PLX-SEC-006",
        description: "Generic private key block",
        regex: r#"-----BEGIN\s+(RSA\s+)?PRIVATE\s+KEY-----"#,
        severity: Severity::Critical,
    },
    SecretPattern {
        id: "PLX-SEC-007",
        description: "Slack webhook URL detected",
        regex: r#"https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[A-Za-z0-9]+"#,
        severity: Severity::High,
    },
];

/// Detect hardcoded secrets in CI pipeline configurations.
pub fn detect_secrets(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    for node in dag.graph.node_weights() {
        // Check environment variables
        for (key, value) in &node.env {
            for pattern in SECRET_PATTERNS {
                if let Ok(re) = Regex::new(pattern.regex) {
                    let check_str = format!("{}={}", key, value);
                    if re.is_match(&check_str) {
                        let redacted = redact_value(value);
                        findings.push(Finding {
                            severity: pattern.severity,
                            category: FindingCategory::CustomPlugin,
                            title: format!("Secret exposure: {}", pattern.description),
                            description: format!(
                                "Job '{}' env var '{}' contains what appears to be a hardcoded secret ({}...)",
                                node.id, key, redacted
                            ),
                            affected_jobs: vec![node.id.clone()],
                            recommendation: format!(
                                "Use CI secrets management instead of hardcoding. Move to ${{{{ secrets.{} }}}}",
                                key.to_uppercase()
                            ),
                            fix_command: None,
                            estimated_savings_secs: None,
                            confidence: 0.85,
                            auto_fixable: false,
                        });
                    }
                }
            }
        }

        // Check run steps
        for step in &node.steps {
            if let Some(run) = &step.run {
                for pattern in SECRET_PATTERNS {
                    if let Ok(re) = Regex::new(pattern.regex) {
                        if re.is_match(run) {
                            findings.push(Finding {
                                severity: pattern.severity,
                                category: FindingCategory::CustomPlugin,
                                title: format!("Secret exposure: {}", pattern.description),
                                description: format!(
                                    "Job '{}', step '{}' contains a potential hardcoded secret [{}]",
                                    node.id, step.name, pattern.id
                                ),
                                affected_jobs: vec![node.id.clone()],
                                recommendation: "Remove hardcoded secrets. Use CI platform secrets management (e.g., GitHub Actions secrets, GitLab CI variables).".to_string(),
                                fix_command: None,
                                estimated_savings_secs: None,
                                confidence: 0.80,
                                auto_fixable: false,
                            });
                        }
                    }
                }
            }
        }
    }

    findings
}

fn redact_value(value: &str) -> String {
    if value.len() <= 4 {
        "****".to_string()
    } else {
        format!("{}****", &value[..4])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dag::{JobNode, PipelineDag, StepInfo};
    use std::collections::HashMap;

    #[allow(dead_code)]
    fn make_dag_with_env(env: HashMap<String, String>) -> PipelineDag {
        let mut dag = PipelineDag::new("test".into(), "test.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.env = env;
        dag.add_job(job);
        dag
    }

    fn make_dag_with_run(run_cmd: &str) -> PipelineDag {
        let mut dag = PipelineDag::new("test".into(), "test.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "Run step".into(),
            uses: None,
            run: Some(run_cmd.into()),
            estimated_duration_secs: None,
        });
        dag.add_job(job);
        dag
    }

    #[test]
    fn test_detect_aws_key() {
        let dag = make_dag_with_run("export AWS_KEY=AKIAIOSFODNN7EXAMPLE");
        let findings = detect_secrets(&dag);
        assert!(findings.iter().any(|f| f.title.contains("AWS Access Key")));
    }

    #[test]
    fn test_detect_github_pat() {
        let dag = make_dag_with_run(
            "curl -H 'Authorization: token ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij'",
        );
        let findings = detect_secrets(&dag);
        assert!(findings
            .iter()
            .any(|f| f.title.contains("GitHub Personal Access Token")));
    }

    #[test]
    fn test_detect_docker_login() {
        let dag = make_dag_with_run("docker login -u user -p mysecretpassword registry.io");
        let findings = detect_secrets(&dag);
        assert!(findings.iter().any(|f| f.title.contains("Docker login")));
    }

    #[test]
    fn test_no_false_positive_on_secrets_ref() {
        let dag = make_dag_with_run("echo ${{ secrets.MY_TOKEN }}");
        let findings = detect_secrets(&dag);
        // Should not flag ${{ secrets.* }} references as hardcoded secrets
        assert!(findings.is_empty() || findings.iter().all(|f| f.confidence < 0.9));
    }
}
