pub mod cache_gen;
pub mod docker_opt;
pub mod parallel_gen;
pub mod shard_gen;

use crate::analyzer::report::AnalysisReport;
use anyhow::Result;
use serde_yaml::Value;
use std::path::Path;

/// The optimizer takes an analysis report and generates an optimized pipeline config.
pub struct Optimizer;

impl Optimizer {
    /// Generate an optimized workflow YAML from the original file and analysis report.
    pub fn optimize(original_path: &Path, report: &AnalysisReport) -> Result<String> {
        let content = std::fs::read_to_string(original_path)?;
        let mut yaml: Value = serde_yaml::from_str(&content)?;

        // Apply cache optimizations
        cache_gen::apply_cache_optimizations(&mut yaml, report);

        // Apply parallelization optimizations
        parallel_gen::apply_parallel_optimizations(&mut yaml, report);

        // Apply path filter optimization
        apply_path_filter(&mut yaml, report);

        // Apply concurrency optimization
        apply_concurrency(&mut yaml, report);

        // Apply shallow clone optimization
        apply_shallow_clone(&mut yaml, report);

        let result = serde_yaml::to_string(&yaml)?;
        Ok(add_optimization_header(&result, report))
    }

    /// Generate an optimized version from YAML string content.
    pub fn optimize_content(content: &str, report: &AnalysisReport) -> Result<String> {
        let mut yaml: Value = serde_yaml::from_str(content)?;

        cache_gen::apply_cache_optimizations(&mut yaml, report);
        parallel_gen::apply_parallel_optimizations(&mut yaml, report);
        apply_path_filter(&mut yaml, report);
        apply_concurrency(&mut yaml, report);
        apply_shallow_clone(&mut yaml, report);

        let result = serde_yaml::to_string(&yaml)?;
        Ok(add_optimization_header(&result, report))
    }
}

fn apply_path_filter(yaml: &mut Value, report: &AnalysisReport) {
    let has_path_finding = report.findings.iter().any(|f| {
        matches!(
            f.category,
            crate::analyzer::report::FindingCategory::MissingPathFilter
        )
    });

    if !has_path_finding {
        return;
    }

    // Try to add paths-ignore to push triggers
    if let Some(on) = yaml.get_mut("on") {
        if let Some(push) = on.get_mut("push") {
            if push.get("paths-ignore").is_none() {
                let paths_ignore = Value::Sequence(vec![
                    Value::String("docs/**".to_string()),
                    Value::String("*.md".to_string()),
                    Value::String(".gitignore".to_string()),
                    Value::String("LICENSE".to_string()),
                ]);
                if let Some(mapping) = push.as_mapping_mut() {
                    mapping.insert(Value::String("paths-ignore".to_string()), paths_ignore);
                }
            }
        }
        // Also handle pull_request triggers
        if let Some(pr) = on.get_mut("pull_request") {
            if pr.get("paths-ignore").is_none() {
                let paths_ignore = Value::Sequence(vec![
                    Value::String("docs/**".to_string()),
                    Value::String("*.md".to_string()),
                    Value::String(".gitignore".to_string()),
                    Value::String("LICENSE".to_string()),
                ]);
                if let Some(mapping) = pr.as_mapping_mut() {
                    mapping.insert(Value::String("paths-ignore".to_string()), paths_ignore);
                }
            }
        }
    }
}

fn apply_concurrency(yaml: &mut Value, report: &AnalysisReport) {
    let has_concurrency_finding = report.findings.iter().any(|f| {
        matches!(
            f.category,
            crate::analyzer::report::FindingCategory::ConcurrencyControl
        )
    });

    if !has_concurrency_finding {
        return;
    }

    if yaml.get("concurrency").is_none() {
        let mut concurrency = serde_yaml::Mapping::new();
        concurrency.insert(
            Value::String("group".to_string()),
            Value::String("${{ github.workflow }}-${{ github.ref }}".to_string()),
        );
        concurrency.insert(
            Value::String("cancel-in-progress".to_string()),
            Value::Bool(true),
        );

        if let Some(mapping) = yaml.as_mapping_mut() {
            mapping.insert(
                Value::String("concurrency".to_string()),
                Value::Mapping(concurrency),
            );
        }
    }
}

fn apply_shallow_clone(yaml: &mut Value, report: &AnalysisReport) {
    let has_shallow_finding = report.findings.iter().any(|f| {
        matches!(
            f.category,
            crate::analyzer::report::FindingCategory::ShallowClone
        )
    });

    if !has_shallow_finding {
        return;
    }

    // Walk through all jobs and their steps to add fetch-depth: 1
    if let Some(jobs) = yaml.get_mut("jobs").and_then(|v| v.as_mapping_mut()) {
        for (_job_id, job_config) in jobs.iter_mut() {
            if let Some(steps) = job_config
                .get_mut("steps")
                .and_then(|v| v.as_sequence_mut())
            {
                for step in steps.iter_mut() {
                    if let Some(uses) = step.get("uses").and_then(|v| v.as_str()) {
                        if uses.starts_with("actions/checkout") {
                            // Add 'with: { fetch-depth: 1 }' if not present
                            if step.get("with").is_none() {
                                let mut with = serde_yaml::Mapping::new();
                                with.insert(
                                    Value::String("fetch-depth".to_string()),
                                    Value::Number(serde_yaml::Number::from(1)),
                                );
                                if let Some(mapping) = step.as_mapping_mut() {
                                    mapping.insert(
                                        Value::String("with".to_string()),
                                        Value::Mapping(with),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn add_optimization_header(yaml: &str, report: &AnalysisReport) -> String {
    format!(
        "# Optimized by PipelineX v0.1.0\n\
         # Original: {}\n\
         # Estimated improvement: {:.1}% faster\n\
         # Findings applied: {}\n\
         #\n\
         # Run `pipelinex analyze` on this file to verify improvements.\n\
         \n{}",
        report.source_file,
        report.potential_improvement_pct(),
        report.findings.len(),
        yaml
    )
}
