pub mod injection;
pub mod permissions;
pub mod secrets;
pub mod supply_chain;

use crate::analyzer::report::Finding;
use crate::parser::dag::PipelineDag;

/// Run all security scanners on a pipeline DAG.
pub fn scan(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(secrets::detect_secrets(dag));
    findings.extend(permissions::audit_permissions(dag));
    findings.extend(injection::detect_injection(dag));
    findings.extend(supply_chain::assess_supply_chain(dag));
    findings
}
