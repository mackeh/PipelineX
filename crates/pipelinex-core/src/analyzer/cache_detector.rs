use crate::parser::dag::PipelineDag;
use crate::analyzer::report::{Finding, FindingCategory, Severity};
use regex::Regex;

/// Detect missing dependency caches in the pipeline.
pub fn detect_missing_caches(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    for job in dag.graph.node_weights() {
        let has_cache_action = job.steps.iter().any(|s| {
            s.uses.as_ref().is_some_and(|u| u.starts_with("actions/cache"))
        });

        for step in &job.steps {
            if let Some(run) = &step.run {
                let cmd = run.to_lowercase();

                // npm/yarn/pnpm
                if !has_cache_action && is_npm_install(&cmd) {
                    findings.push(Finding {
                        severity: Severity::Critical,
                        category: FindingCategory::MissingCache,
                        title: "No dependency caching for npm/yarn/pnpm".to_string(),
                        description: format!(
                            "Job '{}' runs '{}' without caching node_modules. \
                            This reinstalls all dependencies on every run.",
                            job.id,
                            run.lines().next().unwrap_or(run).trim(),
                        ),
                        affected_jobs: vec![job.id.clone()],
                        recommendation: "Add actions/cache for node_modules keyed on package-lock.json hash, \
                            or use setup-node with built-in caching."
                            .to_string(),
                        fix_command: Some("pipelinex optimize --apply cache".to_string()),
                        estimated_savings_secs: Some(150.0), // ~2.5 min
                        confidence: 0.95,
                        auto_fixable: true,
                    });
                }

                // pip
                if !has_cache_action && is_pip_install(&cmd) {
                    findings.push(Finding {
                        severity: Severity::Critical,
                        category: FindingCategory::MissingCache,
                        title: "No dependency caching for pip".to_string(),
                        description: format!(
                            "Job '{}' runs pip install without caching. \
                            This downloads and installs packages from scratch on every run.",
                            job.id,
                        ),
                        affected_jobs: vec![job.id.clone()],
                        recommendation: "Add actions/cache for pip, keyed on requirements.txt hash."
                            .to_string(),
                        fix_command: Some("pipelinex optimize --apply cache".to_string()),
                        estimated_savings_secs: Some(90.0),
                        confidence: 0.93,
                        auto_fixable: true,
                    });
                }

                // cargo
                if !has_cache_action && is_cargo_build(&cmd) {
                    findings.push(Finding {
                        severity: Severity::Critical,
                        category: FindingCategory::MissingCache,
                        title: "No build caching for Cargo".to_string(),
                        description: format!(
                            "Job '{}' runs cargo build without caching target/ directory. \
                            Rust compilation from scratch is extremely slow.",
                            job.id,
                        ),
                        affected_jobs: vec![job.id.clone()],
                        recommendation: "Add actions/cache or use Swatinem/rust-cache for \
                            target/ and ~/.cargo/registry."
                            .to_string(),
                        fix_command: Some("pipelinex optimize --apply cache".to_string()),
                        estimated_savings_secs: Some(240.0),
                        confidence: 0.95,
                        auto_fixable: true,
                    });
                }

                // gradle/maven
                if !has_cache_action && is_gradle_or_maven(&cmd) {
                    findings.push(Finding {
                        severity: Severity::High,
                        category: FindingCategory::MissingCache,
                        title: "No dependency caching for Gradle/Maven".to_string(),
                        description: format!(
                            "Job '{}' runs a Gradle/Maven build without caching ~/.gradle or ~/.m2.",
                            job.id,
                        ),
                        affected_jobs: vec![job.id.clone()],
                        recommendation: "Add actions/cache for Gradle/Maven dependencies."
                            .to_string(),
                        fix_command: Some("pipelinex optimize --apply cache".to_string()),
                        estimated_savings_secs: Some(120.0),
                        confidence: 0.90,
                        auto_fixable: true,
                    });
                }

                // Docker build without layer caching
                if is_docker_build(&cmd) {
                    let has_docker_cache = job.steps.iter().any(|s| {
                        s.uses.as_ref().is_some_and(|u| u.starts_with("docker/build-push-action"))
                    });
                    if !has_docker_cache && !cmd.contains("--cache-from") {
                        findings.push(Finding {
                            severity: Severity::High,
                            category: FindingCategory::DockerOptimization,
                            title: "Docker build has no layer caching".to_string(),
                            description: format!(
                                "Job '{}' runs docker build without layer caching. \
                                Every build starts from scratch.",
                                job.id,
                            ),
                            affected_jobs: vec![job.id.clone()],
                            recommendation: "Use docker/build-push-action with cache-from/cache-to, \
                                or add --cache-from flag."
                                .to_string(),
                            fix_command: Some("pipelinex optimize --apply docker".to_string()),
                            estimated_savings_secs: Some(240.0),
                            confidence: 0.88,
                            auto_fixable: true,
                        });
                    }
                }
            }
        }
    }

    findings
}

fn is_npm_install(cmd: &str) -> bool {
    let re = Regex::new(r"(npm\s+(ci|install)|yarn\s+install|pnpm\s+install)").unwrap();
    re.is_match(cmd)
}

fn is_pip_install(cmd: &str) -> bool {
    cmd.contains("pip install") || cmd.contains("pip3 install")
}

fn is_cargo_build(cmd: &str) -> bool {
    cmd.contains("cargo build") || cmd.contains("cargo test") || cmd.contains("cargo clippy")
}

fn is_gradle_or_maven(cmd: &str) -> bool {
    cmd.contains("./gradlew") || cmd.contains("gradle ") || cmd.contains("mvn ") || cmd.contains("./mvnw")
}

fn is_docker_build(cmd: &str) -> bool {
    cmd.contains("docker build") || cmd.contains("docker buildx")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::github::GitHubActionsParser;

    #[test]
    fn test_detect_missing_npm_cache() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm ci
      - run: npm run build
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let findings = detect_missing_caches(&dag);
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.title.contains("npm")));
    }

    #[test]
    fn test_no_warning_when_cache_present() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/cache@v4
        with:
          path: node_modules
          key: node-modules
      - run: npm ci
"#;
        let dag = GitHubActionsParser::parse(yaml, "ci.yml".to_string()).unwrap();
        let findings = detect_missing_caches(&dag);
        let npm_findings: Vec<_> = findings.iter()
            .filter(|f| f.title.contains("npm"))
            .collect();
        assert!(npm_findings.is_empty());
    }
}
