use pipelinex_core::analyzer;
use pipelinex_core::optimizer::docker_opt;
use pipelinex_core::optimizer::Optimizer;
use pipelinex_core::parser::aws_codepipeline::AwsCodePipelineParser;
use pipelinex_core::parser::azure::AzurePipelinesParser;
use pipelinex_core::parser::bitbucket::BitbucketParser;
use pipelinex_core::parser::circleci::CircleCIParser;
use pipelinex_core::parser::github::GitHubActionsParser;
use pipelinex_core::parser::gitlab::GitLabCIParser;
use pipelinex_core::parser::jenkins::JenkinsParser;
use std::path::{Path, PathBuf};

/// Get the workspace root (two levels up from CARGO_MANIFEST_DIR of pipelinex-core).
fn fixtures_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // workspace root
        .join("tests/fixtures")
}

fn github_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("github-actions").join(name)
}

fn gitlab_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("gitlab-ci").join(name)
}

fn docker_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("dockerfiles").join(name)
}

fn jenkins_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("jenkins").join(name)
}

fn circleci_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("circleci").join(name)
}

fn bitbucket_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("bitbucket").join(name)
}

fn azure_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("azure-pipelines").join(name)
}

fn aws_codepipeline_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("aws-codepipeline").join(name)
}

// ─── GitHub Actions integration tests ───

#[test]
fn test_analyze_unoptimized_fullstack() {
    let path = github_fixture("unoptimized-fullstack.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert_eq!(report.job_count, 6);
    assert!(report.findings.len() >= 3, "Expected at least 3 findings");
    assert!(
        report.potential_improvement_pct() > 10.0,
        "Expected significant improvement potential"
    );
    assert!(report
        .findings
        .iter()
        .any(|f| f.category == pipelinex_core::analyzer::report::FindingCategory::MissingCache));
}

#[test]
fn test_analyze_optimized_example_has_fewer_findings() {
    let path = github_fixture("optimized-example.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    let critical = report.critical_count();
    assert!(
        critical <= 4,
        "Optimized pipeline should have few critical findings, got {}",
        critical
    );
}

#[test]
fn test_analyze_rust_project() {
    let path = github_fixture("rust-project.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert!(report.job_count >= 3);
    assert!(
        report.findings.iter().any(|f| {
            f.category == pipelinex_core::analyzer::report::FindingCategory::SerialBottleneck
                || f.category == pipelinex_core::analyzer::report::FindingCategory::CriticalPath
        }),
        "Should detect bottleneck or false dependency in serial Rust pipeline"
    );
}

#[test]
fn test_analyze_monorepo_ci() {
    let path = github_fixture("monorepo-ci.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert!(report.job_count >= 6);
    assert!(report.max_parallelism >= 2);
}

#[test]
fn test_analyze_docker_publish() {
    let path = github_fixture("docker-publish.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert_eq!(report.job_count, 3);
    assert!(
        report.max_parallelism >= 2,
        "lint and test should run in parallel"
    );
}

#[test]
fn test_optimize_produces_valid_yaml() {
    let path = github_fixture("unoptimized-fullstack.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);
    let optimized = Optimizer::optimize(&path, &report).unwrap();

    let parsed: serde_yaml::Value = serde_yaml::from_str(&optimized).unwrap();
    assert!(
        parsed.get("jobs").is_some(),
        "Optimized YAML should have jobs"
    );
}

#[test]
fn test_sarif_output_valid_json() {
    let path = github_fixture("unoptimized-fullstack.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);
    let sarif = pipelinex_core::analyzer::sarif::to_sarif(&report);
    let json = serde_json::to_string_pretty(&sarif).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        parsed["version"].as_str().unwrap(),
        "2.1.0",
        "SARIF version should be 2.1.0"
    );
}

#[test]
fn test_graph_outputs() {
    let path = github_fixture("unoptimized-fullstack.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();

    let mermaid = pipelinex_core::graph::to_mermaid(&dag);
    assert!(mermaid.contains("graph LR"));
    assert!(mermaid.contains("-->"));

    let dot = pipelinex_core::graph::to_dot(&dag);
    assert!(dot.contains("digraph"));
    assert!(dot.contains("->"));

    let ascii = pipelinex_core::graph::to_ascii(&dag);
    assert!(!ascii.is_empty());
}

#[test]
fn test_simulation_with_fixture() {
    let path = github_fixture("unoptimized-fullstack.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();
    let result = pipelinex_core::simulator::simulate(&dag, 200, 0.15);

    assert_eq!(result.runs, 200);
    assert!(result.mean_duration_secs > 0.0);
    assert!(result.p90_duration_secs >= result.p50_duration_secs);
    assert_eq!(result.job_stats.len(), 6);
    assert!(!result.histogram.is_empty());
}

#[test]
fn test_cost_estimation() {
    let path = github_fixture("unoptimized-fullstack.yml");
    let dag = GitHubActionsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    let estimate = pipelinex_core::cost::estimate_costs(
        report.total_estimated_duration_secs,
        report.optimized_duration_secs,
        500,
        "ubuntu-latest",
        150.0,
        10,
    );

    assert!(estimate.compute_cost_per_run > 0.0);
    assert!(estimate.monthly_compute_cost > 0.0);
    assert!(estimate.waste_ratio > 0.0);
}

// ─── GitLab CI integration tests ───

#[test]
fn test_analyze_gitlab_simple() {
    let path = gitlab_fixture("simple-pipeline.yml");
    let dag = GitLabCIParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert_eq!(report.provider, "gitlab-ci");
    assert!(report.job_count >= 3);
}

#[test]
fn test_analyze_gitlab_monorepo() {
    let path = gitlab_fixture("monorepo-pipeline.yml");
    let dag = GitLabCIParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert!(report.job_count >= 5);
}

#[test]
fn test_analyze_gitlab_kubernetes() {
    let path = gitlab_fixture("kubernetes-deploy.yml");
    let dag = GitLabCIParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert!(report.job_count >= 4);
}

// ─── Docker optimization integration tests ───

#[test]
fn test_docker_analyze_unoptimized_node() {
    let content = std::fs::read_to_string(docker_fixture("unoptimized-node.Dockerfile")).unwrap();
    let analysis = docker_opt::analyze_dockerfile(&content);

    assert!(
        analysis.findings.len() >= 2,
        "Should detect multiple issues"
    );
    assert!(
        analysis
            .findings
            .iter()
            .any(|f| f.title.contains("COPY . . before")),
        "Should detect COPY before install"
    );
    assert!(analysis.optimized_dockerfile.is_some());
}

#[test]
fn test_docker_analyze_optimized_has_fewer_issues() {
    let content = std::fs::read_to_string(docker_fixture("optimized-node.Dockerfile")).unwrap();
    let analysis = docker_opt::analyze_dockerfile(&content);

    let unoptimized =
        std::fs::read_to_string(docker_fixture("unoptimized-node.Dockerfile")).unwrap();
    let unopt_analysis = docker_opt::analyze_dockerfile(&unoptimized);

    assert!(
        analysis.findings.len() <= unopt_analysis.findings.len(),
        "Optimized Dockerfile should have fewer findings ({} vs {})",
        analysis.findings.len(),
        unopt_analysis.findings.len()
    );
}

#[test]
fn test_docker_analyze_python() {
    let content = std::fs::read_to_string(docker_fixture("python-app.Dockerfile")).unwrap();
    let analysis = docker_opt::analyze_dockerfile(&content);

    assert!(!analysis.findings.is_empty());
    assert!(
        analysis
            .findings
            .iter()
            .any(|f| f.title.contains("Non-slim")),
        "Should detect non-slim Python image"
    );
}

#[test]
fn test_docker_analyze_go() {
    let content = std::fs::read_to_string(docker_fixture("go-service.Dockerfile")).unwrap();
    let analysis = docker_opt::analyze_dockerfile(&content);

    assert!(!analysis.findings.is_empty());
    assert!(
        analysis
            .findings
            .iter()
            .any(|f| f.title.contains("multi-stage")),
        "Should recommend multi-stage build for Go"
    );
}

// ─── Jenkins integration tests ───

#[test]
fn test_analyze_jenkins_simple() {
    let path = jenkins_fixture("simple-pipeline.jenkinsfile");
    let dag = JenkinsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert_eq!(report.provider, "jenkins");
    assert!(report.job_count >= 3);
}

#[test]
fn test_analyze_jenkins_parallel() {
    let path = jenkins_fixture("parallel-pipeline.jenkinsfile");
    let dag = JenkinsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert!(report.job_count >= 5);
}

#[test]
fn test_analyze_jenkins_microservices() {
    let path = jenkins_fixture("microservices-pipeline.jenkinsfile");
    let dag = JenkinsParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert!(report.job_count >= 5);
    // Should detect opportunities for parallelization
    assert!(report.max_parallelism >= 1);
}

// ─── CircleCI integration tests ───

#[test]
fn test_analyze_circleci_config() {
    let path = circleci_fixture("config.yml");
    let dag = CircleCIParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert_eq!(report.provider, "circleci");
    assert_eq!(report.job_count, 5);
    assert!(
        report.max_parallelism >= 2,
        "lint and test should run in parallel"
    );
}

#[test]
fn test_circleci_workflow_dependencies() {
    let path = circleci_fixture("config.yml");
    let dag = CircleCIParser::parse_file(&path).unwrap();

    // Verify the workflow dependency chain
    let deploy_job = dag.get_job("deploy").unwrap();
    assert_eq!(deploy_job.needs, vec!["build"]);

    let build_job = dag.get_job("build").unwrap();
    assert!(build_job.needs.contains(&"lint".to_string()));
    assert!(build_job.needs.contains(&"test".to_string()));
}

#[test]
fn test_circleci_detects_bottlenecks() {
    let path = circleci_fixture("config.yml");
    let dag = CircleCIParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    // Should detect serial bottlenecks or missing caches
    assert!(
        !report.findings.is_empty(),
        "Should detect optimization opportunities"
    );
    assert!(
        report.findings.iter().any(|f| {
            f.category == pipelinex_core::analyzer::report::FindingCategory::SerialBottleneck
                || f.category == pipelinex_core::analyzer::report::FindingCategory::MissingCache
        }),
        "Should detect bottlenecks or missing caches"
    );
}

// ─── Bitbucket Pipelines integration tests ───

#[test]
fn test_analyze_bitbucket_pipelines() {
    let path = bitbucket_fixture("bitbucket-pipelines.yml");
    let dag = BitbucketParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert_eq!(report.provider, "bitbucket");
    assert!(
        report.job_count >= 5,
        "Should have multiple jobs from different pipeline types"
    );
    assert!(
        report.max_parallelism >= 3,
        "Should detect parallel execution"
    );
}

#[test]
fn test_bitbucket_parallel_detection() {
    let path = bitbucket_fixture("bitbucket-pipelines.yml");
    let dag = BitbucketParser::parse_file(&path).unwrap();

    // Verify parallel steps are correctly parsed
    let lint = dag.get_job("main-lint");
    let unit_tests = dag.get_job("main-unit-tests");
    let integration_tests = dag.get_job("main-integration-tests");

    assert!(lint.is_some());
    assert!(unit_tests.is_some());
    assert!(integration_tests.is_some());

    // All three should have the same dependency (install)
    let lint_needs = &lint.unwrap().needs;
    let unit_needs = &unit_tests.unwrap().needs;
    let integration_needs = &integration_tests.unwrap().needs;

    assert_eq!(lint_needs, unit_needs);
    assert_eq!(unit_needs, integration_needs);
}

#[test]
fn test_bitbucket_finds_optimizations() {
    let path = bitbucket_fixture("bitbucket-pipelines.yml");
    let dag = BitbucketParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    // Should detect missing caches and parallel opportunities
    assert!(!report.findings.is_empty());
    assert!(
        report.potential_improvement_pct() > 50.0,
        "Should find significant optimizations"
    );
}

// ─── Azure Pipelines integration tests ───

#[test]
fn test_analyze_azure_pipeline() {
    let path = azure_fixture("azure-stages-jobs.yml");
    let dag = AzurePipelinesParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert_eq!(report.provider, "azure-pipelines");
    assert_eq!(report.job_count, 3);
    assert!(report.max_parallelism >= 1);
}

#[test]
fn test_azure_stage_and_job_dependencies() {
    let path = azure_fixture("azure-stages-jobs.yml");
    let dag = AzurePipelinesParser::parse_file(&path).unwrap();

    let unit_tests = dag.get_job("test-unittests").unwrap();
    assert!(unit_tests.needs.contains(&"build-buildapp".to_string()));

    let deploy = dag.get_job("deploy-deployprod").unwrap();
    assert!(deploy.needs.contains(&"test-unittests".to_string()));
}

// ─── AWS CodePipeline integration tests ───

#[test]
fn test_analyze_aws_codepipeline() {
    let path = aws_codepipeline_fixture("codepipeline.json");
    let dag = AwsCodePipelineParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert_eq!(report.provider, "aws-codepipeline");
    assert_eq!(report.job_count, 4);
    assert!(report.findings.len() >= 1);
}

#[test]
fn test_aws_codepipeline_action_dependencies() {
    let path = aws_codepipeline_fixture("codepipeline.json");
    let dag = AwsCodePipelineParser::parse_file(&path).unwrap();

    let integration = dag.get_job("build-integrationtests").unwrap();
    assert!(integration.needs.contains(&"build-lintandunit".to_string()));
    assert!(integration.needs.contains(&"source-sourceaction".to_string()));

    let deploy = dag.get_job("deploy-deploytoecs").unwrap();
    assert!(deploy.needs.contains(&"build-lintandunit".to_string()));
    assert!(deploy.needs.contains(&"build-integrationtests".to_string()));
}
