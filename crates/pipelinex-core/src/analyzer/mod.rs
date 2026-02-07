pub mod critical_path;
pub mod cache_detector;
pub mod parallel_finder;
pub mod waste_detector;
pub mod report;

use crate::parser::dag::PipelineDag;
use report::{AnalysisReport, Finding};

/// Run all analyzers on a pipeline DAG and produce a unified report.
pub fn analyze(dag: &PipelineDag) -> AnalysisReport {
    let mut findings = Vec::new();

    // Critical path analysis
    let (critical_path, critical_path_duration) = critical_path::find_critical_path(dag);
    findings.extend(critical_path::analyze_critical_path(dag, &critical_path, critical_path_duration));

    // Cache detection
    findings.extend(cache_detector::detect_missing_caches(dag));

    // Parallelization opportunities
    findings.extend(parallel_finder::find_parallelization_opportunities(dag));

    // Waste detection
    findings.extend(waste_detector::detect_waste(dag));

    // Sort findings by severity (critical first)
    findings.sort_by_key(|f| std::cmp::Reverse(f.severity.priority()));

    let total_duration = critical_path_duration;
    let estimated_optimized = estimate_optimized_duration(&findings, total_duration);

    AnalysisReport {
        pipeline_name: dag.name.clone(),
        source_file: dag.source_file.clone(),
        provider: dag.provider.clone(),
        job_count: dag.job_count(),
        step_count: dag.step_count(),
        max_parallelism: dag.max_parallelism(),
        critical_path: critical_path.iter().map(|j| j.id.clone()).collect(),
        critical_path_duration_secs: critical_path_duration,
        total_estimated_duration_secs: total_duration,
        optimized_duration_secs: estimated_optimized,
        findings,
    }
}

fn estimate_optimized_duration(findings: &[Finding], current_duration: f64) -> f64 {
    let total_savings: f64 = findings.iter()
        .filter_map(|f| f.estimated_savings_secs)
        .sum();
    // Don't go below 20% of original (there's always some irreducible time)
    (current_duration - total_savings).max(current_duration * 0.2)
}
