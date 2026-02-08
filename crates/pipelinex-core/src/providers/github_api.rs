use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// GitHub API client for fetching workflow run history
pub struct GitHubClient {
    client: reqwest::Client,
    token: Option<String>,
    base_url: String,
}

/// Workflow run from GitHub Actions API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub run_attempt: u32,
    pub workflow_id: u64,
    pub head_branch: Option<String>,
    pub head_sha: String,
    pub event: String,
}

/// Job within a workflow run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub run_id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub steps: Vec<Step>,
}

/// Step within a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub number: u32,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// API response for workflow runs listing
#[derive(Debug, Deserialize)]
struct WorkflowRunsResponse {
    total_count: u32,
    workflow_runs: Vec<WorkflowRun>,
}

/// API response for jobs listing
#[derive(Debug, Deserialize)]
struct JobsResponse {
    total_count: u32,
    jobs: Vec<Job>,
}

/// Historical timing data for a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobTimingData {
    pub job_name: String,
    pub durations_sec: Vec<f64>,
    pub success_count: usize,
    pub failure_count: usize,
    pub avg_duration_sec: f64,
    pub p50_duration_sec: f64,
    pub p90_duration_sec: f64,
    pub p99_duration_sec: f64,
    pub variance: f64,
}

/// Historical pipeline statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatistics {
    pub workflow_name: String,
    pub total_runs: usize,
    pub success_rate: f64,
    pub avg_duration_sec: f64,
    pub p50_duration_sec: f64,
    pub p90_duration_sec: f64,
    pub p99_duration_sec: f64,
    pub job_timings: Vec<JobTimingData>,
    pub flaky_jobs: Vec<String>,
}

impl GitHubClient {
    /// Create a new GitHub API client
    pub fn new(token: Option<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("PipelineX/0.1.0"),
        );

        if let Some(ref t) = token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", t))
                    .context("Invalid GitHub token")?,
            );
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            client,
            token,
            base_url: "https://api.github.com".to_string(),
        })
    }

    /// Fetch workflow runs for a repository
    pub async fn fetch_workflow_runs(
        &self,
        owner: &str,
        repo: &str,
        workflow_file: &str,
        limit: usize,
    ) -> Result<Vec<WorkflowRun>> {
        let url = format!(
            "{}/repos/{}/{}/actions/workflows/{}/runs",
            self.base_url, owner, repo, workflow_file
        );

        let mut all_runs = Vec::new();
        let per_page = 100.min(limit);
        let mut page = 1;

        while all_runs.len() < limit {
            let response: WorkflowRunsResponse = self
                .client
                .get(&url)
                .query(&[
                    ("per_page", per_page.to_string()),
                    ("page", page.to_string()),
                ])
                .send()
                .await
                .context("Failed to fetch workflow runs")?
                .error_for_status()
                .context("GitHub API returned error")?
                .json()
                .await
                .context("Failed to parse workflow runs response")?;

            if response.workflow_runs.is_empty() {
                break;
            }

            all_runs.extend(response.workflow_runs);
            page += 1;

            if all_runs.len() >= limit {
                all_runs.truncate(limit);
                break;
            }
        }

        Ok(all_runs)
    }

    /// Fetch jobs for a specific workflow run
    pub async fn fetch_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<Vec<Job>> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/jobs",
            self.base_url, owner, repo, run_id
        );

        let response: JobsResponse = self
            .client
            .get(&url)
            .query(&[("per_page", "100")])
            .send()
            .await
            .context("Failed to fetch jobs")?
            .error_for_status()
            .context("GitHub API returned error")?
            .json()
            .await
            .context("Failed to parse jobs response")?;

        Ok(response.jobs)
    }

    /// Calculate statistics from workflow run history
    pub async fn analyze_workflow_history(
        &self,
        owner: &str,
        repo: &str,
        workflow_file: &str,
        run_count: usize,
    ) -> Result<PipelineStatistics> {
        println!("Fetching {} workflow runs from GitHub...", run_count);

        let runs = self
            .fetch_workflow_runs(owner, repo, workflow_file, run_count)
            .await?;

        println!("Fetched {} runs, analyzing jobs...", runs.len());

        // Collect job data for all runs
        let mut job_data: std::collections::HashMap<String, Vec<Job>> =
            std::collections::HashMap::new();

        for (idx, run) in runs.iter().enumerate() {
            if idx % 10 == 0 {
                println!("Analyzing run {}/{}...", idx + 1, runs.len());
            }

            match self.fetch_jobs(owner, repo, run.id).await {
                Ok(jobs) => {
                    for job in jobs {
                        job_data
                            .entry(job.name.clone())
                            .or_insert_with(Vec::new)
                            .push(job);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to fetch jobs for run {}: {}", run.id, e);
                    continue;
                }
            }
        }

        // Calculate statistics per job
        let mut job_timings = Vec::new();
        let mut flaky_jobs = Vec::new();

        for (job_name, jobs) in job_data.iter() {
            let timing_data = Self::calculate_job_timing_stats(job_name, jobs);

            // Detect flaky jobs (high failure rate with variance)
            if timing_data.failure_count > 0
                && timing_data.success_count > 0
                && timing_data.variance > 2.0
            {
                flaky_jobs.push(job_name.clone());
            }

            job_timings.push(timing_data);
        }

        // Calculate overall pipeline statistics
        let completed_runs: Vec<&WorkflowRun> = runs
            .iter()
            .filter(|r| r.conclusion.is_some())
            .collect();

        let success_count = completed_runs
            .iter()
            .filter(|r| r.conclusion.as_deref() == Some("success"))
            .count();

        let durations: Vec<f64> = completed_runs
            .iter()
            .filter_map(|r| {
                let started = r.run_started_at?;
                let updated = r.updated_at;
                Some((updated - started).num_seconds() as f64)
            })
            .collect();

        let (avg, p50, p90, p99) = Self::calculate_percentiles(&durations);

        let workflow_name = runs
            .first()
            .map(|r| r.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(PipelineStatistics {
            workflow_name,
            total_runs: completed_runs.len(),
            success_rate: if completed_runs.is_empty() {
                0.0
            } else {
                success_count as f64 / completed_runs.len() as f64
            },
            avg_duration_sec: avg,
            p50_duration_sec: p50,
            p90_duration_sec: p90,
            p99_duration_sec: p99,
            job_timings,
            flaky_jobs,
        })
    }

    /// Calculate timing statistics for a single job
    fn calculate_job_timing_stats(job_name: &str, jobs: &[Job]) -> JobTimingData {
        let mut durations = Vec::new();
        let mut success_count = 0;
        let mut failure_count = 0;

        for job in jobs {
            match job.conclusion.as_deref() {
                Some("success") => success_count += 1,
                Some("failure") => failure_count += 1,
                _ => continue,
            }

            if let (Some(started), Some(completed)) = (job.started_at, job.completed_at) {
                let duration = (completed - started).num_seconds() as f64;
                if duration > 0.0 {
                    durations.push(duration);
                }
            }
        }

        let (avg, p50, p90, p99) = Self::calculate_percentiles(&durations);
        let variance = Self::calculate_variance(&durations, avg);

        JobTimingData {
            job_name: job_name.to_string(),
            durations_sec: durations,
            success_count,
            failure_count,
            avg_duration_sec: avg,
            p50_duration_sec: p50,
            p90_duration_sec: p90,
            p99_duration_sec: p99,
            variance,
        }
    }

    /// Calculate percentiles from duration data
    fn calculate_percentiles(durations: &[f64]) -> (f64, f64, f64, f64) {
        if durations.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let mut sorted = durations.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let avg = sorted.iter().sum::<f64>() / sorted.len() as f64;

        // Use nearest-rank method for percentiles
        let p50_idx = ((sorted.len() as f64 * 0.50).ceil() as usize).saturating_sub(1);
        let p90_idx = ((sorted.len() as f64 * 0.90).ceil() as usize).saturating_sub(1);
        let p99_idx = ((sorted.len() as f64 * 0.99).ceil() as usize).saturating_sub(1);

        let p50 = sorted[p50_idx.min(sorted.len() - 1)];
        let p90 = sorted[p90_idx.min(sorted.len() - 1)];
        let p99 = sorted[p99_idx.min(sorted.len() - 1)];

        (avg, p50, p90, p99)
    }

    /// Calculate variance
    fn calculate_variance(durations: &[f64], mean: f64) -> f64 {
        if durations.len() < 2 {
            return 0.0;
        }

        let sum_sq_diff: f64 = durations.iter().map(|d| (d - mean).powi(2)).sum();
        sum_sq_diff / durations.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile_calculation() {
        let durations = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
        let (avg, p50, p90, p99) = GitHubClient::calculate_percentiles(&durations);

        assert_eq!(avg, 55.0);
        assert_eq!(p50, 50.0);
        assert_eq!(p90, 90.0);
        assert_eq!(p99, 100.0);
    }

    #[test]
    fn test_variance_calculation() {
        let durations = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let mean = 30.0;
        let variance = GitHubClient::calculate_variance(&durations, mean);

        assert_eq!(variance, 200.0);
    }
}
