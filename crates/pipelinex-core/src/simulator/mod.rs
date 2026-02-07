use crate::parser::dag::PipelineDag;
use petgraph::graph::NodeIndex;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of a Monte Carlo simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub runs: usize,
    pub p50_duration_secs: f64,
    pub p75_duration_secs: f64,
    pub p90_duration_secs: f64,
    pub p99_duration_secs: f64,
    pub mean_duration_secs: f64,
    pub min_duration_secs: f64,
    pub max_duration_secs: f64,
    pub std_dev_secs: f64,
    /// Per-job timing statistics
    pub job_stats: Vec<JobSimStats>,
    /// Distribution histogram buckets (for visualization)
    pub histogram: Vec<HistogramBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSimStats {
    pub job_id: String,
    pub mean_duration_secs: f64,
    pub p50_duration_secs: f64,
    pub p90_duration_secs: f64,
    pub on_critical_path_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    pub lower_bound_secs: f64,
    pub upper_bound_secs: f64,
    pub count: usize,
    pub bar: String,
}

/// Simple pseudo-random number generator (xorshift64) — no external dependency needed.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: if seed == 0 { 1 } else { seed } }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// Generate a uniform random f64 in [0, 1)
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Generate a normally-distributed random f64 using Box-Muller transform.
    fn next_normal(&mut self, mean: f64, std_dev: f64) -> f64 {
        let u1 = self.next_f64().max(1e-10); // Avoid log(0)
        let u2 = self.next_f64();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        mean + z * std_dev
    }
}

/// Run a Monte Carlo simulation of the pipeline.
///
/// Each run samples job durations from a normal distribution around their
/// estimated duration (with configurable variance), then computes the total
/// pipeline time by finding the critical path through the sampled DAG.
pub fn simulate(dag: &PipelineDag, num_runs: usize, variance_factor: f64) -> SimulationResult {
    let mut rng = Rng::new(42);
    let mut run_durations: Vec<f64> = Vec::with_capacity(num_runs);
    let mut job_durations: HashMap<String, Vec<f64>> = HashMap::new();
    let mut job_critical_count: HashMap<String, usize> = HashMap::new();

    // Initialize tracking
    for job in dag.graph.node_weights() {
        job_durations.insert(job.id.clone(), Vec::with_capacity(num_runs));
        job_critical_count.insert(job.id.clone(), 0);
    }

    let topo = match petgraph::algo::toposort(&dag.graph, None) {
        Ok(t) => t,
        Err(_) => return empty_result(num_runs),
    };

    for _ in 0..num_runs {
        // Sample durations for each job
        let mut sampled: HashMap<NodeIndex, f64> = HashMap::new();
        for idx in dag.graph.node_indices() {
            let job = &dag.graph[idx];
            let base = job.estimated_duration_secs;
            let std_dev = base * variance_factor;
            let duration = rng.next_normal(base, std_dev).max(base * 0.1); // Floor at 10% of base
            sampled.insert(idx, duration);

            job_durations.get_mut(&job.id).unwrap().push(duration);
        }

        // Compute critical path for this run
        let mut finish_time: HashMap<NodeIndex, f64> = HashMap::new();
        let mut predecessor: HashMap<NodeIndex, Option<NodeIndex>> = HashMap::new();

        for &node in &topo {
            let deps: Vec<_> = dag.graph.neighbors_directed(node, Direction::Incoming).collect();
            let start_time = deps.iter()
                .map(|dep| finish_time.get(dep).copied().unwrap_or(0.0))
                .fold(0.0f64, f64::max);

            let duration = sampled[&node];
            finish_time.insert(node, start_time + duration);

            let pred = deps.iter()
                .max_by(|a, b| {
                    finish_time.get(a).unwrap_or(&0.0)
                        .partial_cmp(finish_time.get(b).unwrap_or(&0.0))
                        .unwrap()
                })
                .copied();
            predecessor.insert(node, pred);
        }

        let total = finish_time.values().fold(0.0f64, |a, &b| a.max(b));
        run_durations.push(total);

        // Track which jobs are on the critical path
        if let Some((&end_node, _)) = finish_time.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        {
            let mut current = end_node;
            job_critical_count.entry(dag.graph[current].id.clone()).and_modify(|c| *c += 1);
            while let Some(Some(pred)) = predecessor.get(&current) {
                job_critical_count.entry(dag.graph[*pred].id.clone()).and_modify(|c| *c += 1);
                current = *pred;
            }
        }
    }

    // Compute statistics
    run_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mean = run_durations.iter().sum::<f64>() / num_runs as f64;
    let variance = run_durations.iter()
        .map(|d| (d - mean).powi(2))
        .sum::<f64>() / num_runs as f64;
    let std_dev = variance.sqrt();

    let p50 = percentile(&run_durations, 50.0);
    let p75 = percentile(&run_durations, 75.0);
    let p90 = percentile(&run_durations, 90.0);
    let p99 = percentile(&run_durations, 99.0);

    // Job stats
    let mut job_stats: Vec<JobSimStats> = Vec::new();
    for job in dag.graph.node_weights() {
        let durations = job_durations.get(&job.id).unwrap();
        let mut sorted = durations.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let job_mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
        let critical_pct = *job_critical_count.get(&job.id).unwrap_or(&0) as f64 / num_runs as f64 * 100.0;

        job_stats.push(JobSimStats {
            job_id: job.id.clone(),
            mean_duration_secs: job_mean,
            p50_duration_secs: percentile(&sorted, 50.0),
            p90_duration_secs: percentile(&sorted, 90.0),
            on_critical_path_pct: critical_pct,
        });
    }

    // Sort job stats by critical path percentage (most critical first)
    job_stats.sort_by(|a, b| b.on_critical_path_pct.partial_cmp(&a.on_critical_path_pct).unwrap());

    // Build histogram
    let histogram = build_histogram(&run_durations, 20);

    SimulationResult {
        runs: num_runs,
        p50_duration_secs: p50,
        p75_duration_secs: p75,
        p90_duration_secs: p90,
        p99_duration_secs: p99,
        mean_duration_secs: mean,
        min_duration_secs: run_durations.first().copied().unwrap_or(0.0),
        max_duration_secs: run_durations.last().copied().unwrap_or(0.0),
        std_dev_secs: std_dev,
        job_stats,
        histogram,
    }
}

fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (pct / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn build_histogram(sorted: &[f64], num_buckets: usize) -> Vec<HistogramBucket> {
    if sorted.is_empty() {
        return Vec::new();
    }

    let min = sorted.first().unwrap();
    let max = sorted.last().unwrap();
    let range = max - min;
    if range == 0.0 {
        return vec![HistogramBucket {
            lower_bound_secs: *min,
            upper_bound_secs: *max,
            count: sorted.len(),
            bar: "#".repeat(40),
        }];
    }

    let bucket_width = range / num_buckets as f64;
    let mut buckets = Vec::with_capacity(num_buckets);

    for i in 0..num_buckets {
        let lower = min + i as f64 * bucket_width;
        let upper = lower + bucket_width;
        let count = sorted.iter()
            .filter(|&&v| {
                if i == num_buckets - 1 {
                    v >= lower
                } else {
                    v >= lower && v < upper
                }
            })
            .count();
        buckets.push(HistogramBucket {
            lower_bound_secs: lower,
            upper_bound_secs: upper,
            count,
            bar: String::new(), // Filled below
        });
    }

    let max_count = buckets.iter().map(|b| b.count).max().unwrap_or(1);
    for bucket in &mut buckets {
        let bar_len = if max_count > 0 {
            (bucket.count as f64 / max_count as f64 * 40.0).round() as usize
        } else {
            0
        };
        bucket.bar = "#".repeat(bar_len);
    }

    buckets
}

fn empty_result(runs: usize) -> SimulationResult {
    SimulationResult {
        runs,
        p50_duration_secs: 0.0,
        p75_duration_secs: 0.0,
        p90_duration_secs: 0.0,
        p99_duration_secs: 0.0,
        mean_duration_secs: 0.0,
        min_duration_secs: 0.0,
        max_duration_secs: 0.0,
        std_dev_secs: 0.0,
        job_stats: Vec::new(),
        histogram: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::github::GitHubActionsParser;

    #[test]
    fn test_simulation_produces_results() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: npm run build
  test:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - run: npm test
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let result = simulate(&dag, 1000, 0.15);

        assert_eq!(result.runs, 1000);
        assert!(result.mean_duration_secs > 0.0);
        assert!(result.p50_duration_secs > 0.0);
        assert!(result.p90_duration_secs >= result.p50_duration_secs);
        assert!(result.p99_duration_secs >= result.p90_duration_secs);
        assert_eq!(result.job_stats.len(), 2);
        assert!(!result.histogram.is_empty());
    }

    #[test]
    fn test_simulation_parallel_is_faster() {
        // Serial: A -> B -> C
        let serial_yaml = r#"
name: Serial
on: push
jobs:
  a:
    runs-on: ubuntu-latest
    steps:
      - run: npm test
  b:
    needs: a
    runs-on: ubuntu-latest
    steps:
      - run: npm test
  c:
    needs: b
    runs-on: ubuntu-latest
    steps:
      - run: npm test
"#;
        // Parallel: A, B, C all independent
        let parallel_yaml = r#"
name: Parallel
on: push
jobs:
  a:
    runs-on: ubuntu-latest
    steps:
      - run: npm test
  b:
    runs-on: ubuntu-latest
    steps:
      - run: npm test
  c:
    runs-on: ubuntu-latest
    steps:
      - run: npm test
"#;
        let serial_dag = GitHubActionsParser::parse(serial_yaml, "s.yml".to_string()).unwrap();
        let parallel_dag = GitHubActionsParser::parse(parallel_yaml, "p.yml".to_string()).unwrap();

        let serial_result = simulate(&serial_dag, 500, 0.1);
        let parallel_result = simulate(&parallel_dag, 500, 0.1);

        assert!(parallel_result.mean_duration_secs < serial_result.mean_duration_secs);
    }
}
