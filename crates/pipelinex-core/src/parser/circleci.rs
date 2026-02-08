use crate::parser::dag::{PipelineDag, JobNode, StepInfo, CacheConfig};
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::Path;

/// Parser for CircleCI configuration files (.circleci/config.yml).
pub struct CircleCIParser;

impl CircleCIParser {
    /// Parse a CircleCI config from a file path.
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read CircleCI config file: {}", path.display()))?;
        Self::parse(&content, path.display().to_string())
    }

    /// Parse a CircleCI config from string content.
    pub fn parse(content: &str, source: String) -> Result<PipelineDag> {
        let yaml: Value = serde_yaml::from_str(content)
            .context("Failed to parse CircleCI YAML")?;

        let mut dag = PipelineDag::new(
            "CircleCI Pipeline".to_string(),
            source,
            "circleci".to_string(),
        );

        // Extract jobs from the config
        let jobs = yaml.get("jobs")
            .and_then(|j| j.as_mapping())
            .context("No jobs found in CircleCI config")?;

        for (job_name, job_spec) in jobs {
            let job_name_str = job_name.as_str().unwrap_or("unknown").to_string();
            let steps = Self::extract_steps(job_spec);
            let docker_image = Self::extract_docker_image(job_spec);
            let estimated_duration = Self::estimate_duration(&job_name_str, &steps);
            let caches = Self::detect_caches(job_spec, &steps);
            let env = Self::extract_environment(job_spec);

            let job = JobNode {
                id: job_name_str.clone(),
                name: job_name_str,
                steps,
                needs: vec![], // Will be filled in from workflows
                runs_on: docker_image,
                estimated_duration_secs: estimated_duration,
                caches,
                matrix: None,
                condition: None,
                env,
                paths_filter: None,
                paths_ignore: None,
            };

            dag.add_job(job);
        }

        // Parse workflows to build dependencies
        if let Some(workflows) = yaml.get("workflows").and_then(|w| w.as_mapping()) {
            for workflow in workflows.values() {
                if let Some(workflow_jobs) = workflow.get("jobs").and_then(|j| j.as_sequence()) {
                    Self::parse_workflow_dependencies(&mut dag, workflow_jobs)?;
                }
            }
        }

        Ok(dag)
    }

    fn extract_steps(job_spec: &Value) -> Vec<StepInfo> {
        let mut steps = Vec::new();

        if let Some(steps_list) = job_spec.get("steps").and_then(|s| s.as_sequence()) {
            for (i, step) in steps_list.iter().enumerate() {
                // CircleCI steps can be strings ("checkout") or mappings
                let step_name = if let Some(s) = step.as_str() {
                    s.to_string()
                } else if let Some(run) = step.get("run") {
                    if let Some(name) = run.get("name").and_then(|n| n.as_str()) {
                        name.to_string()
                    } else if let Some(cmd) = run.as_str() {
                        cmd.chars().take(50).collect()
                    } else {
                        format!("Step {}", i + 1)
                    }
                } else {
                    step.as_mapping()
                        .and_then(|m| m.keys().next())
                        .and_then(|k| k.as_str())
                        .unwrap_or("step")
                        .to_string()
                };

                let run_cmd = if let Some(run) = step.get("run") {
                    if let Some(cmd) = run.as_str() {
                        Some(cmd.to_string())
                    } else {
                        run.get("command").and_then(|c| c.as_str()).map(String::from)
                    }
                } else {
                    None
                };

                steps.push(StepInfo {
                    name: step_name,
                    uses: None,
                    run: run_cmd,
                    estimated_duration_secs: None,
                });
            }
        }

        steps
    }

    fn extract_docker_image(job_spec: &Value) -> String {
        if let Some(docker) = job_spec.get("docker").and_then(|d| d.as_sequence()) {
            if let Some(first) = docker.first() {
                if let Some(image) = first.get("image").and_then(|i| i.as_str()) {
                    return format!("docker:{}", image);
                }
            }
        }

        if let Some(machine) = job_spec.get("machine") {
            if let Some(image) = machine.get("image").and_then(|i| i.as_str()) {
                return format!("machine:{}", image);
            }
            return "machine:ubuntu".to_string();
        }

        if job_spec.get("macos").is_some() {
            return "macos".to_string();
        }

        "docker:cimg/base".to_string()
    }

    fn extract_environment(job_spec: &Value) -> HashMap<String, String> {
        let mut env = HashMap::new();

        if let Some(environment) = job_spec.get("environment").and_then(|e| e.as_mapping()) {
            for (key, value) in environment {
                if let (Some(k), Some(v)) = (key.as_str(), value.as_str()) {
                    env.insert(k.to_string(), v.to_string());
                }
            }
        }

        env
    }

    fn estimate_duration(job_name: &str, steps: &[StepInfo]) -> f64 {
        let name_lower = job_name.to_lowercase();

        let base = if name_lower.contains("build") {
            240.0
        } else if name_lower.contains("test") {
            300.0
        } else if name_lower.contains("deploy") {
            180.0
        } else if name_lower.contains("lint") {
            60.0
        } else {
            120.0
        };

        base + (steps.len() as f64 * 10.0)
    }

    fn detect_caches(job_spec: &Value, steps: &[StepInfo]) -> Vec<CacheConfig> {
        let mut caches = Vec::new();
        let mut found_types = std::collections::HashSet::new();

        // Check steps for cache commands
        for step in steps {
            if let Some(ref cmd) = step.run {
                let cmd_lower = cmd.to_lowercase();

                if (cmd_lower.contains("npm") || cmd_lower.contains("yarn"))
                    && !found_types.contains("node")
                {
                    caches.push(CacheConfig {
                        path: "node_modules".to_string(),
                        key_pattern: "node-{{ checksum \"package-lock.json\" }}".to_string(),
                        restore_keys: vec!["node-".to_string()],
                    });
                    found_types.insert("node");
                } else if cmd_lower.contains("pip") && !found_types.contains("pip") {
                    caches.push(CacheConfig {
                        path: "~/.cache/pip".to_string(),
                        key_pattern: "pip-{{ checksum \"requirements.txt\" }}".to_string(),
                        restore_keys: vec!["pip-".to_string()],
                    });
                    found_types.insert("pip");
                } else if (cmd_lower.contains("gradle") || cmd_lower.contains("./gradlew"))
                    && !found_types.contains("gradle")
                {
                    caches.push(CacheConfig {
                        path: "~/.gradle".to_string(),
                        key_pattern: "gradle-{{ checksum \"build.gradle\" }}".to_string(),
                        restore_keys: vec!["gradle-".to_string()],
                    });
                    found_types.insert("gradle");
                }
            }
        }

        // Check for explicit save_cache / restore_cache steps
        if let Some(steps_list) = job_spec.get("steps").and_then(|s| s.as_sequence()) {
            for step in steps_list {
                if step.get("save_cache").is_some() || step.get("restore_cache").is_some() {
                    // Already has caching configured
                    return vec![];
                }
            }
        }

        caches
    }

    fn parse_workflow_dependencies(
        dag: &mut PipelineDag,
        workflow_jobs: &[Value],
    ) -> Result<()> {
        for job_entry in workflow_jobs {
            // Jobs can be strings or mappings with "requires"
            let (job_name, requires) = if let Some(name) = job_entry.as_str() {
                (name.to_string(), vec![])
            } else if let Some(mapping) = job_entry.as_mapping() {
                let job_name = mapping
                    .keys()
                    .next()
                    .and_then(|k| k.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let requires = mapping
                    .values()
                    .next()
                    .and_then(|v| v.get("requires"))
                    .and_then(|r| r.as_sequence())
                    .map(|seq| {
                        seq.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                (job_name, requires)
            } else {
                continue;
            };

            // Update needs for this job
            for node_idx in dag.graph.node_indices() {
                let node = &mut dag.graph[node_idx];
                if node.id == job_name {
                    node.needs = requires.clone();
                    break;
                }
            }

            // Add dependencies
            for dep in &requires {
                dag.add_dependency(dep, &job_name)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_circleci() {
        let config = r#"
version: 2.1

jobs:
  build:
    docker:
      - image: cimg/node:18.0
    steps:
      - checkout
      - run: npm install
      - run: npm run build

  test:
    docker:
      - image: cimg/node:18.0
    steps:
      - checkout
      - run: npm install
      - run: npm test

workflows:
  main:
    jobs:
      - build
      - test:
          requires:
            - build
"#;

        let dag = CircleCIParser::parse(config, "config.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 2);
        assert_eq!(dag.provider, "circleci");

        let test_job = dag.get_job("test").unwrap();
        assert_eq!(test_job.needs, vec!["build"]);
    }

    #[test]
    fn test_parse_parallel_jobs() {
        let config = r#"
version: 2.1

jobs:
  lint:
    docker:
      - image: cimg/node:18.0
    steps:
      - checkout
      - run: npm run lint

  unit:
    docker:
      - image: cimg/node:18.0
    steps:
      - checkout
      - run: npm test

  integration:
    docker:
      - image: cimg/node:18.0
    steps:
      - checkout
      - run: npm run test:integration

workflows:
  main:
    jobs:
      - lint
      - unit
      - integration
"#;

        let dag = CircleCIParser::parse(config, "config.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 3);

        // All three should be root jobs (no dependencies)
        assert_eq!(dag.root_jobs().len(), 3);
    }

    #[test]
    fn test_detect_docker_image() {
        let config = r#"
version: 2.1

jobs:
  build:
    docker:
      - image: cimg/python:3.10
    steps:
      - checkout
      - run: pip install -r requirements.txt

workflows:
  main:
    jobs:
      - build
"#;

        let dag = CircleCIParser::parse(config, "config.yml".to_string()).unwrap();
        let build_job = dag.get_job("build").unwrap();
        assert!(build_job.runs_on.contains("python"));
    }
}
