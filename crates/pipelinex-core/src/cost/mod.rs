use serde::{Deserialize, Serialize};

/// GitHub Actions pricing per minute by runner type.
#[derive(Debug, Clone)]
pub struct RunnerPricing {
    pub linux_per_min: f64,
    pub macos_per_min: f64,
    pub windows_per_min: f64,
}

impl Default for RunnerPricing {
    fn default() -> Self {
        Self {
            linux_per_min: 0.008,
            macos_per_min: 0.08,
            windows_per_min: 0.016,
        }
    }
}

/// Cost estimate for a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub compute_cost_per_run: f64,
    pub monthly_compute_cost: f64,
    pub monthly_developer_hours_lost: f64,
    pub monthly_opportunity_cost: f64,
    pub waste_ratio: f64,
}

/// Estimate costs for a pipeline based on timing and run frequency.
pub fn estimate_costs(
    duration_secs: f64,
    optimized_secs: f64,
    runs_per_month: u32,
    runner_type: &str,
    developer_hourly_rate: f64,
    team_size: u32,
) -> CostEstimate {
    let pricing = RunnerPricing::default();

    let rate_per_min = match runner_type {
        r if r.contains("macos") => pricing.macos_per_min,
        r if r.contains("windows") => pricing.windows_per_min,
        _ => pricing.linux_per_min,
    };

    let duration_min = duration_secs / 60.0;
    let compute_cost_per_run = duration_min * rate_per_min;
    let monthly_compute_cost = compute_cost_per_run * runs_per_month as f64;

    // Developer time lost = waiting time per run * runs per dev per month
    let runs_per_dev = runs_per_month as f64 / team_size as f64;
    let wait_hours_per_dev = (duration_secs * runs_per_dev) / 3600.0;
    let monthly_developer_hours_lost = wait_hours_per_dev * team_size as f64;
    let monthly_opportunity_cost = monthly_developer_hours_lost * developer_hourly_rate;

    let savings_secs = (duration_secs - optimized_secs).max(0.0);
    let waste_ratio = if duration_secs > 0.0 {
        savings_secs / duration_secs
    } else {
        0.0
    };

    CostEstimate {
        compute_cost_per_run,
        monthly_compute_cost,
        monthly_developer_hours_lost,
        monthly_opportunity_cost,
        waste_ratio,
    }
}
