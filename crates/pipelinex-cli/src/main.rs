mod display;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use pipelinex_core::analyzer;
use pipelinex_core::flaky_detector::FlakyDetector;
use pipelinex_core::github_actions_to_gitlab_ci;
use pipelinex_core::multi_repo::{analyze_multi_repo, RepoPipeline};
use pipelinex_core::optimizer::Optimizer;
use pipelinex_core::parser::argo::ArgoWorkflowsParser;
use pipelinex_core::parser::aws_codepipeline::AwsCodePipelineParser;
use pipelinex_core::parser::azure::AzurePipelinesParser;
use pipelinex_core::parser::bitbucket::BitbucketParser;
use pipelinex_core::parser::buildkite::BuildkiteParser;
use pipelinex_core::parser::circleci::CircleCIParser;
use pipelinex_core::parser::drone::DroneParser;
use pipelinex_core::parser::github::GitHubActionsParser;
use pipelinex_core::parser::gitlab::GitLabCIParser;
use pipelinex_core::parser::jenkins::JenkinsParser;
use pipelinex_core::parser::tekton::TektonParser;
use pipelinex_core::plugins;
use pipelinex_core::profile_runner_sizing;
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

        /// Output format (text, json, sarif, html, markdown)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Disable all network calls (offline mode for air-gapped environments)
        #[arg(long)]
        offline: bool,

        /// Redact sensitive information from output (for sharing with external parties)
        #[arg(long)]
        redact: bool,

        /// Sign the JSON output with an Ed25519 private key (hex or file path)
        #[arg(long)]
        sign: Option<String>,
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

    /// Apply optimization and create a Pull Request with optimized config
    Apply {
        /// Path to the workflow file to optimize
        path: PathBuf,

        /// GitHub repository (format: owner/repo, auto-detected if not provided)
        #[arg(short, long)]
        repo: Option<String>,

        /// Base branch for PR (default: main)
        #[arg(long, default_value = "main")]
        base: String,

        /// GitHub API token (or set GITHUB_TOKEN env var)
        #[arg(short, long)]
        token: Option<String>,

        /// Skip PR creation and only create branch with optimized config
        #[arg(long)]
        no_pr: bool,
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

    /// Migrate workflow config between CI providers (GitHub Actions -> GitLab CI)
    Migrate {
        /// Path to source workflow file
        path: PathBuf,

        /// Target provider (currently: gitlab-ci)
        #[arg(long, default_value = "gitlab-ci")]
        to: String,

        /// Output file path for migrated config
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (text, json, yaml)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Analyze orchestration patterns across multiple repositories
    MultiRepo {
        /// Path to a root directory containing multiple repositories
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Recommend right-sized runners based on inferred resource pressure
    RightSize {
        /// Path to workflow file or directory containing workflow files
        #[arg(default_value = ".github/workflows/")]
        path: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// External plugin management (scaffold and inspection)
    Plugins {
        #[command(subcommand)]
        command: PluginCommands,
    },

    /// Generate shell completions for Bash, Zsh, Fish, or PowerShell
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Auto-detect CI platform and generate initial configuration
    Init {
        /// Directory to scan for CI configs
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output path for generated config
        #[arg(short, long, default_value = ".pipelinex/config.toml")]
        output: PathBuf,
    },

    /// Compare two pipeline configurations
    Compare {
        /// First pipeline config file
        file_a: PathBuf,

        /// Second pipeline config file
        file_b: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Watch pipeline configs for changes and re-analyze on save
    Watch {
        /// Path to watch (file or directory)
        #[arg(default_value = ".github/workflows/")]
        path: PathBuf,

        /// Output format for analysis
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Lint CI config for syntax errors, deprecations, and typos
    Lint {
        /// Path to workflow file or directory
        #[arg(default_value = ".github/workflows/")]
        path: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Run security scan on pipeline configs (secrets, permissions, injection, supply chain)
    Security {
        /// Path to workflow file or directory
        #[arg(default_value = ".github/workflows/")]
        path: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Check pipeline configs against organisational policy rules
    Policy {
        #[command(subcommand)]
        command: PolicyCommands,
    },

    /// Discover and analyze all CI configs across a monorepo
    Monorepo {
        /// Root directory to scan
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum directory depth to scan
        #[arg(long, default_value = "5")]
        depth: usize,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Generate a CI Software Bill of Materials (CycloneDX SBOM)
    Sbom {
        /// Path to workflow file or directory
        #[arg(default_value = ".github/workflows/")]
        path: PathBuf,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate a pipeline health score badge for READMEs
    Badge {
        /// Path to workflow file
        path: PathBuf,

        /// Output format (markdown, json, url)
        #[arg(short, long, default_value = "markdown")]
        format: String,
    },

    /// Ed25519 key management for report signing
    Keys {
        #[command(subcommand)]
        command: KeysCommands,
    },

    /// Verify a signed PipelineX report
    Verify {
        /// Path to signed report JSON file
        report: PathBuf,

        /// Public key (hex string) or path to key file
        #[arg(long)]
        key: String,
    },

    /// Start MCP (Model Context Protocol) server for AI tool integration
    McpServer,

    /// Explain analysis findings in plain English (LLM-powered or template fallback)
    Explain {
        /// Path to workflow file
        path: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Estimated pipeline runs per month (for impact calculation)
        #[arg(long, default_value = "500")]
        runs_per_month: u32,
    },

    /// What-if simulator: explore optimization impact by modifying the pipeline
    WhatIf {
        /// Path to workflow file
        path: PathBuf,

        /// Modifications to apply (e.g., "add-cache build 120", "remove-dep test->deploy")
        #[arg(short, long)]
        modify: Vec<String>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum KeysCommands {
    /// Generate a new Ed25519 keypair for report signing
    Generate {
        /// Output directory for key files
        #[arg(default_value = ".pipelinex")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum PolicyCommands {
    /// Check pipeline configs against a policy file
    Check {
        /// Path to workflow file or directory
        #[arg(default_value = ".github/workflows/")]
        path: PathBuf,

        /// Path to policy file
        #[arg(short = 'c', long, default_value = ".pipelinex/policy.toml")]
        policy: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Generate a starter policy file
    Init {
        /// Path for the new policy file
        #[arg(default_value = ".pipelinex/policy.toml")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum PluginCommands {
    /// List configured analyzer/optimizer plugins
    List {
        /// Optional explicit manifest path (defaults to PIPELINEX_PLUGIN_MANIFEST)
        #[arg(long)]
        manifest: Option<PathBuf>,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Create a starter plugin manifest template
    Scaffold {
        /// Path to manifest file to create
        #[arg(default_value = ".pipelinex/plugins.json")]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            path,
            format,
            offline: _offline,
            redact,
            sign,
        } => cmd_analyze(&path, &format, redact, sign.as_deref()),
        Commands::Optimize { path, output, diff } => cmd_optimize(&path, output.as_deref(), diff),
        Commands::Diff { path } => cmd_diff(&path),
        Commands::Apply {
            path,
            repo,
            base,
            token,
            no_pr,
        } => cmd_apply(&path, repo.as_deref(), &base, token, no_pr).await,
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
        Commands::Migrate {
            path,
            to,
            output,
            format,
        } => cmd_migrate(&path, &to, output.as_deref(), &format),
        Commands::MultiRepo { path, format } => cmd_multi_repo(&path, &format),
        Commands::RightSize { path, format } => cmd_right_size(&path, &format),
        Commands::Plugins { command } => cmd_plugins(command),
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "pipelinex", &mut std::io::stdout());
            Ok(())
        }
        Commands::Init { path, output } => cmd_init(&path, &output),
        Commands::Compare {
            file_a,
            file_b,
            format,
        } => cmd_compare(&file_a, &file_b, &format),
        Commands::Watch { path, format } => cmd_watch(&path, &format),
        Commands::Lint { path, format } => cmd_lint(&path, &format),
        Commands::Security { path, format } => cmd_security(&path, &format),
        Commands::Policy { command } => cmd_policy(command),
        Commands::Monorepo {
            path,
            depth,
            format,
        } => cmd_monorepo_discover(&path, depth, &format),
        Commands::Sbom { path, output } => cmd_sbom(&path, output.as_deref()),
        Commands::Badge { path, format } => cmd_badge(&path, &format),
        Commands::Keys { command } => cmd_keys(command),
        Commands::Verify { report, key } => cmd_verify(&report, &key),
        Commands::McpServer => {
            pipelinex_core::mcp::run_stdio_server()?;
            Ok(())
        }
        Commands::Explain {
            path,
            format,
            runs_per_month,
        } => cmd_explain(&path, &format, runs_per_month).await,
        Commands::WhatIf {
            path,
            modify,
            format,
        } => cmd_whatif(&path, &modify, &format),
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
    } else if filename == "azure-pipelines.yml"
        || filename == "azure-pipelines.yaml"
        || path_str.contains("azure-pipelines")
    {
        AzurePipelinesParser::parse_file(path)
            .with_context(|| format!("Failed to parse Azure Pipelines file: {}", path.display()))
    } else if filename == "codepipeline.json"
        || filename == "codepipeline.yaml"
        || filename == "codepipeline.yml"
        || filename == "pipeline.json" && path_str.contains("codepipeline")
        || path_str.contains("aws-codepipeline")
    {
        AwsCodePipelineParser::parse_file(path)
            .with_context(|| format!("Failed to parse AWS CodePipeline file: {}", path.display()))
    } else if filename == "bitbucket-pipelines.yml"
        || filename == "bitbucket-pipelines.yaml"
        || path_str.contains("bitbucket")
    {
        BitbucketParser::parse_file(path)
            .with_context(|| format!("Failed to parse Bitbucket Pipelines: {}", path.display()))
    } else if (filename == "pipeline.yml" || filename == "pipeline.yaml")
        && path_str.contains(".buildkite")
        || path_str.contains("buildkite")
    {
        BuildkiteParser::parse_file(path)
            .with_context(|| format!("Failed to parse Buildkite pipeline: {}", path.display()))
    } else if filename == ".drone.yml"
        || filename == ".drone.yaml"
        || filename == ".woodpecker.yml"
        || filename == ".woodpecker.yaml"
        || path_str.contains("drone")
        || path_str.contains("woodpecker")
    {
        DroneParser::parse_file(path)
            .with_context(|| format!("Failed to parse Drone CI file: {}", path.display()))
    } else if path_has_token(&path_str, "tekton") || is_tekton_content(path) {
        TektonParser::parse_file(path)
            .with_context(|| format!("Failed to parse Tekton file: {}", path.display()))
    } else if path_has_token(&path_str, "argo")
        || path_has_token(&path_str, "argoproj")
        || is_argo_content(path)
    {
        ArgoWorkflowsParser::parse_file(path)
            .with_context(|| format!("Failed to parse Argo Workflows file: {}", path.display()))
    } else {
        // Default to GitHub Actions
        GitHubActionsParser::parse_file(path)
            .with_context(|| format!("Failed to parse GitHub Actions file: {}", path.display()))
    }
}

/// Check if file content looks like a Tekton resource.
fn is_tekton_content(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|c| c.contains("tekton.dev") || (c.contains("kind: Pipeline") && c.contains("tasks:")))
        .unwrap_or(false)
}

/// Check if file content looks like an Argo Workflows resource.
fn is_argo_content(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|c| {
            c.contains("argoproj.io") || (c.contains("kind: Workflow") && c.contains("entrypoint:"))
        })
        .unwrap_or(false)
}

fn path_has_token(path: &str, token: &str) -> bool {
    path.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|part| part.eq_ignore_ascii_case(token))
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
            .chain(
                glob::glob(&format!("{}/**/*.json", path.display()))
                    .context("Failed to read glob pattern")?,
            )
            .filter_map(|r| r.ok())
            .collect();
        files.sort();
        return Ok(files);
    }

    anyhow::bail!("Path '{}' does not exist", path.display());
}

fn discover_repo_roots(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        anyhow::bail!("Path '{}' does not exist", root.display());
    }
    if root.is_file() {
        anyhow::bail!("'{}' is a file. Expected a directory.", root.display());
    }

    let mut repos = vec![root.to_path_buf()];
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".git" || name == "target" || name == "node_modules" {
                continue;
            }
            repos.push(entry.path());
        }
    }

    repos.sort();
    repos.dedup();
    Ok(repos)
}

fn discover_repo_pipeline_files(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for fixed in [
        ".gitlab-ci.yml",
        ".gitlab-ci.yaml",
        ".drone.yml",
        ".drone.yaml",
        ".woodpecker.yml",
        ".woodpecker.yaml",
        "Jenkinsfile",
        "bitbucket-pipelines.yml",
        "bitbucket-pipelines.yaml",
        "azure-pipelines.yml",
        "azure-pipelines.yaml",
        "codepipeline.json",
        "codepipeline.yml",
        "codepipeline.yaml",
        "pipeline.json",
        ".circleci/config.yml",
        ".circleci/config.yaml",
        ".buildkite/pipeline.yml",
        ".buildkite/pipeline.yaml",
    ] {
        let path = repo_root.join(fixed);
        if path.is_file() {
            files.push(path);
        }
    }

    let gh_patterns = [
        format!("{}/.github/workflows/*.yml", repo_root.display()),
        format!("{}/.github/workflows/*.yaml", repo_root.display()),
    ];
    for pattern in gh_patterns {
        let entries = glob::glob(&pattern).context("Failed to read workflow glob pattern")?;
        for path in entries.flatten() {
            files.push(path);
        }
    }

    let extra_patterns = [
        format!("{}/.tekton/**/*.yml", repo_root.display()),
        format!("{}/.tekton/**/*.yaml", repo_root.display()),
        format!("{}/tekton/**/*.yml", repo_root.display()),
        format!("{}/tekton/**/*.yaml", repo_root.display()),
        format!("{}/.argo/**/*.yml", repo_root.display()),
        format!("{}/.argo/**/*.yaml", repo_root.display()),
        format!("{}/argo/**/*.yml", repo_root.display()),
        format!("{}/argo/**/*.yaml", repo_root.display()),
        format!("{}/argo-workflows/**/*.yml", repo_root.display()),
        format!("{}/argo-workflows/**/*.yaml", repo_root.display()),
    ];

    for pattern in extra_patterns {
        let entries = glob::glob(&pattern).context("Failed to read workflow glob pattern")?;
        for path in entries.flatten() {
            files.push(path);
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn cmd_analyze(path: &Path, format: &str, redact: bool, sign_key: Option<&str>) -> Result<()> {
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
        let mut report = analyzer::analyze(&dag);

        if redact {
            report = pipelinex_core::redact::redact_report(&report);
        }

        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&report)?;
                if let Some(key) = sign_key {
                    let key_hex = read_key_material(key)?;
                    let signed = pipelinex_core::sign_report(&json, &key_hex)?;
                    println!("{}", serde_json::to_string_pretty(&signed)?);
                } else {
                    println!("{}", json);
                }
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
            "markdown" | "md" => {
                print!("{}", display::format_markdown_report(&report));
            }
            _ => {
                display::print_analysis_report(&report);
            }
        }
    }

    Ok(())
}

fn read_key_material(key_or_path: &str) -> Result<String> {
    // If it looks like a hex key (64 chars, all hex), use directly
    if key_or_path.len() == 64 && key_or_path.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(key_or_path.to_string());
    }
    // Otherwise try to read as file
    let path = Path::new(key_or_path);
    if path.is_file() {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read key file: {}", path.display()))?;
        Ok(content.trim().to_string())
    } else {
        Ok(key_or_path.to_string())
    }
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

async fn cmd_apply(
    path: &PathBuf,
    repo_arg: Option<&str>,
    base_branch: &str,
    token: Option<String>,
    no_pr: bool,
) -> Result<()> {
    use std::process::Command;

    // Verify we're in a git repository
    let git_check = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output();

    if git_check.is_err() || !git_check.unwrap().status.success() {
        anyhow::bail!(
            "Not in a git repository. Please run this command from within a git repository."
        );
    }

    // Get the GitHub token
    let github_token = token
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .context("GitHub token required. Set GITHUB_TOKEN env var or use --token")?;

    // Detect repository if not provided
    let repo_name = if let Some(r) = repo_arg {
        r.to_string()
    } else {
        // Try to detect from git remote
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output()
            .context("Failed to get git remote origin")?;

        if !output.status.success() {
            anyhow::bail!("No git remote 'origin' found. Please specify --repo owner/repo");
        }

        let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Parse GitHub URL (supports both HTTPS and SSH)
        let repo = if let Some(captures) =
            regex::Regex::new(r"github\.com[:/](.+?/[^/]+?)(?:\.git)?$")
                .unwrap()
                .captures(&remote_url)
        {
            captures.get(1).unwrap().as_str().to_string()
        } else {
            anyhow::bail!(
                "Could not parse GitHub repository from remote URL: {}",
                remote_url
            );
        };

        repo
    };

    println!("🔍 Analyzing pipeline: {}", path.display());

    // Parse and optimize the pipeline
    let dag = parse_pipeline(path)?;
    let report = analyzer::analyze(&dag);

    if report.findings.is_empty() {
        println!("✅ No optimization opportunities found!");
        return Ok(());
    }

    let optimized_content = Optimizer::optimize(path, &report)?;

    // Create a new branch name
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("config");
    let branch_name = format!("pipelinex-optimize-{}", filename);

    println!("🌿 Creating branch: {}", branch_name);

    // Check if branch already exists
    let branch_exists = Command::new("git")
        .args(["rev-parse", "--verify", &branch_name])
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if branch_exists {
        println!(
            "⚠️  Branch {} already exists. Switching to it...",
            branch_name
        );
        Command::new("git")
            .args(["checkout", &branch_name])
            .status()
            .context("Failed to checkout existing branch")?;
    } else {
        // Create and checkout new branch
        Command::new("git")
            .args(["checkout", "-b", &branch_name])
            .status()
            .context("Failed to create new branch")?;
    }

    // Write optimized config
    println!("📝 Writing optimized configuration...");
    std::fs::write(path, &optimized_content).context("Failed to write optimized configuration")?;

    // Commit changes
    println!("💾 Committing changes...");
    Command::new("git")
        .args(["add", path.to_str().unwrap()])
        .status()
        .context("Failed to git add")?;

    let commit_msg = format!(
        "chore: optimize {} with PipelineX\n\n\
         Found {} optimization opportunities:\n\
         - Estimated time savings: {:.0}%\n\
         - Current duration: {:.0}s → Optimized: {:.0}s\n\n\
         Generated by PipelineX (https://github.com/mackeh/PipelineX)",
        filename,
        report.findings.len(),
        ((report.total_estimated_duration_secs - report.optimized_duration_secs)
            / report.total_estimated_duration_secs
            * 100.0),
        report.total_estimated_duration_secs,
        report.optimized_duration_secs
    );

    Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .status()
        .context("Failed to commit changes")?;

    // Push to remote
    println!("⬆️  Pushing to remote...");
    Command::new("git")
        .args(["push", "-u", "origin", &branch_name])
        .status()
        .context("Failed to push branch")?;

    if no_pr {
        println!("✅ Branch created and pushed. Run with --no-pr=false to create a PR.");
        return Ok(());
    }

    // Create pull request
    println!("🔀 Creating pull request...");

    let parts: Vec<&str> = repo_name.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "Invalid repository format. Expected owner/repo, got: {}",
            repo_name
        );
    }
    let (owner, repo) = (parts[0], parts[1]);

    let client = GitHubClient::new(Some(github_token))?;

    let pr_title = format!("⚡ Optimize {} with PipelineX", filename);
    let pr_body = format!(
        "## Pipeline Optimization\n\n\
         This PR optimizes `{}` to improve CI/CD performance.\n\n\
         ### 📊 Improvements\n\n\
         - **Findings**: {} optimization opportunities\n\
         - **Time Savings**: {:.0}% faster\n\
         - **Current Duration**: {:.0}s\n\
         - **Optimized Duration**: {:.0}s\n\n\
         ### 🔍 Key Optimizations\n\n{}\n\n\
         ---\n\
         Generated by [PipelineX](https://github.com/mackeh/PipelineX) — \
         Your pipelines are slow. PipelineX knows why — and fixes them automatically.",
        path.display(),
        report.findings.len(),
        ((report.total_estimated_duration_secs - report.optimized_duration_secs)
            / report.total_estimated_duration_secs
            * 100.0),
        report.total_estimated_duration_secs,
        report.optimized_duration_secs,
        report
            .findings
            .iter()
            .take(5)
            .map(|f| format!("- **{:?}**: {}", f.severity, f.title))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let pr = client
        .create_pull_request(owner, repo, &pr_title, &pr_body, &branch_name, base_branch)
        .await?;

    println!("\n✅ Pull request created successfully!");
    println!("🔗 {}", pr.html_url);
    println!("📝 PR #{}: {}", pr.number, pr.title);

    Ok(())
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

    if format != "json" {
        println!("🔍 Analyzing workflow run history...");
        println!("   Repository: {}/{}", owner, repo_name);
        println!("   Workflow: {}", workflow_file);
        println!("   Runs to analyze: {}", runs);
        println!();
    }

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

fn cmd_migrate(
    path: &Path,
    target_provider: &str,
    output: Option<&std::path::Path>,
    format: &str,
) -> Result<()> {
    if !path.is_file() {
        anyhow::bail!("'{}' is not a file.", path.display());
    }

    let dag = parse_pipeline(path)?;
    let migration = match target_provider {
        "gitlab" | "gitlab-ci" => github_actions_to_gitlab_ci(&dag)?,
        other => anyhow::bail!(
            "Unsupported migration target '{}'. Supported targets: gitlab-ci",
            other
        ),
    };

    if let Some(out_path) = output {
        std::fs::write(out_path, &migration.yaml)?;
    }

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&migration)?);
        }
        "yaml" => {
            if output.is_none() {
                print!("{}", migration.yaml);
            } else if let Some(out_path) = output {
                println!("Migrated config written to {}", out_path.display());
            }
        }
        _ => {
            println!("Migration completed:");
            println!("  Source: {}", migration.source_provider);
            println!("  Target: {}", migration.target_provider);
            println!("  Jobs converted: {}", migration.converted_jobs);
            if migration.warnings.is_empty() {
                println!("  Warnings: none");
            } else {
                println!("  Warnings: {}", migration.warnings.len());
                for warning in &migration.warnings {
                    println!("  - {}", warning);
                }
            }

            match output {
                Some(out_path) => {
                    println!("Migrated config written to {}", out_path.display());
                }
                None => {
                    println!();
                    print!("{}", migration.yaml);
                }
            }
        }
    }

    Ok(())
}

fn cmd_multi_repo(path: &Path, format: &str) -> Result<()> {
    let repo_roots = discover_repo_roots(path)?;

    let mut pipelines = Vec::new();
    let mut skipped = Vec::new();

    for repo_root in repo_roots {
        let repo_name = repo_root
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| repo_root.display().to_string());

        let files = discover_repo_pipeline_files(&repo_root)?;
        for file in files {
            match parse_pipeline(&file) {
                Ok(dag) => pipelines.push(RepoPipeline {
                    repo: repo_name.clone(),
                    dag,
                }),
                Err(error) => skipped.push((file, error.to_string())),
            }
        }
    }

    if pipelines.is_empty() {
        anyhow::bail!(
            "No CI pipeline files were found or parsed under '{}'",
            path.display()
        );
    }

    let report = analyze_multi_repo(&pipelines);

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("PipelineX Multi-Repo Analysis");
    println!(
        "  Repositories: {}  Workflows: {}",
        report.repo_count, report.workflow_count
    );
    println!(
        "  Orchestration edges: {}",
        report.orchestration_edges.len()
    );
    println!("  Findings: {}", report.findings.len());
    println!();

    println!("Repository Summary:");
    for repo in &report.repos {
        println!(
            "  - {}: {} workflows, {} jobs, slowest critical path {}s",
            repo.repo,
            repo.workflow_count,
            repo.total_jobs,
            repo.max_critical_path_secs.round()
        );
    }

    if !report.orchestration_edges.is_empty() {
        println!();
        println!("Detected Orchestration Edges:");
        for edge in &report.orchestration_edges {
            println!(
                "  - {} -> {} ({}, confidence {:.0}%)",
                edge.from_repo,
                edge.to_repo,
                edge.trigger_hint,
                edge.confidence * 100.0
            );
        }
    }

    if !report.findings.is_empty() {
        println!();
        println!("Findings:");
        for finding in &report.findings {
            println!("  - [{}] {}", finding.severity.symbol(), finding.title);
            println!("    {}", finding.description);
            println!("    Recommendation: {}", finding.recommendation);
        }
    }

    if !skipped.is_empty() {
        println!();
        println!(
            "Skipped {} file(s) that could not be parsed as supported CI configs:",
            skipped.len()
        );
        for (file, error) in skipped.iter().take(5) {
            println!("  - {}: {}", file.display(), error);
        }
        if skipped.len() > 5 {
            println!("  - ... and {} more", skipped.len() - 5);
        }
    }

    Ok(())
}

fn cmd_right_size(path: &Path, format: &str) -> Result<()> {
    let files = discover_workflow_files(path)?;
    if files.is_empty() {
        anyhow::bail!("No workflow files found at '{}'", path.display());
    }

    #[derive(serde::Serialize)]
    struct Output {
        source_file: String,
        report: pipelinex_core::RunnerSizingReport,
    }

    let mut outputs = Vec::new();
    for file in &files {
        let dag = parse_pipeline(file)?;
        let report = profile_runner_sizing(&dag);
        outputs.push(Output {
            source_file: file.display().to_string(),
            report,
        });
    }

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&outputs)?);
        return Ok(());
    }

    for output in &outputs {
        display::print_runner_sizing_report(Path::new(&output.source_file), &output.report);
    }

    Ok(())
}

fn cmd_init(scan_path: &Path, output: &Path) -> Result<()> {
    println!("PipelineX Init — Scanning for CI configurations...");
    println!();

    // Auto-detect CI platforms
    let detections: Vec<(&str, &str)> = vec![
        (".github/workflows/", "github-actions"),
        (".gitlab-ci.yml", "gitlab-ci"),
        (".gitlab-ci.yaml", "gitlab-ci"),
        ("Jenkinsfile", "jenkins"),
        (".circleci/config.yml", "circleci"),
        (".circleci/config.yaml", "circleci"),
        ("bitbucket-pipelines.yml", "bitbucket"),
        ("bitbucket-pipelines.yaml", "bitbucket"),
        ("azure-pipelines.yml", "azure-pipelines"),
        ("azure-pipelines.yaml", "azure-pipelines"),
        (".buildkite/pipeline.yml", "buildkite"),
        (".buildkite/pipeline.yaml", "buildkite"),
    ];

    let mut detected = Vec::new();
    for (path, provider) in &detections {
        let full = scan_path.join(path);
        if full.exists() {
            detected.push((*provider, full));
        }
    }

    if detected.is_empty() {
        println!("  No CI configurations found in '{}'.", scan_path.display());
        println!("  Run this command from your project root directory.");
        return Ok(());
    }

    println!("  Detected CI platforms:");
    let mut seen_providers = std::collections::HashSet::new();
    for (provider, path) in &detected {
        if seen_providers.insert(*provider) {
            println!("    - {} ({})", provider, path.display());
        }
    }
    println!();

    // Generate config file
    let primary_provider = detected[0].0;
    let config_content = format!(
        r#"# PipelineX Configuration
# Generated by `pipelinex init`

[general]
provider = "{}"
severity_threshold = "medium"
output_format = "text"

[cost]
runs_per_month = 500
team_size = 10
hourly_rate = 150.0

[analysis]
# Enable security scanning
security_scan = true
# Enable lint checking
lint = true
"#,
        primary_provider,
    );

    // Create parent directory
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory '{}'", parent.display()))?;
    }

    std::fs::write(output, &config_content)
        .with_context(|| format!("Failed to write config to '{}'", output.display()))?;

    println!("  Config written to: {}", output.display());
    println!();
    println!("  Next steps:");
    println!("    pipelinex analyze    — Analyze your pipelines");
    println!("    pipelinex lint       — Lint your CI configs");
    println!("    pipelinex security   — Run security scan");
    println!();

    Ok(())
}

fn cmd_compare(file_a: &Path, file_b: &Path, format: &str) -> Result<()> {
    if !file_a.is_file() {
        anyhow::bail!("'{}' is not a file.", file_a.display());
    }
    if !file_b.is_file() {
        anyhow::bail!("'{}' is not a file.", file_b.display());
    }

    let dag_a = parse_pipeline(file_a)?;
    let dag_b = parse_pipeline(file_b)?;
    let report_a = analyzer::analyze(&dag_a);
    let report_b = analyzer::analyze(&dag_b);

    match format {
        "json" => {
            #[derive(serde::Serialize)]
            struct CompareOutput {
                file_a: String,
                file_b: String,
                report_a: pipelinex_core::AnalysisReport,
                report_b: pipelinex_core::AnalysisReport,
                duration_delta_secs: f64,
                findings_delta: i64,
            }

            let output = CompareOutput {
                file_a: file_a.display().to_string(),
                file_b: file_b.display().to_string(),
                duration_delta_secs: report_b.total_estimated_duration_secs
                    - report_a.total_estimated_duration_secs,
                findings_delta: report_b.findings.len() as i64 - report_a.findings.len() as i64,
                report_a,
                report_b,
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            display::print_comparison(
                &report_a,
                &report_b,
                &file_a.display().to_string(),
                &file_b.display().to_string(),
            );
        }
    }

    Ok(())
}

fn cmd_watch(path: &Path, format: &str) -> Result<()> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let format = format.to_string();
    let watch_path = if path.is_file() {
        path.parent().unwrap_or(path).to_path_buf()
    } else {
        path.to_path_buf()
    };

    if !watch_path.exists() {
        anyhow::bail!("Watch path '{}' does not exist", watch_path.display());
    }

    println!(
        "PipelineX Watch — Monitoring {} for changes (Ctrl+C to stop)",
        watch_path.display()
    );
    println!();

    // Do an initial analysis
    let _ = run_analysis_for_watch(path, &format);

    let (tx, rx) = mpsc::channel::<Result<Event, notify::Error>>();
    let mut watcher =
        RecommendedWatcher::new(tx, Config::default()).context("Failed to create file watcher")?;

    watcher
        .watch(&watch_path, RecursiveMode::Recursive)
        .context("Failed to start watching")?;

    let mut last_run = Instant::now();
    let debounce = Duration::from_millis(500);

    for event in rx {
        match event {
            Ok(event) => {
                let is_relevant = event.paths.iter().any(|p| {
                    let ext = p.extension().and_then(|e| e.to_str());
                    matches!(ext, Some("yml") | Some("yaml") | Some("json"))
                });

                if is_relevant && last_run.elapsed() > debounce {
                    last_run = Instant::now();
                    // Clear screen
                    print!("\x1b[2J\x1b[H");
                    println!(
                        "[{}] Change detected, re-analysing...",
                        chrono::Local::now().format("%H:%M:%S")
                    );
                    println!();
                    let _ = run_analysis_for_watch(path, &format);
                }
            }
            Err(e) => {
                eprintln!("Watch error: {:?}", e);
            }
        }
    }

    Ok(())
}

fn run_analysis_for_watch(path: &Path, format: &str) -> Result<()> {
    let files = discover_workflow_files(path)?;
    for file in &files {
        match parse_pipeline(file) {
            Ok(dag) => {
                let report = analyzer::analyze(&dag);
                match format {
                    "json" => {
                        let json = serde_json::to_string_pretty(&report)?;
                        println!("{}", json);
                    }
                    "markdown" | "md" => {
                        print!("{}", display::format_markdown_report(&report));
                    }
                    _ => {
                        display::print_analysis_report(&report);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error parsing {}: {}", file.display(), e);
            }
        }
    }
    Ok(())
}

fn cmd_lint(path: &Path, format: &str) -> Result<()> {
    let files = discover_workflow_files(path)?;

    if files.is_empty() {
        anyhow::bail!("No workflow files found at '{}'", path.display());
    }

    let mut exit_code = 0;

    for file in &files {
        let content = std::fs::read_to_string(file)
            .with_context(|| format!("Failed to read '{}'", file.display()))?;

        let dag = parse_pipeline(file)?;
        let report = pipelinex_core::linter::lint(&content, &dag);

        if report.exit_code() > exit_code {
            exit_code = report.exit_code();
        }

        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&report)?;
                println!("{}", json);
            }
            _ => {
                display::print_lint_report(&report);
            }
        }
    }

    if exit_code == 2 {
        anyhow::bail!("Lint check failed with errors");
    }

    Ok(())
}

fn cmd_security(path: &Path, format: &str) -> Result<()> {
    let files = discover_workflow_files(path)?;

    if files.is_empty() {
        anyhow::bail!("No workflow files found at '{}'", path.display());
    }

    for file in &files {
        let dag = parse_pipeline(file)?;
        let findings = pipelinex_core::security::scan(&dag);

        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&findings)?;
                println!("{}", json);
            }
            _ => {
                display::print_security_report(&findings, &file.display().to_string());
            }
        }
    }

    Ok(())
}

fn cmd_policy(command: PolicyCommands) -> Result<()> {
    match command {
        PolicyCommands::Init { path } => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = pipelinex_core::policy::generate_default_policy();
            std::fs::write(&path, content)?;
            println!("Policy file created: {}", path.display());
            println!("Edit this file to configure your organisation's CI policy rules.");
            Ok(())
        }
        PolicyCommands::Check {
            path,
            policy: policy_path,
            format,
        } => {
            let policy = pipelinex_core::load_policy(&policy_path).with_context(|| {
                format!("Failed to load policy from '{}'", policy_path.display())
            })?;

            let files = discover_workflow_files(&path)?;
            if files.is_empty() {
                anyhow::bail!("No workflow files found at '{}'", path.display());
            }

            let mut any_failed = false;

            for file in &files {
                let dag = parse_pipeline(file)?;
                let report = pipelinex_core::check_policy(&dag, &policy);

                if !report.passed {
                    any_failed = true;
                }

                match format.as_str() {
                    "json" => {
                        let json = serde_json::to_string_pretty(&report)?;
                        println!("{}", json);
                    }
                    _ => {
                        display::print_policy_report(&report);
                    }
                }
            }

            if any_failed {
                anyhow::bail!("Policy check failed");
            }

            Ok(())
        }
    }
}

fn cmd_monorepo_discover(path: &Path, max_depth: usize, format: &str) -> Result<()> {
    let discovered = pipelinex_core::discovery::discover_monorepo(path, max_depth)?;

    if discovered.is_empty() {
        anyhow::bail!(
            "No CI pipeline files found under '{}' (depth {})",
            path.display(),
            max_depth
        );
    }

    let summary = pipelinex_core::discovery::aggregate_discovery(path, &discovered);

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    println!("PipelineX Monorepo Discovery — {}", path.display());
    println!(
        "  Found {} pipeline files across {} packages",
        summary.total_pipeline_files,
        summary.packages.len()
    );
    println!();

    // Now analyze each discovered file
    let mut total_findings = 0;
    let mut total_jobs = 0;

    for pipeline in &discovered {
        match parse_pipeline(&pipeline.file_path) {
            Ok(dag) => {
                let report = analyzer::analyze(&dag);
                total_findings += report.findings.len();
                total_jobs += report.job_count;
                println!(
                    "  [{}] {} — {} jobs, {} findings",
                    pipeline.package_name,
                    pipeline.relative_path,
                    report.job_count,
                    report.findings.len()
                );
            }
            Err(e) => {
                println!(
                    "  [{}] {} — Error: {}",
                    pipeline.package_name, pipeline.relative_path, e
                );
            }
        }
    }

    println!();
    println!(
        "  Total: {} jobs, {} findings across {} files",
        total_jobs,
        total_findings,
        discovered.len()
    );
    println!();

    Ok(())
}

fn cmd_sbom(path: &Path, output: Option<&std::path::Path>) -> Result<()> {
    let files = discover_workflow_files(path)?;
    if files.is_empty() {
        anyhow::bail!("No workflow files found at '{}'", path.display());
    }

    let mut dags = Vec::new();
    for file in &files {
        dags.push(parse_pipeline(file)?);
    }

    let dag_refs: Vec<&pipelinex_core::PipelineDag> = dags.iter().collect();
    let sbom = pipelinex_core::generate_sbom(&dag_refs);
    let json = serde_json::to_string_pretty(&sbom)?;

    match output {
        Some(out_path) => {
            std::fs::write(out_path, &json)?;
            println!("SBOM written to {}", out_path.display());
            println!(
                "  Components: {} | Format: CycloneDX {}",
                sbom.components.len(),
                sbom.spec_version
            );
        }
        None => {
            println!("{}", json);
        }
    }

    Ok(())
}

fn cmd_badge(path: &Path, format: &str) -> Result<()> {
    if !path.is_file() {
        anyhow::bail!("'{}' is not a file.", path.display());
    }

    let dag = parse_pipeline(path)?;
    let report = analyzer::analyze(&dag);
    let badge = pipelinex_core::badge::generate_badge(&report);

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&badge)?);
        }
        "url" => {
            println!("{}", badge.shields_url);
        }
        _ => {
            println!("{}", badge.markdown);
            println!();
            println!(
                "  Score: {}/100 ({}) | {:.0}% optimized",
                badge.score, badge.grade, badge.optimization_pct
            );
            println!();
            println!("  Add the line above to your README.md");
        }
    }

    Ok(())
}

fn cmd_keys(command: KeysCommands) -> Result<()> {
    match command {
        KeysCommands::Generate { path } => {
            std::fs::create_dir_all(&path)?;

            let (private_key, public_key) = pipelinex_core::generate_keypair()?;

            let private_path = path.join("private.key");
            let public_path = path.join("public.key");

            std::fs::write(&private_path, &private_key)?;
            std::fs::write(&public_path, &public_key)?;

            println!("Ed25519 keypair generated:");
            println!("  Private key: {}", private_path.display());
            println!("  Public key:  {}", public_path.display());
            println!();
            println!(
                "Sign reports:  pipelinex analyze ci.yml --format json --sign {}",
                private_path.display()
            );
            println!(
                "Verify:        pipelinex verify report.json --key {}",
                public_path.display()
            );
            println!();
            println!("Keep your private key secure. Share only the public key.");

            Ok(())
        }
    }
}

fn cmd_verify(report_path: &Path, key: &str) -> Result<()> {
    let content = std::fs::read_to_string(report_path)
        .with_context(|| format!("Failed to read report: {}", report_path.display()))?;

    let signed: pipelinex_core::signing::SignedReport =
        serde_json::from_str(&content).context("Failed to parse signed report JSON")?;

    let public_key = read_key_material(key)?;

    let valid = pipelinex_core::verify_report(&signed, &public_key)?;

    if valid {
        println!("Signature VALID — report is authentic and untampered.");
        std::process::exit(0);
    } else {
        println!("Signature INVALID — report may have been tampered with!");
        std::process::exit(1);
    }
}

async fn cmd_explain(path: &Path, format: &str, runs_per_month: u32) -> Result<()> {
    let files = discover_workflow_files(path)?;
    if files.is_empty() {
        anyhow::bail!("No workflow files found at '{}'", path.display());
    }

    let explainer = pipelinex_core::explainer::Explainer::from_env();

    for file in &files {
        let dag = parse_pipeline(file)?;
        let report = analyzer::analyze(&dag);

        if report.findings.is_empty() {
            println!("No findings to explain for {}", file.display());
            continue;
        }

        let mut context = pipelinex_core::explainer::PipelineContext::from_dag(&dag);
        context.runs_per_month = runs_per_month;

        let explanations = explainer.explain_all(&report.findings, &context).await;

        match format {
            "json" => {
                println!("{}", serde_json::to_string_pretty(&explanations)?);
            }
            _ => {
                println!();
                println!(
                    " PipelineX Explain — {} ({} findings)",
                    file.display(),
                    explanations.len()
                );
                println!();
                print!(
                    "{}",
                    pipelinex_core::explainer::format_explanations(&explanations)
                );
            }
        }
    }

    Ok(())
}

fn cmd_whatif(path: &Path, modifications: &[String], format: &str) -> Result<()> {
    if !path.is_file() {
        anyhow::bail!("'{}' is not a file.", path.display());
    }

    let dag = parse_pipeline(path)?;

    if modifications.is_empty() {
        // Show available jobs and help
        println!();
        println!(" PipelineX What-If Simulator");
        println!();
        println!(" Available jobs in {}:", path.display());
        for job_id in dag.job_ids() {
            let job = dag.get_job(&job_id).unwrap();
            println!(
                "   {} ({:.0}s, depends on: [{}])",
                job_id,
                job.estimated_duration_secs,
                job.needs.join(", ")
            );
        }
        println!();
        println!(" Available modifications:");
        println!("   --modify \"add-cache <job> [savings_secs]\"");
        println!("   --modify \"remove-cache <job>\"");
        println!("   --modify \"remove-dep <from>-><to>\"");
        println!("   --modify \"add-dep <from>-><to>\"");
        println!("   --modify \"remove-job <job>\"");
        println!("   --modify \"set-duration <job> <seconds>\"");
        println!("   --modify \"change-runner <job> <runner>\"");
        println!();
        println!(" Example:");
        println!("   pipelinex what-if ci.yml --modify \"add-cache build 120\" --modify \"remove-dep lint->deploy\"");
        println!();
        return Ok(());
    }

    let mods: Vec<pipelinex_core::whatif::Modification> = modifications
        .iter()
        .map(|m| pipelinex_core::whatif::parse_modification(m))
        .collect::<Result<Vec<_>>>()?;

    let result = pipelinex_core::whatif::simulate(&dag, &mods);

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        _ => {
            display::print_whatif_result(&result);
        }
    }

    Ok(())
}

fn cmd_plugins(command: PluginCommands) -> Result<()> {
    match command {
        PluginCommands::Scaffold { path } => {
            plugins::scaffold_manifest(&path)?;
            println!("Plugin manifest scaffold ready: {}", path.display());
            Ok(())
        }
        PluginCommands::List { manifest, format } => {
            let loaded = if let Some(path) = manifest {
                plugins::load_manifest_from_path(path)?
            } else {
                plugins::load_manifest_from_env()?.unwrap_or_default()
            };

            #[derive(serde::Serialize)]
            struct Output {
                analyzers: Vec<String>,
                optimizers: Vec<String>,
            }

            let output = Output {
                analyzers: loaded.analyzers.iter().map(|p| p.id.clone()).collect(),
                optimizers: loaded.optimizers.iter().map(|p| p.id.clone()).collect(),
            };

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Analyzer plugins:");
                if output.analyzers.is_empty() {
                    println!("  (none)");
                } else {
                    for id in output.analyzers {
                        println!("  - {}", id);
                    }
                }

                println!("Optimizer plugins:");
                if output.optimizers.is_empty() {
                    println!("  (none)");
                } else {
                    for id in output.optimizers {
                        println!("  - {}", id);
                    }
                }
            }

            Ok(())
        }
    }
}
