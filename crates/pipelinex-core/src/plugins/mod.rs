use crate::analyzer::report::{Finding, FindingCategory, Severity};
use crate::parser::dag::PipelineDag;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Manifest format for external plugins.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginManifest {
    #[serde(default)]
    pub analyzers: Vec<ExternalAnalyzerPlugin>,
    #[serde(default)]
    pub optimizers: Vec<ExternalOptimizerPlugin>,
}

/// External analyzer plugin config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAnalyzerPlugin {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// External optimizer plugin config.
///
/// This is scaffolded for future optimizer execution orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalOptimizerPlugin {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
struct PluginRunInput {
    pipeline: PipelineSummary,
}

#[derive(Debug, Clone, Serialize)]
struct PipelineSummary {
    name: String,
    source_file: String,
    provider: String,
    job_count: usize,
    step_count: usize,
    max_parallelism: usize,
    jobs: Vec<JobSummary>,
}

#[derive(Debug, Clone, Serialize)]
struct JobSummary {
    id: String,
    name: String,
    needs: Vec<String>,
    runs_on: String,
    step_count: usize,
    estimated_duration_secs: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct PluginResultEnvelope {
    #[serde(default)]
    findings: Vec<PluginFinding>,
}

#[derive(Debug, Clone, Deserialize)]
struct PluginFinding {
    severity: String,
    title: String,
    description: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    affected_jobs: Option<Vec<String>>,
    #[serde(default)]
    recommendation: Option<String>,
    #[serde(default)]
    fix_command: Option<String>,
    #[serde(default)]
    estimated_savings_secs: Option<f64>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    auto_fixable: Option<bool>,
}

fn default_timeout_ms() -> u64 {
    10_000
}

fn default_true() -> bool {
    true
}

/// Load plugin manifest from `PIPELINEX_PLUGIN_MANIFEST`.
pub fn load_manifest_from_env() -> anyhow::Result<Option<PluginManifest>> {
    let Some(path) = std::env::var("PIPELINEX_PLUGIN_MANIFEST").ok() else {
        return Ok(None);
    };

    if path.trim().is_empty() {
        return Ok(None);
    }

    let manifest = load_manifest_from_path(PathBuf::from(path))?;
    Ok(Some(manifest))
}

/// Load plugin manifest from a path.
pub fn load_manifest_from_path(path: PathBuf) -> anyhow::Result<PluginManifest> {
    let content = std::fs::read_to_string(&path).map_err(|error| {
        anyhow::anyhow!(
            "Failed to read plugin manifest '{}': {}",
            path.display(),
            error
        )
    })?;

    let manifest: PluginManifest = serde_json::from_str(&content).map_err(|error| {
        anyhow::anyhow!(
            "Invalid plugin manifest JSON '{}': {}",
            path.display(),
            error
        )
    })?;

    Ok(manifest)
}

/// Run analyzer plugins configured in environment manifest.
///
/// Failures are non-fatal and returned as plugin error findings.
pub fn run_external_analyzer_plugins(dag: &PipelineDag) -> Vec<Finding> {
    let manifest = match load_manifest_from_env() {
        Ok(Some(m)) => m,
        Ok(None) => return Vec::new(),
        Err(error) => {
            return vec![plugin_error_finding(
                "plugin-manifest".to_string(),
                format!("Failed to load plugin manifest: {error}"),
            )]
        }
    };

    run_external_analyzer_plugins_with_manifest(dag, &manifest)
}

/// Run analyzer plugins from an explicit manifest.
pub fn run_external_analyzer_plugins_with_manifest(
    dag: &PipelineDag,
    manifest: &PluginManifest,
) -> Vec<Finding> {
    let input = PluginRunInput {
        pipeline: summarize_pipeline(dag),
    };

    let input_json = match serde_json::to_string(&input) {
        Ok(json) => json,
        Err(error) => {
            return vec![plugin_error_finding(
                "plugin-runtime".to_string(),
                format!("Failed to serialize plugin input: {error}"),
            )]
        }
    };

    let mut findings = Vec::new();
    for plugin in manifest.analyzers.iter().filter(|plugin| plugin.enabled) {
        match run_single_analyzer_plugin(plugin, &input_json) {
            Ok(plugin_findings) => findings.extend(plugin_findings),
            Err(error) => findings.push(plugin_error_finding(plugin.id.clone(), error)),
        }
    }
    findings
}

fn summarize_pipeline(dag: &PipelineDag) -> PipelineSummary {
    let jobs = dag
        .graph
        .node_weights()
        .map(|job| JobSummary {
            id: job.id.clone(),
            name: job.name.clone(),
            needs: job.needs.clone(),
            runs_on: job.runs_on.clone(),
            step_count: job.steps.len(),
            estimated_duration_secs: job.estimated_duration_secs,
        })
        .collect::<Vec<_>>();

    PipelineSummary {
        name: dag.name.clone(),
        source_file: dag.source_file.clone(),
        provider: dag.provider.clone(),
        job_count: dag.job_count(),
        step_count: dag.step_count(),
        max_parallelism: dag.max_parallelism(),
        jobs,
    }
}

fn run_single_analyzer_plugin(
    plugin: &ExternalAnalyzerPlugin,
    input_json: &str,
) -> Result<Vec<Finding>, String> {
    let mut child = Command::new(&plugin.command)
        .args(&plugin.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to spawn plugin '{}': {}", plugin.id, error))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(input_json.as_bytes()).map_err(|error| {
            format!(
                "Failed to write stdin for plugin '{}': {}",
                plugin.id, error
            )
        })?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("Failed to wait on plugin '{}': {}", plugin.id, error))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Plugin '{}' exited with {}: {}",
            plugin.id,
            output.status,
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("Plugin '{}' returned non-UTF8 output: {}", plugin.id, error))?;

    parse_plugin_output(plugin, &stdout)
}

fn parse_plugin_output(
    plugin: &ExternalAnalyzerPlugin,
    stdout: &str,
) -> Result<Vec<Finding>, String> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let parsed_as_array = serde_json::from_str::<Vec<PluginFinding>>(trimmed);
    let parsed_findings = match parsed_as_array {
        Ok(findings) => findings,
        Err(_) => {
            let envelope: PluginResultEnvelope =
                serde_json::from_str(trimmed).map_err(|error| {
                    format!(
                        "Plugin '{}' returned invalid JSON output: {}",
                        plugin.id, error
                    )
                })?;
            envelope.findings
        }
    };

    Ok(parsed_findings
        .into_iter()
        .map(|finding| plugin_finding_to_core(plugin, finding))
        .collect())
}

fn plugin_finding_to_core(plugin: &ExternalAnalyzerPlugin, finding: PluginFinding) -> Finding {
    let category = match finding
        .category
        .as_deref()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "criticalpath" | "critical_path" => FindingCategory::CriticalPath,
        "missingcache" | "missing_cache" => FindingCategory::MissingCache,
        "serialbottleneck" | "serial_bottleneck" => FindingCategory::SerialBottleneck,
        "missingpathfilter" | "missing_path_filter" => FindingCategory::MissingPathFilter,
        "shallowclone" | "shallow_clone" => FindingCategory::ShallowClone,
        "redundantsteps" | "redundant_steps" => FindingCategory::RedundantSteps,
        "dockeroptimization" | "docker_optimization" => FindingCategory::DockerOptimization,
        "matrixoptimization" | "matrix_optimization" => FindingCategory::MatrixOptimization,
        "flakytest" | "flaky_test" => FindingCategory::FlakyTest,
        "concurrencycontrol" | "concurrency_control" => FindingCategory::ConcurrencyControl,
        "artifactreuse" | "artifact_reuse" => FindingCategory::ArtifactReuse,
        _ => FindingCategory::CustomPlugin,
    };

    Finding {
        severity: parse_severity(&finding.severity),
        category,
        title: format!("[plugin:{}] {}", plugin.id, finding.title),
        description: finding.description,
        affected_jobs: finding.affected_jobs.unwrap_or_default(),
        recommendation: finding
            .recommendation
            .unwrap_or_else(|| "See plugin output for details.".to_string()),
        fix_command: finding.fix_command,
        estimated_savings_secs: finding.estimated_savings_secs,
        confidence: finding.confidence.unwrap_or(0.7).clamp(0.0, 1.0),
        auto_fixable: finding.auto_fixable.unwrap_or(false),
    }
}

fn parse_severity(value: &str) -> Severity {
    match value.to_lowercase().as_str() {
        "critical" => Severity::Critical,
        "high" => Severity::High,
        "medium" => Severity::Medium,
        "low" => Severity::Low,
        _ => Severity::Info,
    }
}

fn plugin_error_finding(plugin_id: String, message: String) -> Finding {
    Finding {
        severity: Severity::Info,
        category: FindingCategory::CustomPlugin,
        title: format!("[plugin:{plugin_id}] plugin execution issue"),
        description: message,
        affected_jobs: Vec::new(),
        recommendation: "Fix plugin command or manifest configuration.".to_string(),
        fix_command: None,
        estimated_savings_secs: None,
        confidence: 0.3,
        auto_fixable: false,
    }
}

/// Returns optimizer plugin entries declared in the manifest for future optimizer orchestration.
pub fn list_external_optimizer_plugins() -> anyhow::Result<Vec<ExternalOptimizerPlugin>> {
    let manifest = match load_manifest_from_env()? {
        Some(m) => m,
        None => return Ok(Vec::new()),
    };
    Ok(manifest
        .optimizers
        .into_iter()
        .filter(|plugin| plugin.enabled)
        .collect())
}

/// Initialize plugin scaffolding by ensuring a template manifest exists.
///
/// This does not register plugins automatically, but helps users bootstrap manifest wiring.
pub fn scaffold_manifest(path: &Path) -> anyhow::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    if path.exists() {
        return Ok(());
    }

    let template = PluginManifest {
        analyzers: vec![ExternalAnalyzerPlugin {
            id: "example-analyzer".to_string(),
            command: "node".to_string(),
            args: vec!["plugins/example-analyzer.js".to_string()],
            timeout_ms: default_timeout_ms(),
            enabled: false,
        }],
        optimizers: vec![ExternalOptimizerPlugin {
            id: "example-optimizer".to_string(),
            command: "node".to_string(),
            args: vec!["plugins/example-optimizer.js".to_string()],
            timeout_ms: default_timeout_ms(),
            enabled: false,
        }],
    };

    std::fs::write(path, serde_json::to_string_pretty(&template)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dag::PipelineDag;

    #[test]
    fn test_parse_plugin_output_array() {
        let plugin = ExternalAnalyzerPlugin {
            id: "test-plugin".to_string(),
            command: "echo".to_string(),
            args: vec![],
            timeout_ms: 1000,
            enabled: true,
        };

        let findings = parse_plugin_output(
            &plugin,
            r#"[{"severity":"high","title":"Plugin Finding","description":"desc","recommendation":"fix"}]"#,
        )
        .unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::High);
        assert!(findings[0].title.contains("test-plugin"));
    }

    #[test]
    fn test_run_plugins_from_manifest_handles_failure() {
        let mut dag = PipelineDag::new(
            "test".to_string(),
            "test.yml".to_string(),
            "github-actions".to_string(),
        );
        dag.add_job(crate::parser::dag::JobNode::new(
            "build".to_string(),
            "build".to_string(),
        ));

        let manifest = PluginManifest {
            analyzers: vec![ExternalAnalyzerPlugin {
                id: "bad-plugin".to_string(),
                command: "/this/does/not/exist".to_string(),
                args: vec![],
                timeout_ms: 1000,
                enabled: true,
            }],
            optimizers: Vec::new(),
        };

        let findings = run_external_analyzer_plugins_with_manifest(&dag, &manifest);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, FindingCategory::CustomPlugin);
    }
}
