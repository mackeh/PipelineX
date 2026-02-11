use crate::analyzer::report::{AnalysisReport, Finding};
use regex::Regex;

/// Redact sensitive information from an analysis report.
pub fn redact_report(report: &AnalysisReport) -> AnalysisReport {
    let mut redacted = report.clone();

    // Redact source file to relative path only
    redacted.source_file = redact_path(&redacted.source_file);

    // Redact findings
    redacted.findings = redacted.findings.into_iter().map(redact_finding).collect();

    // Redact critical path job names (keep structure, anonymize names)
    // We keep the job names as they are structural, not sensitive

    redacted
}

fn redact_finding(mut finding: Finding) -> Finding {
    finding.description = redact_secrets_in_text(&finding.description);
    finding.recommendation = redact_secrets_in_text(&finding.recommendation);
    if let Some(cmd) = &finding.fix_command {
        finding.fix_command = Some(redact_secrets_in_text(cmd));
    }
    finding
}

fn redact_path(path: &str) -> String {
    // Strip absolute paths, keep only relative from project root
    if let Some(idx) = path.rfind(".github/") {
        return path[idx..].to_string();
    }
    if let Some(idx) = path.rfind(".gitlab-ci") {
        return path[idx..].to_string();
    }
    if let Some(idx) = path.rfind(".circleci/") {
        return path[idx..].to_string();
    }
    if let Some(idx) = path.rfind(".buildkite/") {
        return path[idx..].to_string();
    }

    // Generic: strip everything before the last component
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("***")
        .to_string()
}

fn redact_secrets_in_text(text: &str) -> String {
    let mut result = text.to_string();

    // Redact secret names: secrets.FOO_BAR -> secrets.***
    let secrets_re = Regex::new(r"secrets\.([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    result = secrets_re.replace_all(&result, "secrets.***").to_string();

    // Redact URLs with authentication
    let url_re = Regex::new(r"https?://[^\s]+@[^\s]+").unwrap();
    result = url_re
        .replace_all(&result, "https://***@***/***")
        .to_string();

    // Redact internal-looking URLs (not github.com, gitlab.com, or bitbucket.org)
    let url_all_re = Regex::new(r"https?://([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})/[^\s]*").unwrap();
    result = url_all_re
        .replace_all(&result, |caps: &regex::Captures| {
            let host = &caps[1];
            if host == "github.com" || host == "gitlab.com" || host == "bitbucket.org" {
                caps[0].to_string()
            } else {
                "https://internal/***/***".to_string()
            }
        })
        .to_string();

    // Redact anything that looks like a token/key value
    let token_re = Regex::new(r"(?i)(token|key|secret|password)\s*[:=]\s*\S+").unwrap();
    result = token_re.replace_all(&result, "$1=***").to_string();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_path_github() {
        assert_eq!(
            redact_path("/home/user/project/.github/workflows/ci.yml"),
            ".github/workflows/ci.yml"
        );
    }

    #[test]
    fn test_redact_path_generic() {
        assert_eq!(redact_path("/absolute/path/to/config.yml"), "config.yml");
    }

    #[test]
    fn test_redact_secrets_in_text() {
        let text = "Use ${{ secrets.MY_TOKEN }} instead";
        let redacted = redact_secrets_in_text(text);
        assert!(redacted.contains("secrets.***"));
        assert!(!redacted.contains("MY_TOKEN"));
    }

    #[test]
    fn test_redact_internal_urls() {
        let text = "Deploy to https://internal.corp.com/api/deploy";
        let redacted = redact_secrets_in_text(text);
        assert!(!redacted.contains("internal.corp.com"));
    }

    #[test]
    fn test_preserve_github_urls() {
        let text = "See https://github.com/actions/checkout for details";
        let redacted = redact_secrets_in_text(text);
        assert!(redacted.contains("github.com/actions/checkout"));
    }
}
