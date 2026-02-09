use crate::parser::dag::{JobNode, PipelineDag};
use serde::{Deserialize, Serialize};

/// Normalized runner size classes used for recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunnerSizeClass {
    Small,
    Medium,
    Large,
    XLarge,
}

impl RunnerSizeClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunnerSizeClass::Small => "small",
            RunnerSizeClass::Medium => "medium",
            RunnerSizeClass::Large => "large",
            RunnerSizeClass::XLarge => "xlarge",
        }
    }
}

/// Resource-pressure profile for a single CI job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRunnerRecommendation {
    pub job_id: String,
    pub current_runner: String,
    pub current_class: RunnerSizeClass,
    pub recommended_class: RunnerSizeClass,
    pub cpu_pressure: u8,
    pub memory_pressure: u8,
    pub io_pressure: u8,
    pub duration_secs: f64,
    pub rationale: Vec<String>,
    pub confidence: f64,
}

impl JobRunnerRecommendation {
    pub fn should_resize(&self) -> bool {
        self.current_class != self.recommended_class
    }
}

/// Aggregate report for runner sizing analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerSizingReport {
    pub pipeline_name: String,
    pub provider: String,
    pub total_jobs: usize,
    pub upsizing_jobs: usize,
    pub downsizing_jobs: usize,
    pub unchanged_jobs: usize,
    pub jobs: Vec<JobRunnerRecommendation>,
}

/// Build runner right-sizing recommendations from inferred resource pressure.
pub fn profile_pipeline(dag: &PipelineDag) -> RunnerSizingReport {
    let mut jobs = dag
        .graph
        .node_weights()
        .map(profile_job)
        .collect::<Vec<JobRunnerRecommendation>>();

    jobs.sort_by(|a, b| a.job_id.cmp(&b.job_id));

    let upsizing_jobs = jobs
        .iter()
        .filter(|job| rank(job.recommended_class) > rank(job.current_class))
        .count();
    let downsizing_jobs = jobs
        .iter()
        .filter(|job| rank(job.recommended_class) < rank(job.current_class))
        .count();
    let unchanged_jobs = jobs.len().saturating_sub(upsizing_jobs + downsizing_jobs);

    RunnerSizingReport {
        pipeline_name: dag.name.clone(),
        provider: dag.provider.clone(),
        total_jobs: jobs.len(),
        upsizing_jobs,
        downsizing_jobs,
        unchanged_jobs,
        jobs,
    }
}

fn profile_job(job: &JobNode) -> JobRunnerRecommendation {
    let mut cpu = 0u8;
    let mut memory = 0u8;
    let mut io = 0u8;
    let mut rationale = Vec::new();

    for step in &job.steps {
        if let Some(run) = &step.run {
            let text = run.to_lowercase();

            if contains_any(
                &text,
                &[
                    "cargo build",
                    "cargo test",
                    "go build",
                    "go test",
                    "mvn test",
                    "mvn package",
                    "gradle test",
                    "./gradlew test",
                    "webpack",
                    "vite build",
                    "tsc",
                    "docker build",
                ],
            ) {
                cpu = cpu.saturating_add(3);
                memory = memory.saturating_add(1);
                rationale.push("compute-heavy build/test workload detected".to_string());
            }

            if contains_any(
                &text,
                &[
                    "docker build",
                    "integration",
                    "e2e",
                    "playwright",
                    "cypress",
                    "java -xmx",
                ],
            ) {
                memory = memory.saturating_add(3);
                rationale.push("memory-heavy workload markers detected".to_string());
            }

            if contains_any(
                &text,
                &[
                    "npm ci",
                    "npm install",
                    "pnpm install",
                    "yarn install",
                    "pip install",
                    "apt-get",
                    "docker pull",
                    "docker push",
                    "git clone",
                    "upload-artifact",
                    "download-artifact",
                ],
            ) {
                io = io.saturating_add(2);
                rationale.push(
                    "io/network-heavy dependency or artifact operations detected".to_string(),
                );
            }
        }

        if let Some(uses) = &step.uses {
            let lower = uses.to_lowercase();
            if lower.contains("upload-artifact") || lower.contains("download-artifact") {
                io = io.saturating_add(2);
                rationale.push("artifact transfer action detected".to_string());
            }
        }
    }

    if let Some(matrix) = &job.matrix {
        if matrix.total_combinations >= 6 {
            cpu = cpu.saturating_add(2);
            memory = memory.saturating_add(1);
            rationale.push(format!(
                "large matrix strategy ({}) adds execution pressure",
                matrix.total_combinations
            ));
        }
    }

    if job.estimated_duration_secs >= 15.0 * 60.0 {
        cpu = cpu.saturating_add(2);
        memory = memory.saturating_add(1);
        rationale.push(format!(
            "long-running job ({:.0}m) suggests resource pressure",
            job.estimated_duration_secs / 60.0
        ));
    } else if job.estimated_duration_secs <= 90.0 {
        rationale.push("short-running job likely over-provisioned on larger runners".to_string());
    }

    let current_class = classify_current_runner(&job.runs_on);
    let recommended_class = classify_recommended_runner(cpu, memory, io);
    let confidence = estimate_confidence(cpu, memory, io, rationale.len());

    let mut deduped_rationale = Vec::new();
    for reason in rationale {
        if !deduped_rationale.contains(&reason) {
            deduped_rationale.push(reason);
        }
    }
    if deduped_rationale.is_empty() {
        deduped_rationale
            .push("insufficient explicit profiling signals; defaulting to medium".into());
    }

    JobRunnerRecommendation {
        job_id: job.id.clone(),
        current_runner: job.runs_on.clone(),
        current_class,
        recommended_class,
        cpu_pressure: cpu.min(10),
        memory_pressure: memory.min(10),
        io_pressure: io.min(10),
        duration_secs: job.estimated_duration_secs,
        rationale: deduped_rationale,
        confidence,
    }
}

fn classify_current_runner(runs_on: &str) -> RunnerSizeClass {
    let lower = runs_on.to_lowercase();
    if lower.contains("2xlarge") || lower.contains("4xlarge") || lower.contains("xlarge") {
        RunnerSizeClass::XLarge
    } else if lower.contains("large") {
        RunnerSizeClass::Large
    } else if lower.contains("small") {
        RunnerSizeClass::Small
    } else {
        RunnerSizeClass::Medium
    }
}

fn classify_recommended_runner(cpu: u8, memory: u8, io: u8) -> RunnerSizeClass {
    let max_pressure = cpu.max(memory).max(io);
    if max_pressure >= 8 || (cpu >= 7 && memory >= 6) {
        RunnerSizeClass::XLarge
    } else if max_pressure >= 5 || (cpu >= 4 && memory >= 4) {
        RunnerSizeClass::Large
    } else if max_pressure <= 2 {
        RunnerSizeClass::Small
    } else {
        RunnerSizeClass::Medium
    }
}

fn estimate_confidence(cpu: u8, memory: u8, io: u8, reasons: usize) -> f64 {
    let score = cpu as usize + memory as usize + io as usize;
    if reasons >= 3 && score >= 8 {
        0.88
    } else if reasons >= 2 && score >= 5 {
        0.78
    } else if reasons >= 1 {
        0.68
    } else {
        0.55
    }
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| text.contains(pattern))
}

fn rank(size: RunnerSizeClass) -> u8 {
    match size {
        RunnerSizeClass::Small => 0,
        RunnerSizeClass::Medium => 1,
        RunnerSizeClass::Large => 2,
        RunnerSizeClass::XLarge => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::github::GitHubActionsParser;

    #[test]
    fn recommends_upsizing_for_heavy_build_job() {
        let yaml = r#"
name: Heavy Build
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [a,b,c,d,e,f]
    steps:
      - run: cargo build --release
      - run: cargo test --all
      - run: docker build -t app .
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let report = profile_pipeline(&dag);
        let job = &report.jobs[0];
        assert_eq!(job.current_class, RunnerSizeClass::Medium);
        assert!(matches!(
            job.recommended_class,
            RunnerSizeClass::Large | RunnerSizeClass::XLarge
        ));
        assert!(job.confidence >= 0.7);
    }

    #[test]
    fn recommends_downsizing_for_simple_short_job() {
        let yaml = r#"
name: Lightweight
on: push
jobs:
  lint:
    runs-on: ubuntu-large
    steps:
      - run: echo hello
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let report = profile_pipeline(&dag);
        let job = &report.jobs[0];
        assert_eq!(job.current_class, RunnerSizeClass::Large);
        assert!(matches!(
            job.recommended_class,
            RunnerSizeClass::Small | RunnerSizeClass::Medium
        ));
    }
}
