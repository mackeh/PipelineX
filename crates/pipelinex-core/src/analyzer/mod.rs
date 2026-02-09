pub mod cache_detector;
pub mod critical_path;
pub mod html_report;
pub mod parallel_finder;
pub mod report;
pub mod runner_sizer;
pub mod sarif;
pub mod waste_detector;

use crate::parser::dag::PipelineDag;
use report::{AnalysisReport, Finding};

/// Run all analyzers on a pipeline DAG and produce a unified report.
pub fn analyze(dag: &PipelineDag) -> AnalysisReport {
    let mut findings = Vec::new();

    // Critical path analysis
    let (critical_path, critical_path_duration) = critical_path::find_critical_path(dag);
    findings.extend(critical_path::analyze_critical_path(
        dag,
        &critical_path,
        critical_path_duration,
    ));

    // Cache detection
    findings.extend(cache_detector::detect_missing_caches(dag));

    // Parallelization opportunities
    findings.extend(parallel_finder::find_parallelization_opportunities(dag));

    // Waste detection
    findings.extend(waste_detector::detect_waste(dag));

    // Runner right-sizing recommendations
    findings.extend(runner_sizer::detect_runner_right_sizing(dag));

    // Optional external analyzer plugins (manifest-driven).
    findings.extend(crate::plugins::run_external_analyzer_plugins(dag));

    // Sort findings by severity (critical first)
    findings.sort_by_key(|f| std::cmp::Reverse(f.severity.priority()));

    let total_duration = critical_path_duration;
    let estimated_optimized = estimate_optimized_duration(&findings, total_duration);

    // Calculate health score
    let critical_count = findings
        .iter()
        .filter(|f| f.severity == report::Severity::Critical)
        .count();
    let high_count = findings
        .iter()
        .filter(|f| f.severity == report::Severity::High)
        .count();
    let medium_count = findings
        .iter()
        .filter(|f| f.severity == report::Severity::Medium)
        .count();

    let calculator = crate::health_score::HealthScoreCalculator::new();
    let health_score = calculator.calculate(
        total_duration,
        estimated_optimized,
        0.95, // Default to 95% success rate (will be updated with real data if available)
        dag.max_parallelism() as f64 / dag.job_count().max(1) as f64,
        detect_has_caching(&findings),
        critical_count,
        high_count,
        medium_count,
    );

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
        health_score: Some(health_score),
    }
}

fn detect_has_caching(findings: &[report::Finding]) -> bool {
    // If no "Missing Cache" findings, assume caching is present
    !findings
        .iter()
        .any(|f| matches!(f.category, report::FindingCategory::MissingCache))
}

fn estimate_optimized_duration(findings: &[Finding], current_duration: f64) -> f64 {
    let total_savings: f64 = findings
        .iter()
        .filter_map(|f| f.estimated_savings_secs)
        .sum();
    // Don't go below 20% of original (there's always some irreducible time)
    (current_duration - total_savings).max(current_duration * 0.2)
}
