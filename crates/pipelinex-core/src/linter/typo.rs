use super::{LintFinding, LintSeverity};

const GITHUB_ACTIONS_KEYS: &[&str] = &[
    "name",
    "on",
    "jobs",
    "runs-on",
    "steps",
    "uses",
    "with",
    "env",
    "needs",
    "if",
    "strategy",
    "matrix",
    "services",
    "container",
    "outputs",
    "permissions",
    "concurrency",
    "defaults",
    "timeout-minutes",
    "continue-on-error",
    "runs",
    "secrets",
    "inputs",
    "paths",
    "paths-ignore",
    "branches",
    "branches-ignore",
    "tags",
    "tags-ignore",
    "types",
    "schedule",
    "cron",
    "workflow_dispatch",
    "workflow_call",
    "push",
    "pull_request",
    "pull_request_target",
    "release",
    "id",
    "run",
    "shell",
    "working-directory",
    "fail-fast",
    "max-parallel",
    "include",
    "exclude",
    "upload-artifact",
    "download-artifact",
    "cache",
    "fetch-depth",
    "node-version",
    "python-version",
    "java-version",
    "go-version",
    "group",
    "cancel-in-progress",
];

const GITLAB_CI_KEYS: &[&str] = &[
    "stages",
    "variables",
    "image",
    "services",
    "before_script",
    "after_script",
    "script",
    "stage",
    "only",
    "except",
    "rules",
    "when",
    "allow_failure",
    "needs",
    "dependencies",
    "artifacts",
    "cache",
    "coverage",
    "retry",
    "timeout",
    "parallel",
    "trigger",
    "include",
    "extends",
    "tags",
    "resource_group",
    "environment",
    "release",
    "pages",
    "interruptible",
    "paths",
    "expire_in",
    "reports",
    "untracked",
    "key",
    "policy",
];

/// Check for potential typos in YAML keys using edit distance.
pub fn check_typos(content: &str, provider: &str) -> Vec<LintFinding> {
    let known_keys = match provider {
        "github-actions" => GITHUB_ACTIONS_KEYS,
        "gitlab-ci" => GITLAB_CI_KEYS,
        _ => return Vec::new(),
    };

    let mut findings = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        // Skip comments and empty lines
        if trimmed.starts_with('#') || trimmed.is_empty() || trimmed.starts_with('-') {
            continue;
        }

        // Extract the key (before the colon)
        if let Some(colon_pos) = trimmed.find(':') {
            let key = trimmed[..colon_pos]
                .trim()
                .trim_matches('"')
                .trim_matches('\'');

            // Skip numeric keys, env var values, very short keys
            if key.is_empty() || key.len() < 2 || key.chars().all(|c| c.is_numeric()) {
                continue;
            }

            // Skip keys that are known
            if known_keys.contains(&key) {
                continue;
            }

            // Check if this looks like a job ID, step name, or env var (uppercase)
            if key.chars().all(|c| c.is_uppercase() || c == '_') {
                continue;
            }

            // Find closest match
            let mut best_match = None;
            let mut best_distance = usize::MAX;

            for &known in known_keys {
                let dist = strsim::damerau_levenshtein(key, known);
                if dist < best_distance && dist <= 2 && dist > 0 {
                    best_distance = dist;
                    best_match = Some(known);
                }
            }

            if let Some(suggestion) = best_match {
                findings.push(LintFinding {
                    severity: LintSeverity::Warning,
                    rule_id: "PLX-LINT-TYPO".to_string(),
                    message: format!("Possible typo: '{}' — did you mean '{}'?", key, suggestion),
                    suggestion: Some(format!("Replace '{}' with '{}'", key, suggestion)),
                    location: Some(format!("line {}", line_num + 1)),
                });
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_typo_neeed() {
        let content = "jobs:\n  build:\n    neeed: [setup]\n";
        let findings = check_typos(content, "github-actions");
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("needs"));
    }

    #[test]
    fn test_no_false_positive_on_valid_keys() {
        let content = "name: CI\non:\n  push:\njobs:\n  build:\n    runs-on: ubuntu-latest\n";
        let findings = check_typos(content, "github-actions");
        assert!(findings.is_empty());
    }

    #[test]
    fn test_detect_rns_on() {
        let content = "jobs:\n  build:\n    rns-on: ubuntu-latest\n";
        let findings = check_typos(content, "github-actions");
        assert!(findings.iter().any(|f| f.message.contains("runs-on")));
    }
}
