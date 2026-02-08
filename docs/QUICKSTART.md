# PipelineX Quick Start Guide

Get up and running with PipelineX in under 5 minutes.

---

## 📦 Installation

### Option 1: One-Line Install (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/mackeh/PipelineX/main/install.sh | bash
```

### Option 2: Using Cargo

```bash
cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli
```

### Option 3: Docker

```bash
docker pull ghcr.io/mackeh/pipelinex:latest
# Or from Docker Hub
docker pull mackeh/pipelinex:latest
```

### Option 4: From Source

```bash
git clone https://github.com/mackeh/PipelineX
cd PipelineX
make install
```

---

## 🚀 First Analysis

Analyze your CI pipeline in seconds:

```bash
# GitHub Actions
pipelinex analyze .github/workflows/ci.yml

# GitLab CI
pipelinex analyze .gitlab-ci.yml

# Jenkins
pipelinex analyze Jenkinsfile

# CircleCI
pipelinex analyze .circleci/config.yml

# Bitbucket Pipelines
pipelinex analyze bitbucket-pipelines.yml

# Azure Pipelines
pipelinex analyze azure-pipelines.yml

# AWS CodePipeline
pipelinex analyze codepipeline.json

# Buildkite
pipelinex analyze .buildkite/pipeline.yml
```

**Example output:**
```
📊 Pipeline Analysis: .github/workflows/ci.yml
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

⏱️  Estimated Duration: 31m 45s
🎯 Potential Improvement: 80% (6m 12s)
💰 Monthly Cost Savings: $8,420

🔍 Issues Found (12):

🔴 CRITICAL (3)
  • Sequential tests (test-unit, test-integration, test-e2e)
    → Run in parallel: save 22m 15s

  • No test caching configured
    → Add Swatinem/rust-cache@v2: save 4m 30s

  • Duplicate checkout step in 5 jobs
    → Use job artifacts: save 2m 10s

🟠 HIGH (5)
  • No dependency caching
  • Docker layer caching disabled
  • Full checkout on every job
  • Matrix builds not parallelized
  • No build artifact reuse

🟡 MEDIUM (4)
  • Suboptimal runner selection
  • Missing concurrency groups
  • No fail-fast strategy
  • Verbose logging overhead
```

---

## ⚡ Quick Optimizations

### Generate Optimized Pipeline

```bash
pipelinex optimize .github/workflows/ci.yml -o ci-optimized.yml
```

This creates a new file with all recommended optimizations applied.

### View Side-by-Side Diff

```bash
pipelinex diff .github/workflows/ci.yml
```

Shows exactly what changed and why.

---

## 💰 Cost Analysis

Calculate potential savings:

```bash
# Single pipeline
pipelinex cost .github/workflows/ci.yml

# All workflows in directory
pipelinex cost .github/workflows/
```

**Output:**
```
💰 Cost Analysis: .github/workflows/
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Current Monthly Cost:      $12,450
Optimized Monthly Cost:    $ 4,130
Monthly Savings:           $ 8,320 (67%)

Annual Savings:            $99,840

⏱️  Time Savings:
  • Per run: 25m 33s → 6m 12s (76% faster)
  • Monthly developer time saved: 142 hours
  • Annual developer cost savings: $273,460
```

---

## 📊 Visual Reports

### HTML Report (Interactive)

```bash
pipelinex analyze .github/workflows/ci.yml --format html > report.html
open report.html  # macOS
xdg-open report.html  # Linux
```

Includes:
- Interactive pipeline graph
- Clickable issue cards
- Optimization recommendations
- Cost breakdown charts

### SARIF (GitHub Code Scanning)

```bash
pipelinex analyze .github/workflows/ci.yml --format sarif > results.sarif
```

Upload to GitHub Security tab for inline PR annotations.

---

## 🔧 IDE Integration

### VS Code

1. Open PipelineX project folder
2. Tasks are pre-configured in `.vscode/tasks.json`
3. Run: `Cmd/Ctrl + Shift + P` → "Tasks: Run Task"

**Available tasks:**
- PipelineX: Analyze CI Pipeline
- PipelineX: Show Optimizations
- PipelineX: Cost Analysis
- PipelineX: Generate HTML Report
- PipelineX: Run Simulation

### Command Line with Make

```bash
# Show all available commands
make help

# Quick analysis
make analyze

# Generate HTML report
make analyze-html

# Full CI locally
make ci-local

# Build and test
make all
```

---

## 🐳 Docker Usage

### Basic Usage

```bash
docker run --rm -v $(pwd):/workspace:ro \
  mackeh/pipelinex analyze /workspace/.github/workflows/ci.yml
```

### Docker Compose

```bash
# Run analysis
docker-compose run pipelinex analyze .github/workflows/ci.yml

# Generate optimization
docker-compose run pipelinex optimize .github/workflows/ci.yml -o optimized.yml

# Cost analysis
docker-compose run pipelinex-cost
```

---

## 🔗 CI/CD Integration

### GitHub Actions

Add `.github/workflows/pipelinex.yml`:

```yaml
name: Pipeline Analysis

on:
  pull_request:
    paths: ['.github/workflows/**']

permissions:
  security-events: write

jobs:
  analyze:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install PipelineX
        run: cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli
      - name: Analyze
        run: pipelinex analyze .github/workflows/ci.yml --format sarif > results.sarif
      - name: Upload to Code Scanning
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: results.sarif
```

### GitLab CI

Add to `.gitlab-ci.yml`:

```yaml
pipelinex:
  stage: test
  image: rust:latest
  script:
    - cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli
    - pipelinex analyze .gitlab-ci.yml --format json > gl-code-quality-report.json
  artifacts:
    reports:
      codequality: gl-code-quality-report.json
```

### Pre-commit Hook

```bash
# Install pre-commit
pip install pre-commit

# Copy PipelineX config
cp .pre-commit-config.yaml .pre-commit-config.yaml

# Install hooks
pre-commit install

# Test manually
pre-commit run --all-files
```

---

## 🎓 Advanced Features

### Monte Carlo Simulation

Predict pipeline behavior with uncertainty:

```bash
pipelinex simulate .github/workflows/ci.yml
```

Shows timing distributions, failure probabilities, and confidence intervals.

### Pipeline Graph Visualization

```bash
pipelinex graph .github/workflows/ci.yml
```

Generates DOT format graph showing job dependencies and critical path.

### Smart Test Selection

Select only tests affected by code changes:

```bash
pipelinex select-tests --base main --head feature-branch
```

Saves 85%+ test execution time.

### Flaky Test Detection

Analyze JUnit XML results:

```bash
pipelinex flaky test-results/*.xml
```

Identifies unreliable tests with statistical confidence.

---

## 📚 Common Workflows

### Weekly Pipeline Health Check

```bash
#!/bin/bash
# weekly-check.sh

pipelinex analyze .github/workflows/*.yml --format json | \
  jq '{
    pipelines: (.findings | group_by(.file) | length),
    critical: (.findings | map(select(.severity == "Critical")) | length),
    savings: .monthly_compute_savings
  }'
```

Add to cron: `0 9 * * 1` (Monday 9am)

### PR Comment Bot

```bash
#!/bin/bash
# pr-comment.sh

ANALYSIS=$(pipelinex analyze .github/workflows/ci.yml)
DIFF=$(pipelinex diff .github/workflows/ci.yml)

gh pr comment $PR_NUMBER --body "
## 🚀 Pipeline Analysis

\`\`\`
$ANALYSIS
\`\`\`

## Optimizations

\`\`\`
$DIFF
\`\`\`
"
```

### Continuous Monitoring

```bash
# Watch for changes and re-analyze
watch -n 3600 'pipelinex analyze .github/workflows/ci.yml --format json > /tmp/analysis.json'
```

---

## 🆘 Troubleshooting

### PipelineX not found

```bash
# Check installation
which pipelinex

# Add to PATH if needed
export PATH="$HOME/.cargo/bin:$PATH"
```

### Permission denied

```bash
# Linux: run with sudo or install to user directory
cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli --root ~/.local

# Add to PATH
export PATH="$HOME/.local/bin:$PATH"
```

### Analysis too slow

```bash
# Analyze specific files instead of directory
pipelinex analyze .github/workflows/ci.yml

# Use parallel analysis (future feature)
ls .github/workflows/*.yml | xargs -P 4 -I {} pipelinex analyze {}
```

---

## 🔑 Key Commands Reference

| Command | Description |
|---------|-------------|
| `pipelinex analyze <file>` | Analyze pipeline and show issues |
| `pipelinex optimize <file> -o <output>` | Generate optimized pipeline |
| `pipelinex diff <file>` | Show before/after comparison |
| `pipelinex cost <directory>` | Calculate cost savings |
| `pipelinex simulate <file>` | Run Monte Carlo simulation |
| `pipelinex graph <file>` | Visualize pipeline DAG |
| `pipelinex --help` | Show all commands |

---

## 🎯 Next Steps

1. **Optimize your pipeline**: Apply suggested improvements
2. **Integrate with CI**: Add PipelineX to your workflow
3. **Monitor regularly**: Set up weekly analysis
4. **Share results**: Show cost savings to leadership
5. **Contribute**: Help improve PipelineX

---

## 📖 Learn More

- [Full Documentation](../README.md)
- [Integration Guide](INTEGRATIONS.md)
- [Contributing Guide](../CONTRIBUTING.md)
- [GitHub Discussions](https://github.com/mackeh/PipelineX/discussions)

---

**Need help?** Open an issue or start a discussion on GitHub!
