use crate::analyzer::report::{AnalysisReport, FindingCategory};
use serde_yaml::Value;

/// Apply test sharding optimizations to the workflow YAML.
pub fn apply_shard_optimizations(yaml: &mut Value, report: &AnalysisReport) {
    let shard_findings: Vec<_> = report.findings.iter()
        .filter(|f| {
            matches!(f.category, FindingCategory::SerialBottleneck)
                && f.title.contains("sharded")
        })
        .collect();

    if shard_findings.is_empty() {
        return;
    }

    let jobs = match yaml.get_mut("jobs").and_then(|v| v.as_mapping_mut()) {
        Some(j) => j,
        None => return,
    };

    for finding in &shard_findings {
        // Extract shard count from title (e.g., "could be sharded into 4 parallel jobs")
        let shard_count = finding.title
            .split_whitespace()
            .find_map(|w| w.parse::<usize>().ok())
            .unwrap_or(4);

        for job_id in &finding.affected_jobs {
            let job_key = Value::String(job_id.clone());
            if let Some(job_config) = jobs.get_mut(&job_key) {
                inject_shard_strategy(job_config, shard_count);
            }
        }
    }
}

fn inject_shard_strategy(job_config: &mut Value, shard_count: usize) {
    // Only add if no matrix strategy already exists
    if job_config.get("strategy").is_some() {
        return;
    }

    let mut matrix = serde_yaml::Mapping::new();

    // Create shard values: [1, 2, 3, ..., N]
    let shards: Vec<Value> = (1..=shard_count)
        .map(|i| Value::Number(serde_yaml::Number::from(i as u64)))
        .collect();

    matrix.insert(
        Value::String("shard".to_string()),
        Value::Sequence(shards),
    );

    let mut strategy = serde_yaml::Mapping::new();
    strategy.insert(
        Value::String("matrix".to_string()),
        Value::Mapping(matrix),
    );

    if let Some(mapping) = job_config.as_mapping_mut() {
        mapping.insert(
            Value::String("strategy".to_string()),
            Value::Mapping(strategy),
        );
    }
}

/// Generate a smart matrix strategy that reduces combinatorial explosion.
pub fn optimize_matrix(
    variables: &std::collections::HashMap<String, Vec<String>>,
    primary_platform: Option<&str>,
) -> Value {
    let primary = primary_platform.unwrap_or("ubuntu-latest");
    let mut include = Vec::new();

    // Identify OS and version variables
    let os_var = variables.keys().find(|k| {
        let k = k.to_lowercase();
        k == "os" || k.contains("runner") || k.contains("platform")
    });

    let version_var = variables.keys().find(|k| {
        let k = k.to_lowercase();
        k.contains("node") || k.contains("python") || k.contains("ruby")
            || k.contains("java") || k.contains("go") || k.contains("rust")
            || k == "version"
    });

    match (os_var, version_var) {
        (Some(os_key), Some(ver_key)) => {
            let os_values = &variables[os_key];
            let ver_values = &variables[ver_key];

            // Full suite on primary OS with all versions
            for ver in ver_values {
                let mut entry = serde_yaml::Mapping::new();
                entry.insert(
                    Value::String(os_key.clone()),
                    Value::String(primary.to_string()),
                );
                entry.insert(
                    Value::String(ver_key.clone()),
                    Value::String(ver.clone()),
                );
                entry.insert(
                    Value::String("full_suite".to_string()),
                    Value::Bool(true),
                );
                include.push(Value::Mapping(entry));
            }

            // Smoke tests on secondary OSes with default version
            let default_ver = ver_values.iter()
                .find(|v| !v.contains("nightly") && !v.contains("beta"))
                .or(ver_values.first());

            if let Some(default_ver) = default_ver {
                for os in os_values {
                    if os != primary {
                        let mut entry = serde_yaml::Mapping::new();
                        entry.insert(
                            Value::String(os_key.clone()),
                            Value::String(os.clone()),
                        );
                        entry.insert(
                            Value::String(ver_key.clone()),
                            Value::String(default_ver.clone()),
                        );
                        entry.insert(
                            Value::String("full_suite".to_string()),
                            Value::Bool(false),
                        );
                        include.push(Value::Mapping(entry));
                    }
                }
            }
        }
        _ => {
            // Simple case: just pass through as include entries
            // Build a reasonable subset
            for (key, values) in variables {
                for val in values {
                    let mut entry = serde_yaml::Mapping::new();
                    entry.insert(
                        Value::String(key.clone()),
                        Value::String(val.clone()),
                    );
                    include.push(Value::Mapping(entry));
                }
            }
        }
    }

    let mut matrix = serde_yaml::Mapping::new();
    matrix.insert(
        Value::String("include".to_string()),
        Value::Sequence(include),
    );

    Value::Mapping(matrix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_optimize_matrix_reduces_combinations() {
        let mut vars = HashMap::new();
        vars.insert("os".to_string(), vec![
            "ubuntu-latest".to_string(),
            "macos-latest".to_string(),
            "windows-latest".to_string(),
        ]);
        vars.insert("node".to_string(), vec![
            "18".to_string(),
            "20".to_string(),
            "22".to_string(),
        ]);

        // Full matrix = 3 * 3 = 9 combinations
        let optimized = optimize_matrix(&vars, Some("ubuntu-latest"));
        let include = optimized.as_mapping().unwrap()
            .get(&Value::String("include".to_string()))
            .unwrap()
            .as_sequence()
            .unwrap();

        // Should be less than 9 (3 versions on ubuntu + 2 secondary OSes = 5)
        assert!(include.len() < 9);
        assert_eq!(include.len(), 5);
    }
}
