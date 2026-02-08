use crate::analyzer::report::{AnalysisReport, FindingCategory};
use serde_yaml::Value;

/// Apply cache optimizations to the workflow YAML.
pub fn apply_cache_optimizations(yaml: &mut Value, report: &AnalysisReport) {
    let cache_findings: Vec<_> = report.findings.iter()
        .filter(|f| matches!(f.category, FindingCategory::MissingCache))
        .collect();

    if cache_findings.is_empty() {
        return;
    }

    let jobs = match yaml.get_mut("jobs").and_then(|v| v.as_mapping_mut()) {
        Some(j) => j,
        None => return,
    };

    for finding in &cache_findings {
        for job_id in &finding.affected_jobs {
            let job_key = Value::String(job_id.clone());
            if let Some(job_config) = jobs.get_mut(&job_key) {
                inject_cache_step(job_config, &finding.title);
            }
        }
    }
}

fn inject_cache_step(job_config: &mut Value, finding_title: &str) {
    let steps = match job_config.get_mut("steps").and_then(|v| v.as_sequence_mut()) {
        Some(s) => s,
        None => return,
    };

    // Determine what type of cache to inject based on the finding
    let cache_step = if finding_title.contains("npm") || finding_title.contains("yarn") || finding_title.contains("pnpm") {
        create_node_cache_step()
    } else if finding_title.contains("pip") {
        create_pip_cache_step()
    } else if finding_title.contains("Cargo") {
        create_cargo_cache_step()
    } else if finding_title.contains("Gradle") || finding_title.contains("Maven") {
        create_gradle_maven_cache_step()
    } else {
        return;
    };

    // Insert cache step after checkout (index 1, or at 0 if no checkout)
    let checkout_idx = steps.iter().position(|s| {
        s.get("uses")
            .and_then(|v| v.as_str())
            .is_some_and(|u| u.starts_with("actions/checkout"))
    });

    let insert_idx = checkout_idx.map(|i| i + 1).unwrap_or(0);
    steps.insert(insert_idx, cache_step);
}

fn create_node_cache_step() -> Value {
    let mut step = serde_yaml::Mapping::new();
    step.insert(
        Value::String("name".to_string()),
        Value::String("Cache node_modules".to_string()),
    );
    step.insert(
        Value::String("uses".to_string()),
        Value::String("actions/cache@v4".to_string()),
    );

    let mut with = serde_yaml::Mapping::new();
    with.insert(
        Value::String("path".to_string()),
        Value::String("node_modules".to_string()),
    );
    with.insert(
        Value::String("key".to_string()),
        Value::String("node-${{ runner.os }}-${{ hashFiles('package-lock.json', 'yarn.lock', 'pnpm-lock.yaml') }}".to_string()),
    );
    with.insert(
        Value::String("restore-keys".to_string()),
        Value::String("node-${{ runner.os }}-".to_string()),
    );

    step.insert(
        Value::String("with".to_string()),
        Value::Mapping(with),
    );

    Value::Mapping(step)
}

fn create_pip_cache_step() -> Value {
    let mut step = serde_yaml::Mapping::new();
    step.insert(
        Value::String("name".to_string()),
        Value::String("Cache pip packages".to_string()),
    );
    step.insert(
        Value::String("uses".to_string()),
        Value::String("actions/cache@v4".to_string()),
    );

    let mut with = serde_yaml::Mapping::new();
    with.insert(
        Value::String("path".to_string()),
        Value::String("~/.cache/pip".to_string()),
    );
    with.insert(
        Value::String("key".to_string()),
        Value::String("pip-${{ runner.os }}-${{ hashFiles('requirements*.txt', 'setup.py', 'pyproject.toml') }}".to_string()),
    );
    with.insert(
        Value::String("restore-keys".to_string()),
        Value::String("pip-${{ runner.os }}-".to_string()),
    );

    step.insert(
        Value::String("with".to_string()),
        Value::Mapping(with),
    );

    Value::Mapping(step)
}

fn create_cargo_cache_step() -> Value {
    let mut step = serde_yaml::Mapping::new();
    step.insert(
        Value::String("name".to_string()),
        Value::String("Cache Cargo dependencies".to_string()),
    );
    step.insert(
        Value::String("uses".to_string()),
        Value::String("Swatinem/rust-cache@v2".to_string()),
    );

    Value::Mapping(step)
}

fn create_gradle_maven_cache_step() -> Value {
    let mut step = serde_yaml::Mapping::new();
    step.insert(
        Value::String("name".to_string()),
        Value::String("Cache Gradle/Maven packages".to_string()),
    );
    step.insert(
        Value::String("uses".to_string()),
        Value::String("actions/cache@v4".to_string()),
    );

    let mut with = serde_yaml::Mapping::new();
    with.insert(
        Value::String("path".to_string()),
        Value::String("|\n~/.gradle/caches\n~/.gradle/wrapper\n~/.m2/repository".to_string()),
    );
    with.insert(
        Value::String("key".to_string()),
        Value::String("java-${{ runner.os }}-${{ hashFiles('**/*.gradle*', '**/pom.xml') }}".to_string()),
    );
    with.insert(
        Value::String("restore-keys".to_string()),
        Value::String("java-${{ runner.os }}-".to_string()),
    );

    step.insert(
        Value::String("with".to_string()),
        Value::Mapping(with),
    );

    Value::Mapping(step)
}
