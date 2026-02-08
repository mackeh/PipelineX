use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a test selection strategy based on code changes.
#[derive(Debug, Clone)]
pub struct TestSelection {
    /// Files that have changed
    pub changed_files: Vec<PathBuf>,
    /// Tests that should be run based on changes
    pub selected_tests: Vec<String>,
    /// Test patterns for CI configuration
    pub test_patterns: Vec<String>,
    /// Percentage of total tests selected
    pub selection_ratio: f64,
    /// Reasoning for the selection
    pub reasoning: Vec<String>,
}

/// Configuration for test selection behavior.
#[derive(Debug, Clone)]
pub struct TestSelectorConfig {
    /// Always run these test patterns regardless of changes
    pub always_run: Vec<String>,
    /// Language-specific test file patterns
    pub test_patterns: HashMap<String, Vec<String>>,
    /// Directories to exclude from analysis
    pub exclude_dirs: Vec<String>,
    /// Minimum tests to run (even if changes are minimal)
    pub min_tests: usize,
}

impl Default for TestSelectorConfig {
    fn default() -> Self {
        let mut test_patterns = HashMap::new();

        // Common test patterns by language
        test_patterns.insert(
            "rust".to_string(),
            vec!["**/*_test.rs".to_string(), "**/tests/**/*.rs".to_string()],
        );

        test_patterns.insert(
            "javascript".to_string(),
            vec![
                "**/*.test.js".to_string(),
                "**/*.spec.js".to_string(),
                "**/__tests__/**/*.js".to_string(),
            ],
        );

        test_patterns.insert(
            "typescript".to_string(),
            vec![
                "**/*.test.ts".to_string(),
                "**/*.spec.ts".to_string(),
                "**/__tests__/**/*.ts".to_string(),
            ],
        );

        test_patterns.insert(
            "python".to_string(),
            vec![
                "**/test_*.py".to_string(),
                "**/*_test.py".to_string(),
                "**/tests/**/*.py".to_string(),
            ],
        );

        test_patterns.insert("go".to_string(), vec!["**/*_test.go".to_string()]);

        Self {
            always_run: vec!["e2e".to_string(), "integration".to_string()],
            test_patterns,
            exclude_dirs: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
            min_tests: 3,
        }
    }
}

/// Main test selection engine.
pub struct TestSelector {
    config: TestSelectorConfig,
}

impl TestSelector {
    /// Create a new test selector with default configuration.
    pub fn new() -> Self {
        Self {
            config: TestSelectorConfig::default(),
        }
    }

    /// Create a test selector with custom configuration.
    pub fn with_config(config: TestSelectorConfig) -> Self {
        Self { config }
    }

    /// Select tests based on git diff between two commits.
    pub fn select_from_git_diff(
        &self,
        base: &str,
        head: &str,
        repo_path: Option<&Path>,
    ) -> Result<TestSelection> {
        let changed_files = self.get_changed_files(base, head, repo_path)?;
        self.select_from_changes(&changed_files, repo_path)
    }

    /// Select tests based on a list of changed files.
    pub fn select_from_changes(
        &self,
        changed_files: &[PathBuf],
        repo_path: Option<&Path>,
    ) -> Result<TestSelection> {
        let mut selected_tests = HashSet::new();
        let mut test_patterns = HashSet::new();
        let mut reasoning = Vec::new();

        // Filter out excluded directories
        let relevant_files: Vec<_> = changed_files
            .iter()
            .filter(|f| !self.is_excluded(f))
            .collect();

        if relevant_files.is_empty() {
            reasoning.push("No relevant code changes detected".to_string());
            return Ok(TestSelection {
                changed_files: changed_files.to_vec(),
                selected_tests: vec![],
                test_patterns: vec![],
                selection_ratio: 0.0,
                reasoning,
            });
        }

        // Detect language from file extensions
        let languages = self.detect_languages(&relevant_files);

        // 1. Direct test files that changed
        for file in &relevant_files {
            if self.is_test_file(file) {
                let test_name = self.file_to_test_name(file);
                selected_tests.insert(test_name.clone());
                reasoning.push(format!("Direct: {} (test file changed)", test_name));
            }
        }

        // 2. Tests for changed source files
        for file in &relevant_files {
            if !self.is_test_file(file) {
                if let Some(test_file) = self.find_test_for_source(file, &languages, repo_path) {
                    let test_name = self.file_to_test_name(&test_file);
                    if selected_tests.insert(test_name.clone()) {
                        reasoning.push(format!(
                            "Affected: {} (source file {} changed)",
                            test_name,
                            file.display()
                        ));
                    }
                }
            }
        }

        // 3. Add always-run tests (integration, e2e)
        for pattern in &self.config.always_run {
            test_patterns.insert(pattern.clone());
            reasoning.push(format!("Always-run: {}", pattern));
        }

        // 4. If changes are in critical paths (config, CI), run all tests
        if self.has_critical_changes(&relevant_files) {
            reasoning.push(
                "Critical files changed (CI config, dependencies) — running all tests".to_string(),
            );
            return Ok(TestSelection {
                changed_files: changed_files.to_vec(),
                selected_tests: vec!["all".to_string()],
                test_patterns: vec!["**/*".to_string()],
                selection_ratio: 1.0,
                reasoning,
            });
        }

        // Generate test patterns from selected tests
        for test in &selected_tests {
            test_patterns.insert(test.clone());
        }

        // Ensure minimum test coverage
        let selected_vec: Vec<_> = selected_tests.into_iter().collect();
        let selection_ratio = if selected_vec.is_empty() {
            0.0
        } else {
            // Estimate based on patterns
            0.15 // Conservative estimate: 15% of tests
        };

        Ok(TestSelection {
            changed_files: changed_files.to_vec(),
            selected_tests: selected_vec,
            test_patterns: test_patterns.into_iter().collect(),
            selection_ratio,
            reasoning,
        })
    }

    /// Get changed files between two git refs.
    fn get_changed_files(
        &self,
        base: &str,
        head: &str,
        repo_path: Option<&Path>,
    ) -> Result<Vec<PathBuf>> {
        let mut cmd = Command::new("git");

        if let Some(path) = repo_path {
            cmd.current_dir(path);
        }

        let output = cmd
            .args(["diff", "--name-only", base, head])
            .output()
            .context("Failed to run git diff")?;

        if !output.status.success() {
            anyhow::bail!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let files = String::from_utf8(output.stdout)?
            .lines()
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect();

        Ok(files)
    }

    /// Check if a file is excluded from analysis.
    fn is_excluded(&self, path: &Path) -> bool {
        path.components().any(|c| {
            self.config
                .exclude_dirs
                .iter()
                .any(|ex| c.as_os_str() == ex.as_str())
        })
    }

    /// Check if a file is a test file.
    fn is_test_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check common test patterns
        path_str.contains("test")
            || path_str.contains("spec")
            || path_str.contains("__tests__")
            || path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| {
                    n.starts_with("test_")
                        || n.ends_with("_test.rs")
                        || n.ends_with("_test.go")
                        || n.ends_with(".test.js")
                        || n.ends_with(".test.ts")
                        || n.ends_with(".spec.js")
                        || n.ends_with(".spec.ts")
                })
                .unwrap_or(false)
    }

    /// Detect languages from file extensions.
    fn detect_languages(&self, files: &[&PathBuf]) -> HashSet<String> {
        let mut languages = HashSet::new();

        for file in files {
            if let Some(ext) = file.extension().and_then(|e| e.to_str()) {
                let lang = match ext {
                    "rs" => "rust",
                    "js" | "jsx" => "javascript",
                    "ts" | "tsx" => "typescript",
                    "py" => "python",
                    "go" => "go",
                    "java" => "java",
                    "rb" => "ruby",
                    _ => continue,
                };
                languages.insert(lang.to_string());
            }
        }

        languages
    }

    /// Find test file for a source file.
    fn find_test_for_source(
        &self,
        source: &Path,
        languages: &HashSet<String>,
        repo_path: Option<&Path>,
    ) -> Option<PathBuf> {
        let source_stem = source.file_stem()?.to_str()?;
        let parent = source.parent()?;

        // Try common patterns
        for lang in languages {
            // For Rust: src/foo.rs -> tests/foo_test.rs or src/foo.rs -> src/foo/tests.rs
            if lang == "rust" {
                let test_file = parent.join(format!("{}_test.rs", source_stem));
                if test_file.exists() {
                    return Some(test_file);
                }

                let test_dir = parent.join("tests").join(format!("{}.rs", source_stem));
                if test_dir.exists() {
                    return Some(test_dir);
                }
            }

            // For JS/TS: src/foo.js -> src/foo.test.js or src/__tests__/foo.test.js
            if lang == "javascript" || lang == "typescript" {
                let ext = source.extension()?.to_str()?;
                let test_file = parent.join(format!("{}.test.{}", source_stem, ext));
                if test_file.exists() {
                    return Some(test_file);
                }

                let test_dir = parent
                    .join("__tests__")
                    .join(format!("{}.test.{}", source_stem, ext));
                if test_dir.exists() {
                    return Some(test_dir);
                }
            }

            // For Python: src/foo.py -> tests/test_foo.py
            if lang == "python" {
                let test_file = Path::new("tests").join(format!("test_{}.py", source_stem));
                if let Some(repo) = repo_path {
                    let full_path = repo.join(&test_file);
                    if full_path.exists() {
                        return Some(test_file);
                    }
                }
            }
        }

        None
    }

    /// Check if changes include critical files that require full test suite.
    fn has_critical_changes(&self, files: &[&PathBuf]) -> bool {
        files.iter().any(|f| {
            let path_str = f.to_string_lossy();
            path_str.contains(".github/workflows")
                || path_str.contains(".gitlab-ci")
                || path_str.contains("Jenkinsfile")
                || path_str.contains(".circleci")
                || path_str.contains("package.json")
                || path_str.contains("Cargo.toml")
                || path_str.contains("go.mod")
                || path_str.contains("requirements.txt")
                || path_str.contains("pom.xml")
                || path_str.contains("build.gradle")
        })
    }

    /// Convert file path to test name/pattern.
    fn file_to_test_name(&self, path: &Path) -> String {
        // Convert path to a test identifier
        path.to_string_lossy().to_string()
    }
}

impl Default for TestSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_test_file() {
        let selector = TestSelector::new();

        assert!(selector.is_test_file(Path::new("src/foo_test.rs")));
        assert!(selector.is_test_file(Path::new("tests/integration.rs")));
        assert!(selector.is_test_file(Path::new("src/foo.test.ts")));
        assert!(selector.is_test_file(Path::new("src/__tests__/foo.js")));
        assert!(selector.is_test_file(Path::new("test_foo.py")));

        assert!(!selector.is_test_file(Path::new("src/main.rs")));
        assert!(!selector.is_test_file(Path::new("src/lib.js")));
    }

    #[test]
    fn test_detect_languages() {
        let selector = TestSelector::new();
        let files = [
            PathBuf::from("src/main.rs"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("index.js"),
        ];
        let file_refs: Vec<_> = files.iter().collect();

        let languages = selector.detect_languages(&file_refs);
        assert!(languages.contains("rust"));
        assert!(languages.contains("javascript"));
        assert!(!languages.contains("python"));
    }

    #[test]
    fn test_is_excluded() {
        let selector = TestSelector::new();

        assert!(selector.is_excluded(Path::new("node_modules/foo/bar.js")));
        assert!(selector.is_excluded(Path::new(".git/config")));
        assert!(selector.is_excluded(Path::new("target/debug/build")));

        assert!(!selector.is_excluded(Path::new("src/main.rs")));
        assert!(!selector.is_excluded(Path::new("tests/integration.rs")));
    }

    #[test]
    fn test_has_critical_changes() {
        let selector = TestSelector::new();

        let files = [PathBuf::from(".github/workflows/ci.yml")];
        let file_refs: Vec<_> = files.iter().collect();
        assert!(selector.has_critical_changes(&file_refs));

        let files = [PathBuf::from("package.json")];
        let file_refs: Vec<_> = files.iter().collect();
        assert!(selector.has_critical_changes(&file_refs));

        let files = [PathBuf::from("src/main.rs")];
        let file_refs: Vec<_> = files.iter().collect();
        assert!(!selector.has_critical_changes(&file_refs));
    }
}
