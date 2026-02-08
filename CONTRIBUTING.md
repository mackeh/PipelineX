# Contributing to PipelineX

Thank you for your interest in contributing to PipelineX! We welcome contributions of all kinds: bug fixes, new features, documentation improvements, and more.

## 🚀 Quick Start

```bash
# Clone the repository
git clone https://github.com/mackeh/PipelineX.git
cd PipelineX

# Build the project
cargo build

# Run tests
cargo test

# Run the CLI locally
cargo run -- analyze tests/fixtures/github-actions/unoptimized-fullstack.yml
```

## 📁 Project Structure

```
PipelineX/
├── crates/
│   ├── pipelinex-core/       # Core analysis engine (library)
│   │   ├── src/
│   │   │   ├── parser/       # CI platform parsers
│   │   │   ├── analyzer/     # Bottleneck detection
│   │   │   ├── optimizer/    # Config generation
│   │   │   ├── simulator/    # Monte Carlo simulation
│   │   │   ├── graph/        # DAG visualization
│   │   │   ├── cost/         # Cost estimation
│   │   │   ├── test_selector.rs    # Smart test selection
│   │   │   └── flaky_detector.rs   # Flaky test detection
│   │   └── tests/            # Integration tests
│   └── pipelinex-cli/        # CLI interface (binary)
│       └── src/
│           ├── main.rs       # Command handlers
│           └── display.rs    # Terminal output formatting
├── tests/fixtures/           # Test data for all CI platforms
└── docs/                     # Documentation
```

## 🎯 Ways to Contribute

### 1. **Add a New CI Platform Parser**

Want to add support for Azure Pipelines, AWS CodePipeline, or another CI platform? Here's how:

**Template:** `crates/pipelinex-core/src/parser/YOUR_PLATFORM.rs`

```rust
use crate::parser::dag::{PipelineDag, JobNode, StepInfo, CacheConfig};
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::path::Path;

pub struct YourPlatformParser;

impl YourPlatformParser {
    pub fn parse_file(path: &Path) -> Result<PipelineDag> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content, path.display().to_string())
    }

    pub fn parse(content: &str, source: String) -> Result<PipelineDag> {
        let yaml: Value = serde_yaml::from_str(content)?;

        let mut dag = PipelineDag::new(
            "Your Platform Pipeline".to_string(),
            source,
            "your-platform".to_string(),
        );

        // Parse jobs from YAML
        // Add jobs to DAG
        // Set up dependencies

        Ok(dag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pipeline() {
        let config = r#"
# Your platform's YAML here
"#;
        let dag = YourPlatformParser::parse(config, "test.yml".to_string()).unwrap();
        assert_eq!(dag.provider, "your-platform");
    }
}
```

**Steps:**
1. Create `src/parser/your_platform.rs`
2. Add module to `src/parser/mod.rs`: `pub mod your_platform;`
3. Export in `src/lib.rs`: `pub use parser::your_platform::YourPlatformParser;`
4. Add detection in `crates/pipelinex-cli/src/main.rs` `parse_pipeline()` function
5. Create test fixture in `tests/fixtures/your-platform/`
6. Add integration test in `crates/pipelinex-core/tests/integration_tests.rs`
7. Update README.md "Supported CI Platforms" table

**See examples:**
- Simple: `src/parser/circleci.rs`
- Complex: `src/parser/gitlab.rs`

### 2. **Add a New Analyzer**

Detect a new type of bottleneck or antipattern:

**Template:** `crates/pipelinex-core/src/analyzer/YOUR_DETECTOR.rs`

```rust
use crate::analyzer::report::{Finding, Severity, FindingCategory};
use crate::parser::dag::PipelineDag;

pub fn detect(dag: &PipelineDag) -> Vec<Finding> {
    let mut findings = Vec::new();

    for node in dag.graph.node_weights() {
        // Your detection logic here
        if /* condition */ {
            findings.push(Finding {
                title: "Your finding title".to_string(),
                message: "Detailed explanation".to_string(),
                severity: Severity::High,
                category: FindingCategory::YourCategory,
                job: Some(node.name.clone()),
                estimated_savings_secs: Some(180.0),
                confidence: 85,
                auto_fixable: true,
                fix_suggestion: Some("How to fix this".to_string()),
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_issue() {
        // Create test DAG
        // Run detector
        // Assert findings
    }
}
```

**Steps:**
1. Create your detector in `src/analyzer/`
2. Add module to `src/analyzer/mod.rs`
3. Call it in `src/analyzer/mod.rs` `analyze()` function
4. Add tests
5. Update documentation

### 3. **Add an Optimizer**

Generate optimized configs or fix specific issues:

**Template:** `crates/pipelinex-core/src/optimizer/YOUR_OPTIMIZER.rs`

```rust
use crate::analyzer::report::AnalysisReport;
use anyhow::Result;

pub fn optimize(original_content: &str, report: &AnalysisReport) -> Result<String> {
    let mut optimized = original_content.to_string();

    // Apply optimizations based on report findings
    for finding in &report.findings {
        match finding.category {
            // Your optimization logic
            _ => {}
        }
    }

    Ok(optimized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization() {
        // Test that optimization works correctly
    }
}
```

### 4. **Improve Documentation**

- Add examples to README.md
- Write blog posts or tutorials
- Create video walkthroughs
- Improve code comments
- Add more test fixtures

### 5. **Report Bugs**

Found a bug? Please open an issue with:
- Description of the problem
- Steps to reproduce
- Expected vs actual behavior
- Your environment (OS, Rust version)
- Sample pipeline YAML (if applicable)

## 🧪 Testing Guidelines

### Unit Tests

Add tests in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_feature() {
        // Arrange
        // Act
        // Assert
    }
}
```

### Integration Tests

Add to `crates/pipelinex-core/tests/integration_tests.rs`:

```rust
#[test]
fn test_your_integration() {
    let path = your_fixture("test-file.yml");
    let dag = YourParser::parse_file(&path).unwrap();
    let report = analyzer::analyze(&dag);

    assert!(report.findings.len() > 0);
}
```

### Run Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_name

# With output
cargo test -- --nocapture

# Integration tests only
cargo test --test integration_tests
```

## 📝 Code Style

- Follow Rust conventions (use `cargo fmt`)
- Run `cargo clippy` and fix warnings
- Add comments for complex logic
- Keep functions small and focused
- Write descriptive variable names

## 🔄 Pull Request Process

1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feature/your-feature-name`
3. **Make** your changes
4. **Test** thoroughly: `cargo test`
5. **Commit** with descriptive messages:
   ```
   Add Azure Pipelines parser

   - Parse azure-pipelines.yml structure
   - Support stages, jobs, and dependencies
   - Add cache detection
   - Include 3 test fixtures
   ```
6. **Push** to your fork: `git push origin feature/your-feature-name`
7. **Open** a Pull Request with:
   - Clear description of changes
   - Why the change is needed
   - Any breaking changes
   - Screenshots (if UI changes)

## 💡 Feature Request Process

Have an idea? Open an issue with:
- **Problem:** What problem does this solve?
- **Solution:** How should it work?
- **Alternatives:** What other approaches did you consider?
- **Impact:** Who benefits from this feature?

## 🎨 Design Principles

1. **Offline-first:** PipelineX should work without internet
2. **Zero config:** No account or setup required
3. **Fast:** Analysis should be near-instant
4. **Accurate:** Detection should have high confidence
5. **Actionable:** Every finding should have a fix suggestion
6. **Multi-platform:** Support all major CI systems

## 📚 Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Cargo Guide](https://doc.rust-lang.org/cargo/)
- [GitHub Actions Docs](https://docs.github.com/en/actions)
- [GitLab CI Docs](https://docs.gitlab.com/ee/ci/)
- [CircleCI Docs](https://circleci.com/docs/)
- [Bitbucket Pipelines Docs](https://support.atlassian.com/bitbucket-cloud/docs/get-started-with-bitbucket-pipelines/)

## 🏆 Recognition

Contributors will be:
- Listed in release notes
- Added to README.md contributors section
- Credited in commit messages with `Co-Authored-By`

## 📬 Questions?

- Open a [GitHub Discussion](https://github.com/mackeh/PipelineX/discussions)
- Comment on relevant issues
- Check existing documentation

---

**Thank you for making PipelineX better!** 🚀
