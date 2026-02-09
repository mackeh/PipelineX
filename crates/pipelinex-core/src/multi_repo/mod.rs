use crate::analyzer::critical_path;
use crate::analyzer::report::Severity;
use crate::parser::dag::PipelineDag;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// A parsed pipeline associated with a repository name.
#[derive(Debug, Clone)]
pub struct RepoPipeline {
    pub repo: String,
    pub dag: PipelineDag,
}

/// Aggregated summary for one repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    pub repo: String,
    pub workflow_count: usize,
    pub total_jobs: usize,
    pub max_critical_path_secs: f64,
    pub providers: Vec<String>,
}

/// A detected orchestration relationship between repositories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationEdge {
    pub from_repo: String,
    pub to_repo: String,
    pub source_file: String,
    pub trigger_hint: String,
    pub confidence: f64,
}

/// A cross-repository optimization finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRepoFinding {
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub affected_repos: Vec<String>,
    pub estimated_savings_secs: Option<f64>,
    pub confidence: f64,
}

/// Full report for multi-repository analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRepoReport {
    pub repo_count: usize,
    pub workflow_count: usize,
    pub repos: Vec<RepoSummary>,
    pub orchestration_edges: Vec<OrchestrationEdge>,
    pub findings: Vec<MultiRepoFinding>,
}

/// Analyze multiple repositories and detect orchestration patterns.
pub fn analyze_multi_repo(pipelines: &[RepoPipeline]) -> MultiRepoReport {
    let repos = summarize_repositories(pipelines);
    let edges = detect_cross_repo_edges(pipelines);

    let mut findings = Vec::new();
    findings.extend(detect_orchestration_fanout(&edges));
    findings.extend(detect_orchestration_fanin(&edges));
    findings.extend(detect_repeated_commands(pipelines));
    findings.extend(detect_monorepo_orchestration_risk(pipelines));
    findings.extend(detect_duration_skew(&repos));
    findings.sort_by_key(|f| std::cmp::Reverse(f.severity.priority()));

    MultiRepoReport {
        repo_count: repos.len(),
        workflow_count: pipelines.len(),
        repos,
        orchestration_edges: edges,
        findings,
    }
}

fn summarize_repositories(pipelines: &[RepoPipeline]) -> Vec<RepoSummary> {
    #[derive(Default)]
    struct Acc {
        workflow_count: usize,
        total_jobs: usize,
        max_critical_path_secs: f64,
        providers: BTreeSet<String>,
    }

    let mut by_repo: BTreeMap<String, Acc> = BTreeMap::new();
    for pipeline in pipelines {
        let entry = by_repo.entry(pipeline.repo.clone()).or_default();
        entry.workflow_count += 1;
        entry.total_jobs += pipeline.dag.job_count();
        entry.providers.insert(pipeline.dag.provider.clone());
        let duration = critical_path::find_critical_path(&pipeline.dag).1;
        if duration > entry.max_critical_path_secs {
            entry.max_critical_path_secs = duration;
        }
    }

    by_repo
        .into_iter()
        .map(|(repo, acc)| RepoSummary {
            repo,
            workflow_count: acc.workflow_count,
            total_jobs: acc.total_jobs,
            max_critical_path_secs: acc.max_critical_path_secs,
            providers: acc.providers.into_iter().collect(),
        })
        .collect()
}

fn detect_cross_repo_edges(pipelines: &[RepoPipeline]) -> Vec<OrchestrationEdge> {
    let repo_names = pipelines
        .iter()
        .map(|pipeline| pipeline.repo.to_lowercase())
        .collect::<BTreeSet<_>>();

    let mut edges = Vec::new();
    for pipeline in pipelines {
        let from_repo = pipeline.repo.to_lowercase();
        for job in pipeline.dag.graph.node_weights() {
            for step in &job.steps {
                let mut text = String::new();
                if let Some(uses) = &step.uses {
                    text.push_str(uses);
                    text.push('\n');
                }
                if let Some(run) = &step.run {
                    text.push_str(run);
                }
                if text.trim().is_empty() {
                    continue;
                }

                let lower = text.to_lowercase();
                if !looks_like_orchestration_step(&lower) {
                    continue;
                }

                for target_repo in &repo_names {
                    if target_repo == &from_repo {
                        continue;
                    }
                    if contains_repo_reference(&lower, target_repo) {
                        let (hint, confidence) = detect_trigger_hint(&lower);
                        edges.push(OrchestrationEdge {
                            from_repo: pipeline.repo.clone(),
                            to_repo: target_repo.clone(),
                            source_file: pipeline.dag.source_file.clone(),
                            trigger_hint: hint.to_string(),
                            confidence,
                        });
                    }
                }
            }
        }
    }

    edges.sort_by(|a, b| {
        a.from_repo
            .cmp(&b.from_repo)
            .then(a.to_repo.cmp(&b.to_repo))
            .then(a.source_file.cmp(&b.source_file))
    });
    edges.dedup_by(|a, b| {
        a.from_repo == b.from_repo
            && a.to_repo == b.to_repo
            && a.source_file == b.source_file
            && a.trigger_hint == b.trigger_hint
    });
    edges
}

fn detect_orchestration_fanout(edges: &[OrchestrationEdge]) -> Vec<MultiRepoFinding> {
    let mut outgoing: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for edge in edges {
        outgoing
            .entry(&edge.from_repo)
            .or_default()
            .insert(&edge.to_repo);
    }

    let mut findings = Vec::new();
    for (repo, targets) in outgoing {
        if targets.len() >= 2 {
            findings.push(MultiRepoFinding {
                severity: Severity::High,
                title: format!("Repository '{}' is a cross-repo orchestration hub", repo),
                description: format!(
                    "Detected fan-out from '{}' to {} repositories. This can become a \
                    bottleneck and create cascading failures when upstream pipelines retry.",
                    repo,
                    targets.len()
                ),
                recommendation: "Introduce explicit contracts (versioned artifacts/events), \
                    add retry/backoff boundaries, and split independent downstream triggers."
                    .to_string(),
                affected_repos: std::iter::once(repo.to_string())
                    .chain(targets.into_iter().map(ToString::to_string))
                    .collect(),
                estimated_savings_secs: Some(300.0),
                confidence: 0.82,
            });
        }
    }
    findings
}

fn detect_orchestration_fanin(edges: &[OrchestrationEdge]) -> Vec<MultiRepoFinding> {
    let mut incoming: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for edge in edges {
        incoming
            .entry(&edge.to_repo)
            .or_default()
            .insert(&edge.from_repo);
    }

    let mut findings = Vec::new();
    for (repo, sources) in incoming {
        if sources.len() >= 2 {
            findings.push(MultiRepoFinding {
                severity: Severity::Medium,
                title: format!("Repository '{}' has multi-source trigger fan-in", repo),
                description: format!(
                    "Detected orchestration fan-in from {} upstream repositories. \
                    Without deduplication/idempotency, duplicated builds are likely.",
                    sources.len()
                ),
                recommendation: "Gate downstream execution with event correlation keys \
                    and deduplicate repeated triggers for the same commit/version."
                    .to_string(),
                affected_repos: std::iter::once(repo.to_string())
                    .chain(sources.into_iter().map(ToString::to_string))
                    .collect(),
                estimated_savings_secs: Some(180.0),
                confidence: 0.76,
            });
        }
    }
    findings
}

fn detect_repeated_commands(pipelines: &[RepoPipeline]) -> Vec<MultiRepoFinding> {
    let mut commands: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for pipeline in pipelines {
        for job in pipeline.dag.graph.node_weights() {
            for step in &job.steps {
                let Some(run) = &step.run else {
                    continue;
                };

                for raw_line in run.lines() {
                    let normalized = normalize_command(raw_line);
                    if !is_reusable_ci_command(&normalized) {
                        continue;
                    }

                    commands
                        .entry(normalized)
                        .or_default()
                        .insert(pipeline.repo.clone());
                }
            }
        }
    }

    let repeated = commands
        .iter()
        .filter_map(|(cmd, repos)| (repos.len() >= 2).then_some((cmd, repos)))
        .collect::<Vec<_>>();

    if repeated.is_empty() {
        return Vec::new();
    }

    let mut repos = BTreeSet::new();
    let mut examples = Vec::new();
    for (cmd, cmd_repos) in repeated.iter().take(5) {
        repos.extend(cmd_repos.iter().cloned());
        examples.push(format!("`{}` ({} repos)", cmd, cmd_repos.len()));
    }

    vec![MultiRepoFinding {
        severity: Severity::Medium,
        title: "Standardize repeated commands across repositories".to_string(),
        description: format!(
            "Detected repeated CI commands across repositories: {}.",
            examples.join(", ")
        ),
        recommendation: "Create shared templates/reusable workflows for repeated command \
            sequences to reduce maintenance overhead and drift."
            .to_string(),
        affected_repos: repos.into_iter().collect(),
        estimated_savings_secs: Some((repeated.len() as f64).min(6.0) * 60.0),
        confidence: 0.74,
    }]
}

fn detect_monorepo_orchestration_risk(pipelines: &[RepoPipeline]) -> Vec<MultiRepoFinding> {
    let mut by_repo: BTreeMap<&str, Vec<&PipelineDag>> = BTreeMap::new();
    for pipeline in pipelines {
        by_repo
            .entry(&pipeline.repo)
            .or_default()
            .push(&pipeline.dag);
    }

    let mut findings = Vec::new();
    for (repo, dags) in by_repo {
        if dags.len() < 3 {
            continue;
        }

        let push_or_pr_without_paths = dags
            .iter()
            .filter(|dag| {
                dag.triggers.iter().any(|trigger| {
                    (trigger.event == "push" || trigger.event == "pull_request")
                        && trigger.paths.is_none()
                        && trigger.paths_ignore.is_none()
                })
            })
            .count();

        if push_or_pr_without_paths >= 2 {
            findings.push(MultiRepoFinding {
                severity: Severity::High,
                title: format!("Monorepo orchestration risk in '{}'", repo),
                description: format!(
                    "Repository '{}' has {} workflows triggered on push/PR without path \
                    filters. This often causes full-fanout CI runs for small scoped changes.",
                    repo, push_or_pr_without_paths
                ),
                recommendation: "Add path-scoped triggers per workflow and introduce an \
                    orchestrator workflow that routes only impacted packages/services."
                    .to_string(),
                affected_repos: vec![repo.to_string()],
                estimated_savings_secs: Some(push_or_pr_without_paths as f64 * 90.0),
                confidence: 0.8,
            });
        }
    }

    findings
}

fn detect_duration_skew(repos: &[RepoSummary]) -> Vec<MultiRepoFinding> {
    if repos.len() < 3 {
        return Vec::new();
    }

    let mut durations = repos
        .iter()
        .map(|repo| repo.max_critical_path_secs)
        .collect::<Vec<_>>();
    durations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = durations[durations.len() / 2];
    if median <= 0.0 {
        return Vec::new();
    }

    let Some(slowest) = repos.iter().max_by(|a, b| {
        a.max_critical_path_secs
            .partial_cmp(&b.max_critical_path_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
    }) else {
        return Vec::new();
    };

    if slowest.max_critical_path_secs > median * 1.8 {
        return vec![MultiRepoFinding {
            severity: Severity::Medium,
            title: format!(
                "Repository '{}' is significantly slower than peer median",
                slowest.repo
            ),
            description: format!(
                "Slowest critical path is {:.0}s vs median {:.0}s across analyzed repositories.",
                slowest.max_critical_path_secs, median
            ),
            recommendation: "Prioritize optimization on the slowest repository and align \
                orchestration boundaries so downstream repos do not wait on non-critical jobs."
                .to_string(),
            affected_repos: vec![slowest.repo.clone()],
            estimated_savings_secs: Some((slowest.max_critical_path_secs - median) * 0.35),
            confidence: 0.72,
        }];
    }

    Vec::new()
}

fn looks_like_orchestration_step(text: &str) -> bool {
    let keywords = [
        "repository_dispatch",
        "workflow_dispatch",
        "gh workflow run",
        "trigger pipeline",
        "pipeline trigger",
        "/dispatches",
        "/pipelines",
    ];
    keywords.iter().any(|kw| text.contains(kw))
}

fn contains_repo_reference(text: &str, repo_name: &str) -> bool {
    let slash_pattern = format!("/{}", repo_name);
    let direct_pattern = format!("repo {}", repo_name);
    text.contains(&slash_pattern) || text.contains(repo_name) || text.contains(&direct_pattern)
}

fn detect_trigger_hint(text: &str) -> (&'static str, f64) {
    if text.contains("repository_dispatch") || text.contains("/dispatches") {
        return ("repository_dispatch", 0.9);
    }
    if text.contains("workflow_dispatch") || text.contains("gh workflow run") {
        return ("workflow_dispatch", 0.85);
    }
    if text.contains("pipeline trigger") || text.contains("trigger pipeline") {
        return ("pipeline_trigger", 0.75);
    }
    ("cross_repo_reference", 0.6)
}

fn normalize_command(line: &str) -> String {
    line.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

fn is_reusable_ci_command(command: &str) -> bool {
    if command.is_empty() {
        return false;
    }

    let prefixes = [
        "cargo test",
        "cargo check",
        "cargo build",
        "npm ci",
        "npm install",
        "npm test",
        "npm run build",
        "pnpm install",
        "pnpm test",
        "yarn install",
        "yarn test",
        "pip install",
        "pytest",
        "go test",
        "go build",
        "mvn test",
        "mvn package",
        "gradle test",
        "./gradlew test",
        "./gradlew build",
    ];
    prefixes.iter().any(|prefix| command.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::github::GitHubActionsParser;

    fn parse(repo: &str, file: &str, yaml: &str) -> RepoPipeline {
        let dag = GitHubActionsParser::parse(yaml, file.to_string()).unwrap();
        RepoPipeline {
            repo: repo.to_string(),
            dag,
        }
    }

    #[test]
    fn detects_cross_repo_orchestration_hub() {
        let orchestrator = parse(
            "orchestrator",
            ".github/workflows/release.yml",
            r#"
name: release
on: push
jobs:
  dispatch:
    runs-on: ubuntu-latest
    steps:
      - run: gh workflow run deploy.yml --repo acme/api-service
      - run: gh workflow run deploy.yml --repo acme/web-app
"#,
        );
        let api = parse(
            "api-service",
            ".github/workflows/ci.yml",
            r#"
name: ci
on: push
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --all
"#,
        );
        let web = parse(
            "web-app",
            ".github/workflows/ci.yml",
            r#"
name: ci
on: push
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - run: npm test
"#,
        );

        let report = analyze_multi_repo(&[orchestrator, api, web]);
        assert!(report
            .orchestration_edges
            .iter()
            .any(|edge| edge.from_repo == "orchestrator" && edge.to_repo == "api-service"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.title.contains("cross-repo orchestration hub")));
    }

    #[test]
    fn detects_repeated_ci_commands_across_repos() {
        let repo_a = parse(
            "repo-a",
            ".github/workflows/ci.yml",
            r#"
name: ci
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: npm ci
      - run: npm test
"#,
        );
        let repo_b = parse(
            "repo-b",
            ".github/workflows/ci.yml",
            r#"
name: ci
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: npm ci
      - run: npm test
"#,
        );

        let report = analyze_multi_repo(&[repo_a, repo_b]);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.title.contains("Standardize repeated commands")));
    }

    #[test]
    fn detects_monorepo_risk_without_path_filters() {
        let repo = "platform-monorepo";
        let wf1 = parse(
            repo,
            ".github/workflows/service-a.yml",
            r#"
name: service-a
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: npm ci
"#,
        );
        let wf2 = parse(
            repo,
            ".github/workflows/service-b.yml",
            r#"
name: service-b
on:
  pull_request:
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - run: npm test
"#,
        );
        let wf3 = parse(
            repo,
            ".github/workflows/service-c.yml",
            r#"
name: service-c
on: push
jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - run: npm run lint
"#,
        );

        let report = analyze_multi_repo(&[wf1, wf2, wf3]);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.title.contains("Monorepo orchestration risk")));
    }
}
