use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Result of discovering CI configs across a monorepo.
#[derive(Debug, Clone)]
pub struct DiscoveredPipeline {
    pub package_name: String,
    pub file_path: PathBuf,
    pub relative_path: String,
}

/// Monorepo discovery result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MonorepoDiscovery {
    pub root: String,
    pub packages: Vec<PackageInfo>,
    pub total_pipeline_files: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PackageInfo {
    pub name: String,
    pub path: String,
    pub pipeline_files: Vec<String>,
}

const CI_PATTERNS: &[&str] = &[
    ".github/workflows/*.yml",
    ".github/workflows/*.yaml",
    ".gitlab-ci.yml",
    ".gitlab-ci.yaml",
    "Jenkinsfile",
    ".circleci/config.yml",
    ".circleci/config.yaml",
    "bitbucket-pipelines.yml",
    "bitbucket-pipelines.yaml",
    "azure-pipelines.yml",
    "azure-pipelines.yaml",
    ".buildkite/pipeline.yml",
    ".buildkite/pipeline.yaml",
    "codepipeline.yml",
    "codepipeline.yaml",
    "codepipeline.json",
];

/// Recursively discover CI pipeline files in a monorepo up to `max_depth` levels.
pub fn discover_monorepo(root: &Path, max_depth: usize) -> Result<Vec<DiscoveredPipeline>> {
    if !root.exists() {
        anyhow::bail!("Path '{}' does not exist", root.display());
    }
    if !root.is_dir() {
        anyhow::bail!("'{}' is not a directory", root.display());
    }

    let mut results = Vec::new();

    // First, check root for CI configs
    discover_at_path(root, root, &mut results)?;

    // Recurse into subdirectories
    walk_dirs(root, root, 0, max_depth, &mut results)?;

    results.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    results.dedup_by(|a, b| a.file_path == b.file_path);

    Ok(results)
}

fn walk_dirs(
    root: &Path,
    current: &Path,
    depth: usize,
    max_depth: usize,
    results: &mut Vec<DiscoveredPipeline>,
) -> Result<()> {
    if depth >= max_depth {
        return Ok(());
    }

    let entries = std::fs::read_dir(current)
        .with_context(|| format!("Failed to read directory '{}'", current.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden dirs, build artifacts, and node_modules
        if name_str.starts_with('.')
            || name_str == "target"
            || name_str == "node_modules"
            || name_str == "vendor"
            || name_str == "dist"
            || name_str == "build"
            || name_str == "__pycache__"
        {
            continue;
        }

        discover_at_path(&path, root, results)?;
        walk_dirs(root, &path, depth + 1, max_depth, results)?;
    }

    Ok(())
}

fn discover_at_path(dir: &Path, root: &Path, results: &mut Vec<DiscoveredPipeline>) -> Result<()> {
    let package_name = infer_package_name(dir, root);

    for pattern in CI_PATTERNS {
        let full_pattern = format!("{}/{}", dir.display(), pattern);
        if let Ok(entries) = glob::glob(&full_pattern) {
            for entry in entries.flatten() {
                if entry.is_file() {
                    let relative = entry
                        .strip_prefix(root)
                        .unwrap_or(&entry)
                        .display()
                        .to_string();
                    results.push(DiscoveredPipeline {
                        package_name: package_name.clone(),
                        file_path: entry,
                        relative_path: relative,
                    });
                }
            }
        }
    }

    // Check fixed-name files
    for fixed in &[
        ".gitlab-ci.yml",
        ".gitlab-ci.yaml",
        "Jenkinsfile",
        "bitbucket-pipelines.yml",
        "bitbucket-pipelines.yaml",
        "azure-pipelines.yml",
        "azure-pipelines.yaml",
        "codepipeline.yml",
        "codepipeline.yaml",
        "codepipeline.json",
    ] {
        let path = dir.join(fixed);
        if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            results.push(DiscoveredPipeline {
                package_name: package_name.clone(),
                file_path: path,
                relative_path: relative,
            });
        }
    }

    Ok(())
}

fn infer_package_name(dir: &Path, root: &Path) -> String {
    if dir == root {
        return "(root)".to_string();
    }

    // Try to read package name from common config files
    // package.json
    let pkg_json = dir.join("package.json");
    if pkg_json.is_file() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                    return name.to_string();
                }
            }
        }
    }

    // Cargo.toml
    let cargo_toml = dir.join("Cargo.toml");
    if cargo_toml.is_file() {
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            if let Ok(toml_val) = content.parse::<toml::Table>() {
                if let Some(name) = toml_val
                    .get("package")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                {
                    return name.to_string();
                }
            }
        }
    }

    // Fall back to directory name
    dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Aggregate discovery results into a structured report.
pub fn aggregate_discovery(root: &Path, pipelines: &[DiscoveredPipeline]) -> MonorepoDiscovery {
    let mut packages: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for p in pipelines {
        packages
            .entry(p.package_name.clone())
            .or_default()
            .push(p.relative_path.clone());
    }

    let mut package_infos: Vec<PackageInfo> = packages
        .into_iter()
        .map(|(name, files)| PackageInfo {
            path: if name == "(root)" {
                ".".to_string()
            } else {
                name.clone()
            },
            name,
            pipeline_files: files,
        })
        .collect();
    package_infos.sort_by(|a, b| a.name.cmp(&b.name));

    MonorepoDiscovery {
        root: root.display().to_string(),
        packages: package_infos,
        total_pipeline_files: pipelines.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_monorepo_nonexistent() {
        let result = discover_monorepo(Path::new("/nonexistent/path"), 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_discover_monorepo_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = discover_monorepo(tmp.path(), 5).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_discover_monorepo_with_github_actions() {
        let tmp = tempfile::tempdir().unwrap();
        let workflows = tmp.path().join(".github/workflows");
        fs::create_dir_all(&workflows).unwrap();
        fs::write(workflows.join("ci.yml"), "name: CI").unwrap();

        let result = discover_monorepo(tmp.path(), 5).unwrap();
        assert!(!result.is_empty());
        assert!(result.iter().any(|p| p.relative_path.contains("ci.yml")));
    }

    #[test]
    fn test_infer_package_name_from_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let name = infer_package_name(tmp.path(), tmp.path());
        assert_eq!(name, "(root)");
    }
}
