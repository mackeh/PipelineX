use colored::*;
use pipelinex_core::analyzer::report::{AnalysisReport, Finding, Severity, format_duration};
use pipelinex_core::cost::CostEstimate;
use pipelinex_core::health_score::HealthScore;
use pipelinex_core::simulator::SimulationResult;
use pipelinex_core::optimizer::docker_opt::{DockerAnalysis, DockerSeverity};
use pipelinex_core::test_selector::TestSelection;
use pipelinex_core::flaky_detector::{FlakyReport, FlakyCategory};
use similar::{ChangeTag, TextDiff};
use std::path::{Path, PathBuf};

/// Print a full analysis report to the terminal.
pub fn print_analysis_report(report: &AnalysisReport) {
    println!();
    println!(
        "{}",
        format!(" PipelineX v{} — Analyzing {}", env!("CARGO_PKG_VERSION"), report.source_file).bold()
    );
    println!();

    // Pipeline structure summary
    println!(" {}", "Pipeline Structure".bold().underline());
    println!(
        " {} {} jobs, {} steps",
        "|-".dimmed(),
        report.job_count,
        report.step_count
    );
    println!(
        " {} Max parallelism: {}",
        "|-".dimmed(),
        report.max_parallelism
    );
    println!(
        " {} Critical path: {} ({})",
        "|-".dimmed(),
        report.critical_path.join(" -> "),
        format_duration(report.critical_path_duration_secs)
    );
    println!(
        " {} Provider: {}",
        "|-".dimmed(),
        report.provider.cyan()
    );
    println!();

    // Separator
    println!(" {}", "=".repeat(60).dimmed());
    println!();

    // Findings
    if report.findings.is_empty() {
        println!(
            " {} {}",
            "OK".green().bold(),
            "No significant bottlenecks detected. Your pipeline looks good!"
        );
    } else {
        for finding in &report.findings {
            print_finding(finding);
            println!();
        }
    }

    // Separator
    println!(" {}", "=".repeat(60).dimmed());
    println!();

    // Summary
    println!(" {}", "Summary".bold().underline());
    println!(
        " {} Current est. pipeline time:    {}",
        "|-".dimmed(),
        format_duration(report.total_estimated_duration_secs)
    );
    println!(
        " {} Optimized projection:          {}",
        "|-".dimmed(),
        format_duration(report.optimized_duration_secs).green()
    );
    println!(
        " {} Potential time savings:        {:.1}%",
        "|-".dimmed(),
        report.potential_improvement_pct()
    );

    let critical = report.critical_count();
    let high = report.high_count();
    let medium = report.medium_count();
    println!(
        " {} Findings: {} critical, {} high, {} medium",
        "|-".dimmed(),
        if critical > 0 {
            critical.to_string().red().bold().to_string()
        } else {
            "0".to_string()
        },
        if high > 0 {
            high.to_string().yellow().bold().to_string()
        } else {
            "0".to_string()
        },
        medium,
    );

    // Health score
    if let Some(ref health) = report.health_score {
        println!(
            " {} Pipeline Health: {} {}/100 ({})",
            "|-".dimmed(),
            health.grade.emoji(),
            format!("{:.0}", health.total_score).bold(),
            health.grade.label().cyan()
        );
    }
    println!();

    if !report.findings.is_empty() {
        println!(
            " Run {} to generate optimized config",
            format!("pipelinex optimize {}", report.source_file).cyan()
        );
        println!(
            " Run {} to see changes",
            format!("pipelinex diff {}", report.source_file).cyan()
        );
        println!(
            " Run {} to simulate timing",
            format!("pipelinex simulate {}", report.source_file).cyan()
        );
        println!(
            " Run {} to visualize the DAG",
            format!("pipelinex graph {}", report.source_file).cyan()
        );
    }
    println!();
}

fn print_finding(finding: &Finding) {
    let severity_tag = match finding.severity {
        Severity::Critical => format!(" {} ", finding.severity.symbol()).on_red().white().bold().to_string(),
        Severity::High => format!(" {} ", finding.severity.symbol()).on_yellow().black().bold().to_string(),
        Severity::Medium => format!(" {} ", finding.severity.symbol()).on_blue().white().bold().to_string(),
        Severity::Low => format!(" {} ", finding.severity.symbol()).dimmed().to_string(),
        Severity::Info => format!(" {} ", finding.severity.symbol()).dimmed().to_string(),
    };

    println!(" {} {}", severity_tag, finding.title.bold());
    println!("   {} {}", "|".dimmed(), finding.description);

    if let Some(savings) = finding.estimated_savings_secs {
        println!(
            "   {} Estimated savings: {}/run",
            "|".dimmed(),
            format_duration(savings).green()
        );
    }

    println!(
        "   {} Confidence: {:.0}%{}",
        "|".dimmed(),
        finding.confidence * 100.0,
        if finding.auto_fixable {
            " | Auto-fixable".green().to_string()
        } else {
            String::new()
        }
    );

    println!("   {} {}", "|".dimmed(), finding.recommendation.dimmed());

    if let Some(cmd) = &finding.fix_command {
        println!("   {} Fix: {}", "|".dimmed(), cmd.cyan());
    }
}

/// Print a diff between original and optimized pipeline.
pub fn print_diff(original: &str, optimized: &str, filename: &str) {
    println!();
    println!(
        "{}",
        format!(" PipelineX — Diff for {}", filename).bold()
    );
    println!();

    let diff = TextDiff::from_lines(original, optimized);
    let mut has_changes = false;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                has_changes = true;
                print!("{}", format!("- {}", change).red());
            }
            ChangeTag::Insert => {
                has_changes = true;
                print!("{}", format!("+ {}", change).green());
            }
            ChangeTag::Equal => {
                print!("  {}", change);
            }
        }
    }

    if !has_changes {
        println!(" {}", "No changes needed — pipeline is already well-optimized!".green());
    }
    println!();
}

/// Print a cost estimate report.
pub fn print_cost_report(
    file: &Path,
    report: &AnalysisReport,
    estimate: &CostEstimate,
    runs_per_month: u32,
    team_size: u32,
) {
    println!();
    println!(
        "{}",
        format!(" PipelineX Cost Report — {}", file.display()).bold()
    );
    println!();

    println!(
        " {} Pipeline runs/month:        {}",
        "|-".dimmed(),
        runs_per_month
    );
    println!(
        " {} Team size:                  {} developers",
        "|-".dimmed(),
        team_size
    );
    println!(
        " {} Current pipeline time:      {}",
        "|-".dimmed(),
        format_duration(report.total_estimated_duration_secs)
    );
    println!(
        " {} Optimized pipeline time:    {}",
        "|-".dimmed(),
        format_duration(report.optimized_duration_secs).green()
    );
    println!();

    println!(" {}", "Cost Breakdown".bold().underline());
    println!(
        "   Compute cost per run:         ${:.3}",
        estimate.compute_cost_per_run
    );
    println!(
        "   Monthly compute cost:         ${:.2}",
        estimate.monthly_compute_cost
    );
    println!(
        "   Developer hours lost/month:   {:.1} hours",
        estimate.monthly_developer_hours_lost
    );
    println!(
        "   Opportunity cost/month:       {}",
        format!("${:.0}", estimate.monthly_opportunity_cost).red()
    );
    println!(
        "   Waste ratio:                  {:.1}%",
        estimate.waste_ratio * 100.0
    );
    println!();

    let recoverable_compute = estimate.monthly_compute_cost * estimate.waste_ratio;
    let recoverable_dev_hours = estimate.monthly_developer_hours_lost * estimate.waste_ratio;
    println!(" {}", "Recoverable Savings".bold().underline());
    println!(
        "   Monthly compute savings:      {}",
        format!("${:.2}", recoverable_compute).green()
    );
    println!(
        "   Monthly dev hours saved:      {}",
        format!("{:.1} hours", recoverable_dev_hours).green()
    );
    println!(
        "   Annual savings:               {}",
        format!("${:.0}", (recoverable_compute + recoverable_dev_hours * 150.0) * 12.0)
            .green()
            .bold()
    );
    println!();
}

/// Print Monte Carlo simulation results.
pub fn print_simulation_report(pipeline_name: &str, result: &SimulationResult) {
    println!();
    println!(
        "{}",
        format!(" PipelineX Simulation — {} ({} runs)", pipeline_name, result.runs).bold()
    );
    println!();

    // Duration distribution
    println!(" {}", "Duration Distribution".bold().underline());
    println!(
        "   Min:     {}",
        format_duration(result.min_duration_secs)
    );
    println!(
        "   p50:     {}",
        format_duration(result.p50_duration_secs).green()
    );
    println!(
        "   p75:     {}",
        format_duration(result.p75_duration_secs)
    );
    println!(
        "   p90:     {}",
        format_duration(result.p90_duration_secs).yellow()
    );
    println!(
        "   p99:     {}",
        format_duration(result.p99_duration_secs).red()
    );
    println!(
        "   Max:     {}",
        format_duration(result.max_duration_secs)
    );
    println!(
        "   Mean:    {} (std dev: {})",
        format_duration(result.mean_duration_secs),
        format_duration(result.std_dev_secs)
    );
    println!();

    // Histogram
    println!(" {}", "Timing Histogram".bold().underline());
    for bucket in &result.histogram {
        if bucket.count > 0 {
            let label = format!(
                "   {:>6} - {:>6}",
                format_duration(bucket.lower_bound_secs),
                format_duration(bucket.upper_bound_secs)
            );
            let bar = "#".repeat(bucket.bar.len()).blue().to_string();
            println!("{} {} {}", label, bar, bucket.count);
        }
    }
    println!();

    // Job stats
    if !result.job_stats.is_empty() {
        println!(" {}", "Job Analysis".bold().underline());
        println!(
            "   {:<20} {:>8} {:>8} {:>8} {:>10}",
            "Job".underline(),
            "Mean".underline(),
            "p50".underline(),
            "p90".underline(),
            "Crit.Path%".underline()
        );
        for job in &result.job_stats {
            let crit_color = if job.on_critical_path_pct > 80.0 {
                format!("{:.0}%", job.on_critical_path_pct).red().to_string()
            } else if job.on_critical_path_pct > 50.0 {
                format!("{:.0}%", job.on_critical_path_pct).yellow().to_string()
            } else {
                format!("{:.0}%", job.on_critical_path_pct)
            };

            println!(
                "   {:<20} {:>8} {:>8} {:>8} {:>10}",
                job.job_id,
                format_duration(job.mean_duration_secs),
                format_duration(job.p50_duration_secs),
                format_duration(job.p90_duration_secs),
                crit_color,
            );
        }
    }
    println!();
}

/// Print Docker analysis results.
pub fn print_docker_analysis(path: &Path, analysis: &DockerAnalysis) {
    println!();
    println!(
        "{}",
        format!(" PipelineX Docker Analysis — {}", path.display()).bold()
    );
    println!();

    println!(
        " {} Est. build time (current):   {}",
        "|-".dimmed(),
        format_duration(analysis.estimated_build_time_before)
    );
    println!(
        " {} Est. build time (optimized): {}",
        "|-".dimmed(),
        format_duration(analysis.estimated_build_time_after).green()
    );
    println!();

    if analysis.findings.is_empty() {
        println!(" {} {}", "OK".green().bold(), "Dockerfile looks well-optimized!");
    } else {
        println!(" {}", "=".repeat(60).dimmed());
        println!();

        for finding in &analysis.findings {
            let tag = match finding.severity {
                DockerSeverity::Critical => " CRITICAL ".on_red().white().bold().to_string(),
                DockerSeverity::Warning => " WARNING ".on_yellow().black().bold().to_string(),
                DockerSeverity::Info => " INFO ".on_blue().white().to_string(),
            };

            println!(" {} {}", tag, finding.title.bold());
            println!("   {} {}", "|".dimmed(), finding.description);
            if let Some(line) = finding.line_number {
                println!("   {} Line: {}", "|".dimmed(), line);
            }
            println!("   {} Fix: {}", "|".dimmed(), finding.fix.cyan());
            println!();
        }

        println!(" {}", "=".repeat(60).dimmed());
        println!();
        println!(
            " Run {} to generate an optimized Dockerfile",
            format!("pipelinex docker {} --optimize", path.display()).cyan()
        );
    }
    println!();
}

/// Print test selection results to the terminal.
pub fn print_test_selection(selection: &TestSelection) {
    println!();
    println!(
        "{}",
        format!(" PipelineX v{} — Smart Test Selection", env!("CARGO_PKG_VERSION")).bold()
    );
    println!();

    // Changed files
    println!(" {}", "Changed Files".bold().underline());
    if selection.changed_files.is_empty() {
        println!(" {} No changes detected", "|".dimmed());
    } else {
        for (i, file) in selection.changed_files.iter().enumerate() {
            if i < 10 {
                println!(" {} {}", "|-".dimmed(), file.display());
            } else if i == 10 {
                println!(" {} ... ({} more files)", "|-".dimmed(), selection.changed_files.len() - 10);
                break;
            }
        }
    }
    println!();

    // Selected tests
    println!(" {}", "Selected Tests".bold().underline());
    if selection.selected_tests.is_empty() {
        println!(" {} No specific tests selected — run all tests", "|".dimmed());
    } else if selection.selected_tests.contains(&"all".to_string()) {
        println!(
            " {} {} Critical changes detected — running all tests",
            "|-".dimmed(),
            "⚠".yellow()
        );
    } else {
        for test in &selection.selected_tests {
            println!(" {} {}", "|-".dimmed(), test.cyan());
        }
    }
    println!();

    // Test patterns (for CI config)
    if !selection.test_patterns.is_empty() {
        println!(" {}", "Test Patterns (for CI config)".bold().underline());
        for pattern in &selection.test_patterns {
            println!(" {} {}", "|-".dimmed(), pattern.yellow());
        }
        println!();
    }

    // Selection ratio
    println!(" {}", "Selection Summary".bold().underline());
    if selection.selection_ratio > 0.0 {
        println!(
            " {} Running ~{:.0}% of tests based on changes",
            "|-".dimmed(),
            selection.selection_ratio * 100.0
        );
        println!(
            " {} Est. time savings: {:.0}%",
            "|-".dimmed(),
            (1.0 - selection.selection_ratio) * 100.0
        );
    } else {
        println!(" {} No tests selected — changes may not affect test code", "|-".dimmed());
    }
    println!();

    // Reasoning
    if !selection.reasoning.is_empty() {
        println!(" {}", "Reasoning".bold().underline());
        for reason in &selection.reasoning {
            println!(" {} {}", "|-".dimmed(), reason);
        }
        println!();
    }

    // Usage hints
    println!(" {}", "=".repeat(60).dimmed());
    println!();
    println!(" {} Integration with CI:", "Tip".green().bold());
    println!("  {} Use {} to get patterns as JSON/YAML",
        "|".dimmed(),
        "pipelinex select-tests --format json".cyan()
    );
    println!("  {} Configure your CI to run only the selected test patterns",
        "|".dimmed()
    );
    println!();
}

/// Print flaky test detection report to the terminal.
pub fn print_flaky_report(report: &FlakyReport, files: &[PathBuf]) {
    println!();
    println!(
        "{}",
        format!(" PipelineX v{} — Flaky Test Detector", env!("CARGO_PKG_VERSION")).bold()
    );
    println!();

    // Input files
    println!(" {}", "Input Files".bold().underline());
    for (i, file) in files.iter().enumerate() {
        if i < 5 {
            println!(" {} {}", "|-".dimmed(), file.display());
        } else if i == 5 {
            println!(" {} ... ({} more files)", "|-".dimmed(), files.len() - 5);
            break;
        }
    }
    println!();

    // Summary
    println!(" {}", "Detection Summary".bold().underline());
    println!(" {} Total tests analyzed: {}", "|-".dimmed(), report.total_tests);
    println!(
        " {} Flaky tests found: {}",
        "|-".dimmed(),
        if report.flaky_tests.is_empty() {
            format!("{}", report.flaky_tests.len()).green()
        } else {
            format!("{}", report.flaky_tests.len()).red()
        }
    );
    println!(
        " {} Flakiness ratio: {:.1}%",
        "|-".dimmed(),
        report.flakiness_ratio * 100.0
    );
    println!(
        " {} Confidence: {}",
        "|-".dimmed(),
        match report.confidence.as_str() {
            "High" => report.confidence.green(),
            "Medium" => report.confidence.yellow(),
            _ => report.confidence.red(),
        }
    );
    println!();

    if report.flaky_tests.is_empty() {
        println!(" {} {}", "✓".green().bold(), "No flaky tests detected! All tests are stable.".green());
        println!();
        return;
    }

    // Flaky tests details
    println!(" {}", "=".repeat(60).dimmed());
    println!();

    for (i, test) in report.flaky_tests.iter().enumerate() {
        if i >= 20 {
            println!(" ... and {} more flaky tests", report.flaky_tests.len() - 20);
            break;
        }

        let score_display = format!("{:.0}%", test.flakiness_score * 100.0);
        let score_colored = if test.flakiness_score >= 0.7 {
            score_display.red().bold()
        } else if test.flakiness_score >= 0.5 {
            score_display.yellow()
        } else {
            score_display.normal()
        };

        println!(" {} Flakiness: {}", "FLAKY".on_red().white().bold(), score_colored);
        println!("   {} {}", "|".dimmed(), test.name.bold());
        println!(
            "   {} Category: {}",
            "|".dimmed(),
            match test.category {
                FlakyCategory::Intermittent => "Intermittent (< 50% failure rate)".yellow(),
                FlakyCategory::Unstable => "Unstable (alternating pass/fail)".red(),
                FlakyCategory::EnvironmentSensitive => "Environment-Sensitive (network, timeouts)".cyan(),
                FlakyCategory::TimingDependent => "Timing-Dependent (race conditions)".magenta(),
            }
        );
        println!(
            "   {} Runs: {} | Passed: {} | Failed: {} | Failure rate: {:.1}%",
            "|".dimmed(),
            test.total_runs,
            test.passes.to_string().green(),
            test.failures.to_string().red(),
            test.failure_rate * 100.0
        );

        if !test.recent_failures.is_empty() {
            println!("   {} Recent failures:", "|".dimmed());
            for (j, error) in test.recent_failures.iter().enumerate() {
                if j >= 2 {
                    break;
                }
                let truncated = if error.len() > 80 {
                    format!("{}...", &error[..77])
                } else {
                    error.clone()
                };
                println!("   {}   - {}", "|".dimmed(), truncated.dimmed());
            }
        }
        println!();
    }

    println!(" {}", "=".repeat(60).dimmed());
    println!();

    // Recommendations
    println!(" {}", "Recommendations".bold().underline());
    println!(" {} Quarantine flaky tests to prevent blocking CI", "|-".dimmed());
    println!(" {} Investigate timing-dependent tests for race conditions", "|-".dimmed());
    println!(" {} Add retries for environment-sensitive tests", "|-".dimmed());
    println!(" {} Track flakiness over time to identify trends", "|-".dimmed());
    println!();

    println!(" {} Next steps:", "Tip".green().bold());
    println!("  {} Run {} to get JSON output",
        "|".dimmed(),
        "pipelinex flaky <path> --format json".cyan()
    );
    println!("  {} Integrate with your CI to automatically detect new flaky tests",
        "|".dimmed()
    );
    println!();
}

use pipelinex_core::providers::github_api::PipelineStatistics;

pub fn print_history_stats(stats: &PipelineStatistics) {
    use colored::Colorize;

    println!("{}", "━".repeat(70).bright_black());
    println!("{}", format!("📊 Pipeline History: {}", stats.workflow_name).bold().cyan());
    println!("{}", "━".repeat(70).bright_black());
    println!();

    // Overall statistics
    println!("{}", " Overall Statistics".bold());
    println!("   Total runs analyzed:  {}", stats.total_runs.to_string().yellow());
    println!("   Success rate:         {:.1}%", (stats.success_rate * 100.0).to_string().green());
    println!();

    // Duration statistics
    println!("{}", " Duration Statistics".bold());
    println!("   Average:   {}", format_duration(stats.avg_duration_sec).yellow());
    println!("   Median:    {}", format_duration(stats.p50_duration_sec).yellow());
    println!("   P90:       {}", format_duration(stats.p90_duration_sec).yellow());
    println!("   P99:       {}", format_duration(stats.p99_duration_sec).yellow());
    println!();

    // Job-level statistics
    if !stats.job_timings.is_empty() {
        println!("{}", " Job Performance".bold());
        println!();

        let mut jobs = stats.job_timings.clone();
        jobs.sort_by(|a, b| b.avg_duration_sec.partial_cmp(&a.avg_duration_sec).unwrap());

        for job in jobs.iter().take(10) {
            let total_runs = job.success_count + job.failure_count;
            let success_rate = if total_runs > 0 {
                job.success_count as f64 / total_runs as f64 * 100.0
            } else {
                0.0
            };

            let job_label = if job.failure_count > 0 && job.success_count > 0 {
                format!("🟡 {}", job.job_name).yellow()
            } else if job.failure_count > 0 {
                format!("🔴 {}", job.job_name).red()
            } else {
                format!("✅ {}", job.job_name).green()
            };

            println!("   {}", job_label);
            println!("      Average: {} | P50: {} | P90: {}",
                format_duration(job.avg_duration_sec).bright_white(),
                format_duration(job.p50_duration_sec).bright_black(),
                format_duration(job.p90_duration_sec).bright_black()
            );
            println!("      Runs: {} | Success rate: {:.1}%",
                total_runs.to_string().bright_black(),
                format!("{:.1}", success_rate)
            );

            // Show variance indicator
            if job.variance > 4.0 {
                println!("      ⚠️  {} (high variance detected)",
                    "Unstable timing".yellow()
                );
            }
            println!();
        }
    }

    // Flaky jobs
    if !stats.flaky_jobs.is_empty() {
        println!("{}", " ⚠️  Potentially Flaky Jobs".bold().yellow());
        for flaky_job in &stats.flaky_jobs {
            println!("   • {}", flaky_job.red());
        }
        println!();
    }

    println!("{}", "━".repeat(70).bright_black());
    println!("{}", " 💡 Insights".bold().cyan());
    println!("{}", "━".repeat(70).bright_black());
    println!();

    // Provide insights
    if stats.p90_duration_sec > stats.avg_duration_sec * 1.5 {
        println!("   {} P90 is significantly higher than average",
            "🔴".red()
        );
        println!("      This indicates high variance in pipeline duration.");
        println!("      Consider investigating slow runs for bottlenecks.");
        println!();
    }

    if stats.success_rate < 0.9 {
        println!("   {} Success rate below 90%",
            "🔴".red()
        );
        println!("      {} of runs fail. Identify and fix flaky tests or unstable jobs.",
            format!("{:.0}%", (1.0 - stats.success_rate) * 100.0)
        );
        println!();
    }

    if !stats.flaky_jobs.is_empty() {
        println!("   {} {} potentially flaky jobs detected",
            "🟡".yellow(),
            stats.flaky_jobs.len()
        );
        println!("      Run 'pipelinex flaky' with JUnit reports to analyze test-level flakiness.");
        println!();
    }

    println!("{}", "━".repeat(70).bright_black());
    println!();
    println!("{}", " Use this data to:".bright_white());
    println!("   • Identify the slowest jobs for optimization");
    println!("   • Spot flaky or unstable jobs");
    println!("   • Track performance trends over time");
    println!("   • Validate that optimizations reduced duration");
    println!();
}

