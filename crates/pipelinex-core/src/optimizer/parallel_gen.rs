use crate::analyzer::report::{AnalysisReport, FindingCategory};
use serde_yaml::Value;

/// Apply parallelization optimizations to the workflow YAML.
pub fn apply_parallel_optimizations(yaml: &mut Value, report: &AnalysisReport) {
    let parallel_findings: Vec<_> = report.findings.iter()
        .filter(|f| matches!(f.category, FindingCategory::SerialBottleneck))
        .collect();

    if parallel_findings.is_empty() {
        return;
    }

    let jobs = match yaml.get_mut("jobs").and_then(|v| v.as_mapping_mut()) {
        Some(j) => j,
        None => return,
    };

    for finding in &parallel_findings {
        // Handle false dependency removal
        if finding.title.contains("unnecessarily") {
            remove_false_dependency(jobs, finding);
        }
    }
}

fn remove_false_dependency(
    jobs: &mut serde_yaml::Mapping,
    finding: &crate::analyzer::report::Finding,
) {
    if finding.affected_jobs.len() < 2 {
        return;
    }

    // The first job in affected_jobs is the dependent, second is the dependency
    let dependent_id = &finding.affected_jobs[0];
    let dependency_id = &finding.affected_jobs[1];

    let job_key = Value::String(dependent_id.clone());
    if let Some(job_config) = jobs.get_mut(&job_key) {
        if let Some(needs) = job_config.get_mut("needs") {
            match needs {
                Value::String(s) if s == dependency_id => {
                    // If this was the only dependency, remove the needs field entirely
                    if let Some(mapping) = job_config.as_mapping_mut() {
                        mapping.remove(&Value::String("needs".to_string()));
                    }
                }
                Value::Sequence(seq) => {
                    seq.retain(|v| {
                        v.as_str().map_or(true, |s| s != dependency_id)
                    });
                    // If no dependencies left, remove needs
                    if seq.is_empty() {
                        if let Some(mapping) = job_config.as_mapping_mut() {
                            mapping.remove(&Value::String("needs".to_string()));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
