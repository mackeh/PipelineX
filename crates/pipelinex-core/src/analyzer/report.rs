use serde::{Deserialize, Serialize};
use crate::health_score::HealthScore;

/// Severity level for analysis findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    pub fn priority(&self) -> u8 {
        match self {
            Severity::Critical => 5,
            Severity::High => 4,
            Severity::Medium => 3,
            Severity::Low => 2,
            Severity::Info => 1,
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::High => "HIGH",
            Severity::Medium => "MEDIUM",
            Severity::Low => "LOW",
            Severity::Info => "INFO",
        }
    }

    pub fn color_code(&self) -> &str {
        match self {
            Severity::Critical => "red",
            Severity::High => "yellow",
            Severity::Medium => "yellow",
            Severity::Low => "blue",
            Severity::Info => "white",
        }
    }
}

/// Category of the finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingCategory {
    CriticalPath,
    MissingCache,
    SerialBottleneck,
    MissingPathFilter,
    ShallowClone,
    RedundantSteps,
    DockerOptimization,
    MatrixOptimization,
    FlakyTest,
    ConcurrencyControl,
    ArtifactReuse,
}

impl FindingCategory {
    pub fn label(&self) -> &str {
        match self {
            FindingCategory::CriticalPath => "Critical Path Bottleneck",
            FindingCategory::MissingCache => "Missing Dependency Cache",
            FindingCategory::SerialBottleneck => "Serial Bottleneck",
            FindingCategory::MissingPathFilter => "Missing Path Filter",
            FindingCategory::ShallowClone => "Full Git Clone",
            FindingCategory::RedundantSteps => "Redundant Steps",
            FindingCategory::DockerOptimization => "Docker Build Optimization",
            FindingCategory::MatrixOptimization => "Matrix Strategy Optimization",
            FindingCategory::FlakyTest => "Flaky Test",
            FindingCategory::ConcurrencyControl => "Missing Concurrency Control",
            FindingCategory::ArtifactReuse => "Missing Artifact Reuse",
        }
    }
}

/// A single analysis finding with actionable recommendations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub category: FindingCategory,
    pub title: String,
    pub description: String,
    pub affected_jobs: Vec<String>,
    pub recommendation: String,
    pub fix_command: Option<String>,
    pub estimated_savings_secs: Option<f64>,
    pub confidence: f64,
    pub auto_fixable: bool,
}

impl Finding {
    pub fn savings_display(&self) -> String {
        match self.estimated_savings_secs {
            Some(secs) => format_duration(secs),
            None => "unknown".to_string(),
        }
    }
}

/// The complete analysis report for a pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub pipeline_name: String,
    pub source_file: String,
    pub provider: String,
    pub job_count: usize,
    pub step_count: usize,
    pub max_parallelism: usize,
    pub critical_path: Vec<String>,
    pub critical_path_duration_secs: f64,
    pub total_estimated_duration_secs: f64,
    pub optimized_duration_secs: f64,
    pub findings: Vec<Finding>,
    pub health_score: Option<HealthScore>,
}

impl AnalysisReport {
    pub fn potential_improvement_pct(&self) -> f64 {
        if self.total_estimated_duration_secs == 0.0 {
            return 0.0;
        }
        (self.total_estimated_duration_secs - self.optimized_duration_secs)
            / self.total_estimated_duration_secs
            * 100.0
    }

    pub fn total_savings_secs(&self) -> f64 {
        self.findings.iter()
            .filter_map(|f| f.estimated_savings_secs)
            .sum()
    }

    pub fn critical_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::Critical).count()
    }

    pub fn high_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::High).count()
    }

    pub fn medium_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::Medium).count()
    }
}

/// Format seconds into a human-readable duration string.
pub fn format_duration(secs: f64) -> String {
    let total_secs = secs.round() as u64;
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    if minutes > 0 {
        format!("{}:{:02}", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}
