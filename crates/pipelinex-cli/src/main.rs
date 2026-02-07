mod display;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pipelinex_core::parser::github::GitHubActionsParser;
use pipelinex_core::analyzer;
use pipelinex_core::optimizer::Optimizer;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pipelinex",
    version,
    about = "PipelineX — CI/CD Bottleneck Analyzer & Auto-Optimizer",
    long_about = "Analyze your CI/CD pipelines, identify bottlenecks, and generate optimized configurations.\n\nYour pipelines are slow. PipelineX knows why — and fixes them automatically."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze pipeline configuration for bottlenecks and optimization opportunities
    Analyze {
        /// Path to workflow file or directory containing workflow files
        #[arg(default_value = ".github/workflows/")]
        path: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Generate an optimized pipeline configuration
    Optimize {
        /// Path to the workflow file to optimize
        path: PathBuf,

        /// Output file path (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Show diff between original and optimized
        #[arg(long)]
        diff: bool,
    },

    /// Show diff between current and optimized pipeline
    Diff {
        /// Path to the workflow file
        path: PathBuf,
    },

    /// Estimate CI/CD costs and potential savings
    Cost {
        /// Path to workflow file or directory
        #[arg(default_value = ".github/workflows/")]
        path: PathBuf,

        /// Estimated pipeline runs per month
        #[arg(long, default_value = "500")]
        runs_per_month: u32,

        /// Team size (number of developers)
        #[arg(long, default_value = "10")]
        team_size: u32,

        /// Average fully-loaded developer hourly rate in USD
        #[arg(long, default_value = "150")]
        hourly_rate: f64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path, format } => cmd_analyze(&path, &format),
        Commands::Optimize { path, output, diff } => cmd_optimize(&path, output.as_deref(), diff),
        Commands::Diff { path } => cmd_diff(&path),
        Commands::Cost { path, runs_per_month, team_size, hourly_rate } => {
            cmd_cost(&path, runs_per_month, team_size, hourly_rate)
        }
    }
}

fn discover_workflow_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.clone()]);
    }

    if path.is_dir() {
        let pattern = format!("{}/**/*.yml", path.display());
        let mut files: Vec<PathBuf> = glob::glob(&pattern)
            .context("Failed to read glob pattern")?
            .chain(
                glob::glob(&format!("{}/**/*.yaml", path.display()))
                    .context("Failed to read glob pattern")?
            )
            .filter_map(|r| r.ok())
            .collect();
        files.sort();
        return Ok(files);
    }

    anyhow::bail!("Path '{}' does not exist", path.display());
}

fn cmd_analyze(path: &PathBuf, format: &str) -> Result<()> {
    let files = discover_workflow_files(path)?;

    if files.is_empty() {
        anyhow::bail!(
            "No workflow files found at '{}'. \
            Make sure the path points to a YAML workflow file or directory.",
            path.display()
        );
    }

    for file in &files {
        let dag = GitHubActionsParser::parse_file(file)
            .with_context(|| format!("Failed to parse {}", file.display()))?;

        let report = analyzer::analyze(&dag);

        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&report)?;
                println!("{}", json);
            }
            _ => {
                display::print_analysis_report(&report);
            }
        }
    }

    Ok(())
}

fn cmd_optimize(path: &PathBuf, output: Option<&std::path::Path>, show_diff: bool) -> Result<()> {
    if !path.is_file() {
        anyhow::bail!("'{}' is not a file. Optimize requires a single workflow file.", path.display());
    }

    let dag = GitHubActionsParser::parse_file(path)?;
    let report = analyzer::analyze(&dag);
    let optimized = Optimizer::optimize(path, &report)?;

    if show_diff {
        let original = std::fs::read_to_string(path)?;
        display::print_diff(&original, &optimized, &path.to_string_lossy());
        return Ok(());
    }

    match output {
        Some(out_path) => {
            std::fs::write(out_path, &optimized)?;
            println!(
                "Optimized config written to {}",
                out_path.display()
            );
        }
        None => {
            print!("{}", optimized);
        }
    }

    Ok(())
}

fn cmd_diff(path: &PathBuf) -> Result<()> {
    cmd_optimize(path, None, true)
}

fn cmd_cost(path: &PathBuf, runs_per_month: u32, team_size: u32, hourly_rate: f64) -> Result<()> {
    let files = discover_workflow_files(path)?;

    if files.is_empty() {
        anyhow::bail!("No workflow files found at '{}'", path.display());
    }

    for file in &files {
        let dag = GitHubActionsParser::parse_file(file)?;
        let report = analyzer::analyze(&dag);

        let runner_type = dag.graph.node_weights()
            .next()
            .map(|j| j.runs_on.as_str())
            .unwrap_or("ubuntu-latest");

        let estimate = pipelinex_core::cost::estimate_costs(
            report.total_estimated_duration_secs,
            report.optimized_duration_secs,
            runs_per_month,
            runner_type,
            hourly_rate,
            team_size,
        );

        display::print_cost_report(file, &report, &estimate, runs_per_month, team_size);
    }

    Ok(())
}
