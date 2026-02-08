use crate::parser::dag::*;
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Parser for Buildkite pipeline files (`.buildkite/pipeline.yml`).
///
/// Supported constructs:
/// - `steps` command entries
/// - `depends_on` relationships
/// - `wait` / `block` barrier semantics
/// - plugin and artifact metadata
pub struct BuildkiteParser;

impl BuildkiteParser {
    /// Parse a Buildkite pipeline file into a Pipeline DAG.
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read Buildkite file: {}", path.display()))?;
        Self::parse(&content, path.to_string_lossy().to_string())
    }

    /// Parse Buildkite YAML content into a Pipeline DAG.
    pub fn parse(content: &str, source_file: String) -> Result<PipelineDag> {
        let yaml: Value = serde_yaml::from_str(content).context("Failed to parse YAML")?;

        let name = yaml
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Buildkite Pipeline")
            .to_string();

        let mut dag = PipelineDag::new(name, source_file, "buildkite".to_string());
        dag.env = parse_env(yaml.get("env"));

        let steps = yaml
            .get("steps")
            .and_then(|v| v.as_sequence())
            .context("No 'steps' section found in Buildkite pipeline")?;

        let mut alias_map: HashMap<String, String> = HashMap::new();
        let mut raw_needs: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_prior_jobs: Vec<String> = Vec::new();
        let mut barrier_needs: Vec<String> = Vec::new();
        let mut synthetic_idx = 0usize;

        for (step_idx, step) in steps.iter().enumerate() {
            if is_wait_or_block(step) {
                barrier_needs = all_prior_jobs.clone();
                continue;
            }

            let parsed = parse_step(step, step_idx, &mut synthetic_idx)?;
            let mut needs = parsed.depends_on_raw.clone();
            if needs.is_empty() && !barrier_needs.is_empty() {
                needs = barrier_needs.clone();
            }

            let mut job = parsed.job;
            job.needs = needs.clone();
            raw_needs.insert(job.id.clone(), needs);
            dag.add_job(job.clone());

            for alias in parsed.aliases {
                alias_map.insert(alias, job.id.clone());
            }

            all_prior_jobs.push(job.id);
        }

        let mut dedup = HashSet::new();
        let mut resolved_needs: HashMap<String, Vec<String>> = HashMap::new();

        for (job_id, deps) in raw_needs {
            let mut resolved = Vec::new();
            for dep in deps {
                let dep_id = if dag.get_job(&dep).is_some() {
                    Some(dep.clone())
                } else {
                    alias_map.get(&dep).cloned()
                };

                if let Some(dep_id) = dep_id {
                    if dep_id == job_id {
                        continue;
                    }
                    if dedup.insert((dep_id.clone(), job_id.clone())) {
                        let _ = dag.add_dependency(&dep_id, &job_id);
                    }
                    if !resolved.contains(&dep_id) {
                        resolved.push(dep_id);
                    }
                }
            }
            resolved_needs.insert(job_id, resolved);
        }

        for (job_id, needs) in resolved_needs {
            if let Some(idx) = dag.node_map.get(&job_id).copied() {
                dag.graph[idx].needs = needs;
            }
        }

        Ok(dag)
    }
}

struct ParsedStep {
    job: JobNode,
    aliases: Vec<String>,
    depends_on_raw: Vec<String>,
}

fn parse_step(step: &Value, step_idx: usize, synthetic_idx: &mut usize) -> Result<ParsedStep> {
    let label = step
        .get("label")
        .and_then(|v| v.as_str())
        .or_else(|| step.get("name").and_then(|v| v.as_str()))
        .or_else(|| step.get("command").and_then(|v| v.as_str()))
        .unwrap_or("step")
        .to_string();

    let key = step
        .get("key")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            *synthetic_idx += 1;
            format!("step-{}-{}", step_idx + 1, synthetic_idx)
        });

    let id = sanitize_id(&key);
    let mut job = JobNode::new(id.clone(), label.clone());

    job.runs_on = parse_agents(step.get("agents"))
        .unwrap_or_else(|| "buildkite:agent".to_string());
    job.condition = step.get("if").and_then(|v| v.as_str()).map(String::from);
    job.env = parse_env(step.get("env"));

    let mut steps = extract_command_steps(step);
    let plugin_steps = extract_plugin_steps(step);
    if !plugin_steps.is_empty() {
        steps.extend(plugin_steps);
    }
    if steps.is_empty() {
        steps.push(StepInfo {
            name: "step".to_string(),
            uses: None,
            run: Some("buildkite step".to_string()),
            estimated_duration_secs: Some(45.0),
        });
    }

    job.caches = detect_caches(&steps, step);
    job.steps = steps;
    job.estimated_duration_secs = job
        .steps
        .iter()
        .filter_map(|s| s.estimated_duration_secs)
        .sum::<f64>()
        .max(20.0);

    if let Some(parallelism) = step.get("parallelism").and_then(|v| v.as_u64()) {
        if parallelism > 1 {
            let shards: Vec<String> = (1..=parallelism).map(|i| i.to_string()).collect();
            let mut vars = HashMap::new();
            vars.insert("BUILDKITE_PARALLEL_JOB".to_string(), shards.clone());
            job.matrix = Some(MatrixStrategy {
                variables: vars,
                total_combinations: shards.len(),
            });
        }
    }

    let mut aliases = vec![id.clone(), key.clone()];
    let label_alias = sanitize_id(&label);
    if !label_alias.is_empty() {
        aliases.push(label_alias);
    }

    Ok(ParsedStep {
        job,
        aliases,
        depends_on_raw: parse_depends_on(step.get("depends_on")),
    })
}

fn is_wait_or_block(step: &Value) -> bool {
    if step.get("wait").is_some() || step.get("block").is_some() {
        return true;
    }
    if let Some(s) = step.as_str() {
        return s.eq_ignore_ascii_case("wait");
    }
    if let Some(step_type) = step.get("type").and_then(|v| v.as_str()) {
        let lower = step_type.to_lowercase();
        return lower == "wait" || lower == "block";
    }
    false
}

fn parse_agents(agents: Option<&Value>) -> Option<String> {
    let Some(agents) = agents else {
        return None;
    };

    if let Some(s) = agents.as_str() {
        return Some(format!("buildkite:{s}"));
    }

    if let Some(map) = agents.as_mapping() {
        if let Some(queue) = map
            .get(Value::String("queue".to_string()))
            .and_then(|v| v.as_str())
        {
            return Some(format!("buildkite:{queue}"));
        }
        if let Some(role) = map
            .get(Value::String("role".to_string()))
            .and_then(|v| v.as_str())
        {
            return Some(format!("buildkite:{role}"));
        }
    }

    None
}

fn parse_depends_on(value: Option<&Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };

    match value {
        Value::String(s) => vec![sanitize_id(s)],
        Value::Sequence(seq) => seq
            .iter()
            .filter_map(|item| match item {
                Value::String(s) => Some(sanitize_id(s)),
                Value::Mapping(map) => map
                    .get(Value::String("step".to_string()))
                    .and_then(|v| v.as_str())
                    .map(sanitize_id),
                _ => None,
            })
            .collect(),
        Value::Mapping(map) => map
            .get(Value::String("step".to_string()))
            .and_then(|v| v.as_str())
            .map(sanitize_id)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_env(env: Option<&Value>) -> HashMap<String, String> {
    let mut parsed = HashMap::new();
    let Some(env) = env else {
        return parsed;
    };

    if let Some(map) = env.as_mapping() {
        for (key, value) in map {
            if let Some(k) = key.as_str() {
                let rendered = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => continue,
                };
                parsed.insert(k.to_string(), rendered);
            }
        }
    }

    parsed
}

fn extract_command_steps(step: &Value) -> Vec<StepInfo> {
    let mut parsed = Vec::new();

    if let Some(command) = step.get("command").and_then(|v| v.as_str()) {
        parsed.push(StepInfo {
            name: "command".to_string(),
            uses: None,
            run: Some(command.to_string()),
            estimated_duration_secs: Some(estimate_cmd_duration(command)),
        });
    }

    if let Some(commands) = step.get("commands").and_then(|v| v.as_sequence()) {
        for (idx, cmd) in commands.iter().enumerate() {
            if let Some(cmd) = cmd.as_str() {
                parsed.push(StepInfo {
                    name: format!("command[{idx}]"),
                    uses: None,
                    run: Some(cmd.to_string()),
                    estimated_duration_secs: Some(estimate_cmd_duration(cmd)),
                });
            }
        }
    }

    if let Some(plugins) = step.get("plugins").and_then(|v| v.as_sequence()) {
        for plugin in plugins {
            if let Some(plugin_str) = plugin.as_str() {
                parsed.push(StepInfo {
                    name: "plugin".to_string(),
                    uses: Some(plugin_str.to_string()),
                    run: None,
                    estimated_duration_secs: Some(10.0),
                });
            }
        }
    }

    parsed
}

fn extract_plugin_steps(step: &Value) -> Vec<StepInfo> {
    let mut parsed = Vec::new();
    let Some(plugins) = step.get("plugins") else {
        return parsed;
    };

    if let Some(seq) = plugins.as_sequence() {
        for entry in seq {
            match entry {
                Value::String(plugin) => parsed.push(StepInfo {
                    name: "plugin".to_string(),
                    uses: Some(plugin.to_string()),
                    run: None,
                    estimated_duration_secs: Some(10.0),
                }),
                Value::Mapping(map) => {
                    for (plugin_name, _) in map {
                        if let Some(plugin_name) = plugin_name.as_str() {
                            parsed.push(StepInfo {
                                name: "plugin".to_string(),
                                uses: Some(plugin_name.to_string()),
                                run: None,
                                estimated_duration_secs: Some(10.0),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    } else if let Some(map) = plugins.as_mapping() {
        for (plugin_name, _) in map {
            if let Some(plugin_name) = plugin_name.as_str() {
                parsed.push(StepInfo {
                    name: "plugin".to_string(),
                    uses: Some(plugin_name.to_string()),
                    run: None,
                    estimated_duration_secs: Some(10.0),
                });
            }
        }
    }

    parsed
}

fn detect_caches(steps: &[StepInfo], step: &Value) -> Vec<CacheConfig> {
    let mut caches = Vec::new();

    let artifact_paths: Vec<String> = step
        .get("artifact_paths")
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(vec![s.to_string()])
            } else {
                v.as_sequence().map(|seq| {
                    seq.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
            }
        })
        .unwrap_or_default();

    if !artifact_paths.is_empty() {
        caches.push(CacheConfig {
            path: artifact_paths.join(","),
            key_pattern: "buildkite-artifacts".to_string(),
            restore_keys: Vec::new(),
        });
    }

    for step in steps {
        let uses = step.uses.as_deref().unwrap_or("").to_lowercase();
        let run = step.run.as_deref().unwrap_or("").to_lowercase();
        if uses.contains("cache")
            || run.contains("npm ci")
            || run.contains("npm install")
            || run.contains("pip install")
            || run.contains("cargo build")
            || run.contains("docker build")
        {
            caches.push(CacheConfig {
                path: "detected".to_string(),
                key_pattern: "detected".to_string(),
                restore_keys: Vec::new(),
            });
            break;
        }
    }

    caches
}

fn estimate_cmd_duration(cmd: &str) -> f64 {
    let cmd = cmd.to_lowercase();
    if cmd.contains("npm ci") || cmd.contains("npm install") {
        return 180.0;
    }
    if cmd.contains("yarn install") || cmd.contains("pnpm install") {
        return 170.0;
    }
    if cmd.contains("pip install") {
        return 140.0;
    }
    if cmd.contains("cargo build") || cmd.contains("go build") {
        return 260.0;
    }
    if cmd.contains("npm run build") || cmd.contains("make build") {
        return 240.0;
    }
    if cmd.contains("npm test") || cmd.contains("pytest") || cmd.contains("cargo test") {
        return 300.0;
    }
    if cmd.contains("docker build") {
        return 280.0;
    }
    if cmd.contains("deploy") {
        return 180.0;
    }
    60.0
}

fn sanitize_id(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut prev_dash = false;
    for ch in value.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };
        if mapped == '-' {
            if !prev_dash {
                out.push(mapped);
                prev_dash = true;
            }
        } else {
            out.push(mapped);
            prev_dash = false;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_depends_and_wait_barrier() {
        let config = r#"
steps:
  - label: ":package: Install"
    key: install
    command: npm ci
  - wait: ~
  - label: ":lint-roller: Lint"
    key: lint
    command: npm run lint
  - label: ":test_tube: Test"
    key: test
    depends_on: lint
    command: npm test
"#;

        let dag = BuildkiteParser::parse(config, ".buildkite/pipeline.yml".to_string()).unwrap();
        assert_eq!(dag.provider, "buildkite");
        assert_eq!(dag.job_count(), 3);

        let lint = dag.get_job("lint").unwrap();
        assert!(lint.needs.contains(&"install".to_string()));

        let test = dag.get_job("test").unwrap();
        assert_eq!(test.needs, vec!["lint"]);
    }

    #[test]
    fn test_parse_plugins_artifacts_parallelism() {
        let config = r#"
steps:
  - label: Build
    key: build
    agents:
      queue: linux-large
    commands:
      - npm ci
      - npm run build
    plugins:
      - docker#v5.10.0:
          image: node:20
    artifact_paths:
      - dist/**
    parallelism: 3
"#;

        let dag = BuildkiteParser::parse(config, ".buildkite/pipeline.yml".to_string()).unwrap();
        let build = dag.get_job("build").unwrap();
        assert_eq!(build.runs_on, "buildkite:linux-large");
        assert!(build.matrix.is_some());
        assert!(build.steps.iter().any(|s| s.uses.is_some()));
        assert!(!build.caches.is_empty());
    }
}
