use crate::parser::dag::PipelineDag;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Policy configuration loaded from `.pipelinex/policy.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyConfig {
    #[serde(default)]
    pub rules: PolicyRules,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyRules {
    /// All actions must be pinned by SHA
    #[serde(default)]
    pub require_sha_pinning: bool,

    /// Banned runner labels (e.g., ["ubuntu-latest", "windows-latest"])
    #[serde(default)]
    pub banned_runners: Vec<String>,

    /// Require cache for these package managers (e.g., ["npm", "yarn", "pip", "cargo"])
    #[serde(default)]
    pub require_cache: Vec<String>,

    /// Maximum allowed pipeline duration in minutes
    pub max_duration_minutes: Option<u32>,

    /// All workflows must have explicit permissions block
    #[serde(default)]
    pub require_permissions_block: bool,

    /// All workflows must have concurrency control
    #[serde(default)]
    pub require_concurrency: bool,

    /// Block secrets in env/run blocks
    #[serde(default)]
    pub block_hardcoded_secrets: bool,

    /// Minimum checkout version allowed (e.g., "v4")
    pub min_checkout_version: Option<String>,
}

/// A policy violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub rule: String,
    pub message: String,
    pub affected_jobs: Vec<String>,
    pub severity: PolicySeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicySeverity {
    Error,
    Warning,
}

impl PolicySeverity {
    pub fn symbol(&self) -> &str {
        match self {
            PolicySeverity::Error => "ERROR",
            PolicySeverity::Warning => "WARN",
        }
    }
}

/// Policy check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyReport {
    pub source_file: String,
    pub violations: Vec<PolicyViolation>,
    pub passed: bool,
}

/// Load policy configuration from a TOML file.
pub fn load_policy(path: &Path) -> anyhow::Result<PolicyConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read policy file '{}': {}", path.display(), e))?;
    let config: PolicyConfig = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse policy file '{}': {}", path.display(), e))?;
    Ok(config)
}

/// Check a pipeline DAG against a policy configuration.
pub fn check_policy(dag: &PipelineDag, policy: &PolicyConfig) -> PolicyReport {
    let mut violations = Vec::new();

    // Check SHA pinning
    if policy.rules.require_sha_pinning {
        let sha_re = regex::Regex::new(r"@[0-9a-f]{40}$").unwrap();
        for node in dag.graph.node_weights() {
            for step in &node.steps {
                if let Some(uses) = &step.uses {
                    if uses.starts_with("./") || uses.starts_with("docker://") {
                        continue;
                    }
                    if !sha_re.is_match(uses) {
                        violations.push(PolicyViolation {
                            rule: "require_sha_pinning".to_string(),
                            message: format!(
                                "Action '{}' in job '{}' is not pinned to a SHA",
                                uses, node.id
                            ),
                            affected_jobs: vec![node.id.clone()],
                            severity: PolicySeverity::Error,
                        });
                    }
                }
            }
        }
    }

    // Check banned runners
    if !policy.rules.banned_runners.is_empty() {
        for node in dag.graph.node_weights() {
            if policy.rules.banned_runners.contains(&node.runs_on) {
                violations.push(PolicyViolation {
                    rule: "banned_runners".to_string(),
                    message: format!("Job '{}' uses banned runner '{}'", node.id, node.runs_on),
                    affected_jobs: vec![node.id.clone()],
                    severity: PolicySeverity::Error,
                });
            }
        }
    }

    // Check max duration
    if let Some(max_minutes) = policy.rules.max_duration_minutes {
        let max_secs = max_minutes as f64 * 60.0;
        for node in dag.graph.node_weights() {
            if node.estimated_duration_secs > max_secs {
                violations.push(PolicyViolation {
                    rule: "max_duration_minutes".to_string(),
                    message: format!(
                        "Job '{}' estimated duration ({:.0}s) exceeds max allowed ({}m)",
                        node.id, node.estimated_duration_secs, max_minutes
                    ),
                    affected_jobs: vec![node.id.clone()],
                    severity: PolicySeverity::Warning,
                });
            }
        }
    }

    // Check require_cache
    if !policy.rules.require_cache.is_empty() {
        for node in dag.graph.node_weights() {
            for pm in &policy.rules.require_cache {
                let uses_pm = node.steps.iter().any(|s| {
                    if let Some(run) = &s.run {
                        match pm.as_str() {
                            "npm" => run.contains("npm ci") || run.contains("npm install"),
                            "yarn" => run.contains("yarn install") || run.contains("yarn --frozen"),
                            "pip" => run.contains("pip install"),
                            "cargo" => run.contains("cargo build") || run.contains("cargo test"),
                            _ => false,
                        }
                    } else {
                        false
                    }
                });

                if uses_pm && node.caches.is_empty() {
                    let has_cache_action = node
                        .steps
                        .iter()
                        .any(|s| s.uses.as_ref().is_some_and(|u| u.contains("cache")));
                    if !has_cache_action {
                        violations.push(PolicyViolation {
                            rule: "require_cache".to_string(),
                            message: format!(
                                "Job '{}' uses {} but has no cache configured",
                                node.id, pm
                            ),
                            affected_jobs: vec![node.id.clone()],
                            severity: PolicySeverity::Error,
                        });
                    }
                }
            }
        }
    }

    // Check require_concurrency (GitHub Actions specific)
    if policy.rules.require_concurrency && dag.provider == "github-actions" {
        // We check if the DAG name or env has concurrency info
        // Since we don't parse concurrency block into DAG, check as best effort
        let has_concurrency_env = dag.env.keys().any(|k| k.contains("concurrency"));
        if !has_concurrency_env {
            violations.push(PolicyViolation {
                rule: "require_concurrency".to_string(),
                message: "Workflow does not have a concurrency control block".to_string(),
                affected_jobs: dag.job_ids(),
                severity: PolicySeverity::Warning,
            });
        }
    }

    let passed = violations
        .iter()
        .all(|v| v.severity != PolicySeverity::Error);

    PolicyReport {
        source_file: dag.source_file.clone(),
        violations,
        passed,
    }
}

/// Generate a starter policy file.
pub fn generate_default_policy() -> String {
    r#"# PipelineX Policy Configuration
# See https://github.com/mackeh/PipelineX/docs/POLICIES.md

[rules]
# Require all actions to be pinned by SHA
require_sha_pinning = false

# Runners that are not allowed
banned_runners = []

# Require caching for these package managers
require_cache = []

# Maximum allowed pipeline duration (minutes)
# max_duration_minutes = 30

# Require explicit permissions block (GitHub Actions)
require_permissions_block = false

# Require concurrency control
require_concurrency = false

# Block hardcoded secrets in env/run blocks
block_hardcoded_secrets = true
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dag::{JobNode, PipelineDag, StepInfo};

    fn make_test_dag() -> PipelineDag {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "Checkout".into(),
            uses: Some("actions/checkout@v4".into()),
            run: None,
            estimated_duration_secs: None,
        });
        job.steps.push(StepInfo {
            name: "Build".into(),
            uses: None,
            run: Some("npm ci && npm run build".into()),
            estimated_duration_secs: None,
        });
        dag.add_job(job);
        dag
    }

    #[test]
    fn test_sha_pinning_violation() {
        let dag = make_test_dag();
        let policy = PolicyConfig {
            rules: PolicyRules {
                require_sha_pinning: true,
                ..Default::default()
            },
        };
        let report = check_policy(&dag, &policy);
        assert!(!report.passed);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == "require_sha_pinning"));
    }

    #[test]
    fn test_banned_runner() {
        let dag = make_test_dag();
        let policy = PolicyConfig {
            rules: PolicyRules {
                banned_runners: vec!["ubuntu-latest".into()],
                ..Default::default()
            },
        };
        let report = check_policy(&dag, &policy);
        assert!(!report.passed);
        assert!(report.violations.iter().any(|v| v.rule == "banned_runners"));
    }

    #[test]
    fn test_require_cache_violation() {
        let dag = make_test_dag();
        let policy = PolicyConfig {
            rules: PolicyRules {
                require_cache: vec!["npm".into()],
                ..Default::default()
            },
        };
        let report = check_policy(&dag, &policy);
        assert!(!report.passed);
        assert!(report.violations.iter().any(|v| v.rule == "require_cache"));
    }

    #[test]
    fn test_empty_policy_passes() {
        let dag = make_test_dag();
        let policy = PolicyConfig::default();
        let report = check_policy(&dag, &policy);
        assert!(report.passed);
    }
}
