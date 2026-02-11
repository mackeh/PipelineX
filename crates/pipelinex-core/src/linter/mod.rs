pub mod deprecation;
pub mod schema;
pub mod typo;

use crate::parser::dag::PipelineDag;
use serde::{Deserialize, Serialize};

/// Severity for lint findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LintSeverity {
    Error,
    Warning,
    Info,
}

impl LintSeverity {
    pub fn symbol(&self) -> &str {
        match self {
            LintSeverity::Error => "ERROR",
            LintSeverity::Warning => "WARNING",
            LintSeverity::Info => "INFO",
        }
    }
}

/// A single lint finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintFinding {
    pub severity: LintSeverity,
    pub rule_id: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub location: Option<String>,
}

/// Complete lint report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintReport {
    pub source_file: String,
    pub provider: String,
    pub findings: Vec<LintFinding>,
    pub errors: usize,
    pub warnings: usize,
}

impl LintReport {
    pub fn exit_code(&self) -> i32 {
        if self.errors > 0 {
            2
        } else if self.warnings > 0 {
            1
        } else {
            0
        }
    }
}

/// Run all lint checks on raw YAML content and parsed DAG.
pub fn lint(content: &str, dag: &PipelineDag) -> LintReport {
    let mut findings = Vec::new();

    // YAML syntax validation happens before this function is called
    // (parsing would fail if YAML is invalid)

    // Deprecation checks
    findings.extend(deprecation::check_deprecations(dag));

    // Typo detection on raw YAML content
    findings.extend(typo::check_typos(content, &dag.provider));

    // Schema validation
    findings.extend(schema::validate_schema(content, &dag.provider));

    let errors = findings
        .iter()
        .filter(|f| f.severity == LintSeverity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == LintSeverity::Warning)
        .count();

    LintReport {
        source_file: dag.source_file.clone(),
        provider: dag.provider.clone(),
        findings,
        errors,
        warnings,
    }
}
