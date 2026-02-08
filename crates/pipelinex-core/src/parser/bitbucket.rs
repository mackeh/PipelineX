use crate::parser::dag::{PipelineDag, JobNode, StepInfo, CacheConfig};
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::Path;

/// Parser for Bitbucket Pipelines configuration files (bitbucket-pipelines.yml).
pub struct BitbucketParser;

impl BitbucketParser {
    /// Parse a Bitbucket Pipelines config from a file path.
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read Bitbucket Pipelines file: {}", path.display()))?;
        Self::parse(&content, path.display().to_string())
    }

    /// Parse a Bitbucket Pipelines config from string content.
    pub fn parse(content: &str, source: String) -> Result<PipelineDag> {
        let yaml: Value = serde_yaml::from_str(content)
            .context("Failed to parse Bitbucket Pipelines YAML")?;

        let mut dag = PipelineDag::new(
            "Bitbucket Pipeline".to_string(),
            source,
            "bitbucket".to_string(),
        );

        // Get default image
        let default_image = yaml
            .get("image")
            .and_then(|i| i.as_str())
            .unwrap_or("atlassian/default-image")
            .to_string();

        let pipelines = yaml
            .get("pipelines")
            .and_then(|p| p.as_mapping())
            .context("No pipelines found in Bitbucket config")?;

        let mut step_counter = 0;

        // Parse default pipeline
        if let Some(default) = pipelines.get("default") {
            Self::parse_pipeline_steps(
                &mut dag,
                default,
                &default_image,
                &mut step_counter,
                "default",
            )?;
        }

        // Parse branch-specific pipelines
        if let Some(branches) = pipelines.get("branches").and_then(|b| b.as_mapping()) {
            for (branch_name, steps) in branches {
                let branch = branch_name.as_str().unwrap_or("unknown");
                Self::parse_pipeline_steps(
                    &mut dag,
                    steps,
                    &default_image,
                    &mut step_counter,
                    branch,
                )?;
            }
        }

        // Parse pull-request pipelines
        if let Some(prs) = pipelines.get("pull-requests").and_then(|p| p.as_mapping()) {
            for (pattern, steps) in prs {
                let pr_pattern = pattern.as_str().unwrap_or("**");
                Self::parse_pipeline_steps(
                    &mut dag,
                    steps,
                    &default_image,
                    &mut step_counter,
                    &format!("pr-{}", pr_pattern),
                )?;
            }
        }

        Ok(dag)
    }

    fn parse_pipeline_steps(
        dag: &mut PipelineDag,
        pipeline: &Value,
        default_image: &str,
        step_counter: &mut usize,
        branch: &str,
    ) -> Result<()> {
        let steps = pipeline
            .as_sequence()
            .context("Pipeline should be a sequence of steps")?;

        let mut previous_jobs = Vec::new();

        for step_or_parallel in steps {
            // Check if it's a parallel block or a single step
            if let Some(step) = step_or_parallel.get("step") {
                // Single step
                let job = Self::parse_step(step, default_image, step_counter, branch)?;
                let job_id = job.id.clone();

                // Add dependencies on previous jobs
                let mut job_with_deps = job;
                job_with_deps.needs = previous_jobs.clone();

                dag.add_job(job_with_deps);

                // Add edges from previous jobs
                for prev in &previous_jobs {
                    dag.add_dependency(prev, &job_id)?;
                }

                previous_jobs = vec![job_id];
            } else if let Some(parallel) = step_or_parallel.get("parallel") {
                // Parallel steps
                let parallel_steps = parallel
                    .as_sequence()
                    .context("Parallel block should be a sequence")?;

                let mut parallel_job_ids = Vec::new();

                for parallel_step in parallel_steps {
                    if let Some(step) = parallel_step.get("step") {
                        let job = Self::parse_step(step, default_image, step_counter, branch)?;
                        let job_id = job.id.clone();

                        // Add dependencies on previous jobs
                        let mut job_with_deps = job;
                        job_with_deps.needs = previous_jobs.clone();

                        dag.add_job(job_with_deps);

                        // Add edges from previous jobs
                        for prev in &previous_jobs {
                            dag.add_dependency(prev, &job_id)?;
                        }

                        parallel_job_ids.push(job_id);
                    }
                }

                previous_jobs = parallel_job_ids;
            }
        }

        Ok(())
    }

    fn parse_step(
        step: &Value,
        default_image: &str,
        step_counter: &mut usize,
        branch: &str,
    ) -> Result<JobNode> {
        *step_counter += 1;

        let name = step
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(&format!("Step {}", step_counter))
            .to_string();

        let id = format!("{}-{}", branch, name.to_lowercase().replace(' ', "-"));

        // Parse script
        let steps = Self::extract_steps(step);

        // Parse image
        let image = step
            .get("image")
            .and_then(|i| i.as_str())
            .unwrap_or(default_image)
            .to_string();

        // Parse caches
        let caches = Self::extract_caches(step);

        // Estimate duration
        let estimated_duration = Self::estimate_duration(&name, &steps);

        // Parse deployment environment
        let deployment = step
            .get("deployment")
            .and_then(|d| d.as_str())
            .map(|d| format!("deployment:{}", d));

        Ok(JobNode {
            id,
            name,
            steps,
            needs: vec![],
            runs_on: format!("bitbucket:{}", image),
            estimated_duration_secs: estimated_duration,
            caches,
            matrix: None,
            condition: deployment,
            env: HashMap::new(),
            paths_filter: None,
            paths_ignore: None,
        })
    }

    fn extract_steps(step: &Value) -> Vec<StepInfo> {
        let mut steps = Vec::new();

        if let Some(script) = step.get("script").and_then(|s| s.as_sequence()) {
            for (i, cmd) in script.iter().enumerate() {
                if let Some(cmd_str) = cmd.as_str() {
                    steps.push(StepInfo {
                        name: format!("Script {}", i + 1),
                        uses: None,
                        run: Some(cmd_str.to_string()),
                        estimated_duration_secs: None,
                    });
                }
            }
        }

        steps
    }

    fn extract_caches(step: &Value) -> Vec<CacheConfig> {
        let mut caches = Vec::new();
        let mut found_types = std::collections::HashSet::new();

        if let Some(cache_list) = step.get("caches").and_then(|c| c.as_sequence()) {
            for cache in cache_list {
                if let Some(cache_name) = cache.as_str() {
                    match cache_name {
                        "node" if !found_types.contains("node") => {
                            caches.push(CacheConfig {
                                path: "node_modules".to_string(),
                                key_pattern: "node-{{ checksum \"package-lock.json\" }}".to_string(),
                                restore_keys: vec!["node-".to_string()],
                            });
                            found_types.insert("node");
                        }
                        "pip" if !found_types.contains("pip") => {
                            caches.push(CacheConfig {
                                path: "~/.cache/pip".to_string(),
                                key_pattern: "pip-{{ checksum \"requirements.txt\" }}".to_string(),
                                restore_keys: vec!["pip-".to_string()],
                            });
                            found_types.insert("pip");
                        }
                        "maven" if !found_types.contains("maven") => {
                            caches.push(CacheConfig {
                                path: "~/.m2/repository".to_string(),
                                key_pattern: "maven-{{ checksum \"pom.xml\" }}".to_string(),
                                restore_keys: vec!["maven-".to_string()],
                            });
                            found_types.insert("maven");
                        }
                        "gradle" if !found_types.contains("gradle") => {
                            caches.push(CacheConfig {
                                path: "~/.gradle".to_string(),
                                key_pattern: "gradle-{{ checksum \"build.gradle\" }}".to_string(),
                                restore_keys: vec!["gradle-".to_string()],
                            });
                            found_types.insert("gradle");
                        }
                        _ => {}
                    }
                }
            }
        }

        caches
    }

    fn estimate_duration(name: &str, steps: &[StepInfo]) -> f64 {
        let name_lower = name.to_lowercase();

        let base = if name_lower.contains("deploy") {
            180.0
        } else if name_lower.contains("build") {
            240.0
        } else if name_lower.contains("test") {
            300.0
        } else if name_lower.contains("lint") {
            60.0
        } else {
            120.0
        };

        base + (steps.len() as f64 * 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pipeline() {
        let config = r#"
image: node:18

pipelines:
  default:
    - step:
        name: Build
        script:
          - npm install
          - npm run build
"#;

        let dag = BitbucketParser::parse(config, "bitbucket-pipelines.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 1);
        assert_eq!(dag.provider, "bitbucket");
    }

    #[test]
    fn test_parse_parallel_steps() {
        let config = r#"
image: node:18

pipelines:
  default:
    - step:
        name: Install
        script:
          - npm ci
    - parallel:
        - step:
            name: Lint
            script:
              - npm run lint
        - step:
            name: Test
            script:
              - npm test
"#;

        let dag = BitbucketParser::parse(config, "bitbucket-pipelines.yml".to_string()).unwrap();
        assert_eq!(dag.job_count(), 3);

        // Lint and Test should both depend on Install
        let lint = dag.get_job("default-lint").unwrap();
        let test = dag.get_job("default-test").unwrap();

        assert_eq!(lint.needs, vec!["default-install"]);
        assert_eq!(test.needs, vec!["default-install"]);
    }

    #[test]
    fn test_parse_with_caches() {
        let config = r#"
image: node:18

pipelines:
  default:
    - step:
        name: Build
        caches:
          - node
        script:
          - npm ci
          - npm run build
"#;

        let dag = BitbucketParser::parse(config, "bitbucket-pipelines.yml".to_string()).unwrap();
        let job = dag.get_job("default-build").unwrap();

        assert!(!job.caches.is_empty());
        assert!(job.caches[0].path.contains("node_modules"));
    }

    #[test]
    fn test_parse_deployment_step() {
        let config = r#"
image: node:18

pipelines:
  branches:
    main:
      - step:
          name: Deploy
          deployment: production
          script:
            - npm run deploy
"#;

        let dag = BitbucketParser::parse(config, "bitbucket-pipelines.yml".to_string()).unwrap();
        let job = dag.get_job("main-deploy").unwrap();

        assert!(job.condition.is_some());
        assert!(job.condition.as_ref().unwrap().contains("production"));
    }
}
