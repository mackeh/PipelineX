use colored::*;
use pipelinex_core::analyzer::report::{AnalysisReport, Finding, Severity, format_duration};
use pipelinex_core::cost::CostEstimate;
use pipelinex_core::simulator::SimulationResult;
use pipelinex_core::optimizer::docker_opt::{DockerAnalysis, DockerSeverity};
use similar::{ChangeTag, TextDiff};
use std::path::Path;

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
