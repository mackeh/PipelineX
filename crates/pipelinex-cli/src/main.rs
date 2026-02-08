mod display;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pipelinex_core::analyzer;
use pipelinex_core::flaky_detector::FlakyDetector;
use pipelinex_core::optimizer::Optimizer;
use pipelinex_core::parser::bitbucket::BitbucketParser;
use pipelinex_core::parser::circleci::CircleCIParser;
use pipelinex_core::parser::github::GitHubActionsParser;
use pipelinex_core::parser::gitlab::GitLabCIParser;
use pipelinex_core::parser::jenkins::JenkinsParser;
use pipelinex_core::providers::GitHubClient;
use pipelinex_core::test_selector::TestSelector;
use std::path::{Path, PathBuf};

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

        /// Output format (text, json, sarif, html)
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

    /// Generate a visual pipeline DAG diagram
    Graph {
        /// Path to workflow file
        path: PathBuf,

        /// Output format (mermaid, dot, ascii)
        #[arg(short, long, default_value = "mermaid")]
        format: String,

        /// Output file path (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run Monte Carlo simulation of pipeline timing
    Simulate {
        /// Path to workflow file
        path: PathBuf,

        /// Number of simulation runs
        #[arg(long, default_value = "1000")]
        runs: usize,

        /// Variance factor for timing (0.0 = deterministic, 0.3 = high variance)
        #[arg(long, default_value = "0.15")]
        variance: f64,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Analyze a Dockerfile for optimization opportunities
    Docker {
        /// Path to Dockerfile
        #[arg(default_value = "Dockerfile")]
        path: PathBuf,

        /// Output optimized Dockerfile
        #[arg(long)]
        optimize: bool,

        /// Output file path for optimized Dockerfile
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Select tests to run based on code changes (smart test selection)
    SelectTests {
        /// Base commit/branch for comparison
        #[arg(default_value = "HEAD~1")]
        base: String,

        /// Head commit/branch for comparison
        #[arg(default_value = "HEAD")]
        head: String,

        /// Repository path
        #[arg(short, long)]
        repo: Option<PathBuf>,

        /// Output format (text, json, yaml)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Detect flaky tests from JUnit XML reports
    Flaky {
        /// Paths to JUnit XML files or directory containing them
        paths: Vec<PathBuf>,

        /// Minimum runs required to detect flakiness
        #[arg(long, default_value = "10")]
        min_runs: usize,

        /// Flakiness threshold (0.0-1.0)
        #[arg(long, default_value = "0.3")]
        threshold: f64,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Fetch and analyze workflow run history from GitHub
    History {
        /// Repository (format: owner/repo, e.g., "microsoft/vscode")
        #[arg(short, long)]
        repo: String,

        /// Workflow file name (e.g., "ci.yml", ".github/workflows/ci.yml")
        #[arg(short, long)]
        workflow: String,

        /// Number of runs to analyze
        #[arg(short, long, default_value = "100")]
        runs: usize,

        /// GitHub API token (or set GITHUB_TOKEN env var)
        #[arg(short, long)]
        token: Option<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path, format } => cmd_analyze(&path, &format),
        Commands::Optimize { path, output, diff } => cmd_optimize(&path, output.as_deref(), diff),
        Commands::Diff { path } => cmd_diff(&path),
        Commands::Cost {
            path,
            runs_per_month,
            team_size,
            hourly_rate,
        } => cmd_cost(&path, runs_per_month, team_size, hourly_rate),
        Commands::Graph {
            path,
            format,
            output,
        } => cmd_graph(&path, &format, output.as_deref()),
        Commands::Simulate {
            path,
            runs,
            variance,
            format,
        } => cmd_simulate(&path, runs, variance, &format),
        Commands::Docker {
            path,
            optimize,
            output,
        } => cmd_docker(&path, optimize, output.as_deref()),
        Commands::SelectTests {
            base,
            head,
            repo,
            format,
        } => cmd_select_tests(&base, &head, repo.as_deref(), &format),
        Commands::Flaky {
            paths,
            min_runs,
            threshold,
            format,
        } => cmd_flaky(&paths, min_runs, threshold, &format),
        Commands::History {
            repo,
            workflow,
            runs,
            token,
            format,
        } => cmd_history(&repo, &workflow, runs, token, &format).await,
    }
}

/// Detect CI provider from file path and parse accordingly.
fn parse_pipeline(path: &std::path::Path) -> Result<pipelinex_core::PipelineDag> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let path_str = path.to_string_lossy().to_lowercase();

    if filename == ".gitlab-ci.yml" || filename == ".gitlab-ci.yaml" || path_str.contains("gitlab")
    {
        GitLabCIParser::parse_file(path)
            .with_context(|| format!("Failed to parse GitLab CI file: {}", path.display()))
    } else if filename == "Jenkinsfile"
        || filename.ends_with(".jenkinsfile")
        || filename.ends_with(".groovy")
        || path_str.contains("jenkins")
    {
        JenkinsParser::parse_file(path)
            .with_context(|| format!("Failed to parse Jenkinsfile: {}", path.display()))
    } else if path_str.contains("circleci") || path_str.contains(".circleci") {
        CircleCIParser::parse_file(path)
            .with_context(|| format!("Failed to parse CircleCI config: {}", path.display()))
    } else if filename == "bitbucket-pipelines.yml"
        || filename == "bitbucket-pipelines.yaml"
        || path_str.contains("bitbucket")
    {
        BitbucketParser::parse_file(path)
            .with_context(|| format!("Failed to parse Bitbucket Pipelines: {}", path.display()))
    } else {
        // Default to GitHub Actions
        GitHubActionsParser::parse_file(path)
            .with_context(|| format!("Failed to parse GitHub Actions file: {}", path.display()))
    }
}

fn discover_workflow_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    if path.is_dir() {
        let pattern = format!("{}/**/*.yml", path.display());
        let mut files: Vec<PathBuf> = glob::glob(&pattern)
            .context("Failed to read glob pattern")?
            .chain(
                glob::glob(&format!("{}/**/*.yaml", path.display()))
                    .context("Failed to read glob pattern")?,
            )
            .filter_map(|r| r.ok())
            .collect();
        files.sort();
        return Ok(files);
    }

    anyhow::bail!("Path '{}' does not exist", path.display());
}

fn cmd_analyze(path: &Path, format: &str) -> Result<()> {
    let files = discover_workflow_files(path)?;

    if files.is_empty() {
        anyhow::bail!(
            "No workflow files found at '{}'. \
            Make sure the path points to a YAML workflow file or directory.",
            path.display()
        );
    }

    for file in &files {
        let dag = parse_pipeline(file)?;
        let report = analyzer::analyze(&dag);

        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&report)?;
                println!("{}", json);
            }
            "sarif" => {
                let sarif = pipelinex_core::analyzer::sarif::to_sarif(&report);
                let json = serde_json::to_string_pretty(&sarif)?;
                println!("{}", json);
            }
            "html" => {
                let html =
                    pipelinex_core::analyzer::html_report::generate_html_report(&report, &dag);
                println!("{}", html);
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
        anyhow::bail!(
            "'{}' is not a file. Optimize requires a single workflow file.",
            path.display()
        );
    }

    let dag = parse_pipeline(path)?;
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
            println!("Optimized config written to {}", out_path.display());
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

fn cmd_cost(path: &Path, runs_per_month: u32, team_size: u32, hourly_rate: f64) -> Result<()> {
    let files = discover_workflow_files(path)?;

    if files.is_empty() {
        anyhow::bail!("No workflow files found at '{}'", path.display());
    }

    for file in &files {
        let dag = parse_pipeline(file)?;
        let report = analyzer::analyze(&dag);

        let runner_type = dag
            .graph
            .node_weights()
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

fn cmd_graph(path: &Path, format: &str, output: Option<&std::path::Path>) -> Result<()> {
    if !path.is_file() {
        anyhow::bail!("'{}' is not a file.", path.display());
    }

    let dag = parse_pipeline(path)?;

    let content = match format {
        "dot" | "graphviz" => pipelinex_core::graph::to_dot(&dag),
        "ascii" | "text" => pipelinex_core::graph::to_ascii(&dag),
        _ => pipelinex_core::graph::to_mermaid(&dag),
    };

    match output {
        Some(out_path) => {
            std::fs::write(out_path, &content)?;
            println!("Graph written to {}", out_path.display());
        }
        None => {
            println!("{}", content);
        }
    }

    Ok(())
}

fn cmd_simulate(path: &Path, runs: usize, variance: f64, format: &str) -> Result<()> {
    if !path.is_file() {
        anyhow::bail!("'{}' is not a file.", path.display());
    }

    let dag = parse_pipeline(path)?;
    let result = pipelinex_core::simulator::simulate(&dag, runs, variance);

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&result)?;
            println!("{}", json);
        }
        _ => {
            display::print_simulation_report(&dag.name, &result);
        }
    }

    Ok(())
}

fn cmd_docker(path: &PathBuf, optimize: bool, output: Option<&std::path::Path>) -> Result<()> {
    if !path.is_file() {
        anyhow::bail!("'{}' is not a file.", path.display());
    }

    let content = std::fs::read_to_string(path)?;
    let analysis = pipelinex_core::optimizer::docker_opt::analyze_dockerfile(&content);

    if optimize {
        if let Some(optimized) = &analysis.optimized_dockerfile {
            match output {
                Some(out_path) => {
                    std::fs::write(out_path, optimized)?;
                    println!("Optimized Dockerfile written to {}", out_path.display());
                }
                None => {
                    print!("{}", optimized);
                }
            }
        }
    } else {
        display::print_docker_analysis(path, &analysis);
    }

    Ok(())
}

fn cmd_select_tests(
    base: &str,
    head: &str,
    repo: Option<&std::path::Path>,
    format: &str,
) -> Result<()> {
    let selector = TestSelector::new();
    let selection = selector.select_from_git_diff(base, head, repo)?;

    match format {
        "json" => {
            #[derive(serde::Serialize)]
            struct Output {
                changed_files: Vec<String>,
                selected_tests: Vec<String>,
                test_patterns: Vec<String>,
                selection_ratio: f64,
                reasoning: Vec<String>,
            }

            let output = Output {
                changed_files: selection
                    .changed_files
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
                selected_tests: selection.selected_tests,
                test_patterns: selection.test_patterns,
                selection_ratio: selection.selection_ratio,
                reasoning: selection.reasoning,
            };

            let json = serde_json::to_string_pretty(&output)?;
            println!("{}", json);
        }
        "yaml" => {
            #[derive(serde::Serialize)]
            struct Output {
                changed_files: Vec<String>,
                selected_tests: Vec<String>,
                test_patterns: Vec<String>,
                selection_ratio: f64,
                reasoning: Vec<String>,
            }

            let output = Output {
                changed_files: selection
                    .changed_files
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
                selected_tests: selection.selected_tests,
                test_patterns: selection.test_patterns,
                selection_ratio: selection.selection_ratio,
                reasoning: selection.reasoning,
            };

            let yaml = serde_yaml::to_string(&output)?;
            println!("{}", yaml);
        }
        _ => {
            display::print_test_selection(&selection);
        }
    }

    Ok(())
}

fn cmd_flaky(paths: &[PathBuf], min_runs: usize, threshold: f64, format: &str) -> Result<()> {
    if paths.is_empty() {
        anyhow::bail!("No paths provided. Specify JUnit XML files or directories.");
    }

    // Collect all JUnit XML files
    let mut junit_files = Vec::new();
    for path in paths {
        if path.is_file() {
            if path.extension().and_then(|e| e.to_str()) == Some("xml") {
                junit_files.push(path.clone());
            }
        } else if path.is_dir() {
            // Find all XML files in directory
            let pattern = format!("{}/**/*.xml", path.display());
            let files: Vec<PathBuf> = glob::glob(&pattern)
                .context("Failed to read glob pattern")?
                .filter_map(|r| r.ok())
                .collect();
            junit_files.extend(files);
        }
    }

    if junit_files.is_empty() {
        anyhow::bail!("No JUnit XML files found in provided paths");
    }

    let detector = FlakyDetector::with_config(min_runs, threshold);
    let report = detector.analyze_junit_files(&junit_files)?;

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&report)?;
            println!("{}", json);
        }
        _ => {
            display::print_flaky_report(&report, &junit_files);
        }
    }

    Ok(())
}

async fn cmd_history(
    repo: &str,
    workflow: &str,
    runs: usize,
    token: Option<String>,
    format: &str,
) -> Result<()> {
    // Parse repository owner/name
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid repository format. Expected: owner/repo (e.g., microsoft/vscode)");
    }
    let (owner, repo_name) = (parts[0], parts[1]);

    // Normalize workflow file name
    let workflow_file = if workflow.starts_with(".github/workflows/") {
        workflow.trim_start_matches(".github/workflows/")
    } else {
        workflow
    };

    // Get token from argument or environment
    let api_token = token.or_else(|| std::env::var("GITHUB_TOKEN").ok());

    println!("🔍 Analyzing workflow run history...");
    println!("   Repository: {}/{}", owner, repo_name);
    println!("   Workflow: {}", workflow_file);
    println!("   Runs to analyze: {}", runs);
    println!();

    // Create GitHub API client
    let client = GitHubClient::new(api_token).context("Failed to create GitHub API client")?;

    // Fetch and analyze workflow history
    let stats = client
        .analyze_workflow_history(owner, repo_name, workflow_file, runs)
        .await
        .context("Failed to analyze workflow history")?;

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&stats)?;
            println!("{}", json);
        }
        _ => {
            display::print_history_stats(&stats);
        }
    }

    Ok(())
}
