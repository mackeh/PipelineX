use crate::parser::dag::PipelineDag;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// CycloneDX BOM format for CI pipeline components.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiSbom {
    pub bom_format: String,
    pub spec_version: String,
    pub version: u32,
    pub metadata: SbomMetadata,
    pub components: Vec<SbomComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbomMetadata {
    pub timestamp: String,
    pub tools: Vec<SbomTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbomTool {
    pub vendor: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SbomComponent {
    #[serde(rename = "type")]
    pub component_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Generate a CycloneDX SBOM from one or more pipeline DAGs.
pub fn generate_sbom(dags: &[&PipelineDag]) -> CiSbom {
    let mut components = BTreeSet::new();

    for dag in dags {
        for node in dag.graph.node_weights() {
            for step in &node.steps {
                if let Some(uses) = &step.uses {
                    if let Some(component) = parse_uses_to_component(uses) {
                        components.insert(component);
                    }
                }

                // Extract Docker images from run steps
                if let Some(run) = &step.run {
                    for component in extract_docker_images(run) {
                        components.insert(component);
                    }
                }
            }

            // Runner as a component
            if !node.runs_on.is_empty() {
                components.insert(SbomComponent {
                    component_type: "operating-system".to_string(),
                    name: node.runs_on.clone(),
                    version: None,
                    purl: None,
                    description: Some("CI runner image".to_string()),
                });
            }
        }
    }

    CiSbom {
        bom_format: "CycloneDX".to_string(),
        spec_version: "1.5".to_string(),
        version: 1,
        metadata: SbomMetadata {
            timestamp: chrono::Utc::now().to_rfc3339(),
            tools: vec![SbomTool {
                vendor: "PipelineX".to_string(),
                name: "pipelinex".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }],
        },
        components: components.into_iter().collect(),
    }
}

fn parse_uses_to_component(uses: &str) -> Option<SbomComponent> {
    // Skip local actions and docker:// protocol
    if uses.starts_with("./") {
        return None;
    }

    if uses.starts_with("docker://") {
        let image = uses.strip_prefix("docker://")?;
        let (name, version) = split_at_version(image);
        return Some(SbomComponent {
            component_type: "container".to_string(),
            name: name.to_string(),
            version: version.map(|v| v.to_string()),
            purl: Some(format!(
                "pkg:docker/{}{}",
                name,
                version.map(|v| format!("@{}", v)).unwrap_or_default()
            )),
            description: Some("Docker image used in CI step".to_string()),
        });
    }

    // GitHub Action: owner/repo@ref
    let (action, version) = if let Some(idx) = uses.find('@') {
        (&uses[..idx], Some(&uses[idx + 1..]))
    } else {
        (uses, None)
    };

    Some(SbomComponent {
        component_type: "application".to_string(),
        name: action.to_string(),
        version: version.map(|v| v.to_string()),
        purl: Some(format!(
            "pkg:github/{}{}",
            action,
            version.map(|v| format!("@{}", v)).unwrap_or_default()
        )),
        description: None,
    })
}

fn split_at_version(image: &str) -> (&str, Option<&str>) {
    if let Some(idx) = image.rfind(':') {
        // Don't split on port-like patterns
        let after = &image[idx + 1..];
        if after.contains('/') {
            (image, None)
        } else {
            (&image[..idx], Some(after))
        }
    } else {
        (image, None)
    }
}

fn extract_docker_images(run: &str) -> Vec<SbomComponent> {
    let mut components = Vec::new();
    let re = regex::Regex::new(r"docker\s+(?:run|pull|build\s+.*--from=)\s+([^\s]+)").unwrap();

    for cap in re.captures_iter(run) {
        let image = &cap[1];
        // Skip variable references
        if image.contains('$') {
            continue;
        }
        let (name, version) = split_at_version(image);
        components.push(SbomComponent {
            component_type: "container".to_string(),
            name: name.to_string(),
            version: version.map(|v| v.to_string()),
            purl: Some(format!(
                "pkg:docker/{}{}",
                name,
                version.map(|v| format!("@{}", v)).unwrap_or_default()
            )),
            description: Some("Docker image referenced in run step".to_string()),
        });
    }

    components
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dag::{JobNode, PipelineDag, StepInfo};

    #[test]
    fn test_parse_github_action() {
        let component = parse_uses_to_component("actions/checkout@v4").unwrap();
        assert_eq!(component.name, "actions/checkout");
        assert_eq!(component.version.as_deref(), Some("v4"));
        assert!(component.purl.unwrap().contains("pkg:github/"));
    }

    #[test]
    fn test_parse_docker_uses() {
        let component = parse_uses_to_component("docker://node:20-slim").unwrap();
        assert_eq!(component.name, "node");
        assert_eq!(component.version.as_deref(), Some("20-slim"));
        assert_eq!(component.component_type, "container");
    }

    #[test]
    fn test_skip_local_action() {
        let component = parse_uses_to_component("./.github/actions/my-action");
        assert!(component.is_none());
    }

    #[test]
    fn test_generate_sbom() {
        let mut dag = PipelineDag::new("ci".into(), "ci.yml".into(), "github-actions".into());
        let mut job = JobNode::new("build".into(), "Build".into());
        job.steps.push(StepInfo {
            name: "Checkout".into(),
            uses: Some("actions/checkout@v4".into()),
            run: None,
            estimated_duration_secs: None,
        });
        job.steps.push(StepInfo {
            name: "Build".into(),
            uses: None,
            run: Some("docker run node:20 npm test".into()),
            estimated_duration_secs: None,
        });
        dag.add_job(job);

        let sbom = generate_sbom(&[&dag]);
        assert_eq!(sbom.bom_format, "CycloneDX");
        assert!(!sbom.components.is_empty());
        assert!(sbom.components.iter().any(|c| c.name == "actions/checkout"));
    }

    #[test]
    fn test_extract_docker_images() {
        let images =
            extract_docker_images("docker run node:20-slim npm test && docker pull redis:7");
        assert_eq!(images.len(), 2);
        assert!(images.iter().any(|c| c.name == "node"));
        assert!(images.iter().any(|c| c.name == "redis"));
    }
}
