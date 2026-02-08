# PipelineX Integrations

Guide for integrating PipelineX into your CI/CD workflows.

## 📦 Installation

### Binary Release (Recommended)
```bash
# Download from GitHub Releases
curl -L https://github.com/mackeh/PipelineX/releases/latest/download/pipelinex-linux -o pipelinex
chmod +x pipelinex
sudo mv pipelinex /usr/local/bin/
```

### From Source
```bash
cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli
```

### Docker
```bash
docker run --rm -v $(pwd):/workspace mackeh/pipelinex analyze /workspace/.github/workflows/ci.yml
```

---

## 🔧 GitHub Actions Integration

### Option 1: SARIF Upload (Code Scanning)

Add to `.github/workflows/pipelinex.yml`:

```yaml
name: Pipeline Analysis

on:
  pull_request:
    paths:
      - '.github/workflows/**'
  schedule:
    - cron: '0 0 * * 0'

jobs:
  analyze:
    runs-on: ubuntu-latest
    permissions:
      security-events: write
    steps:
      - uses: actions/checkout@v4

      - name: Install PipelineX
        run: cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli

      - name: Analyze Pipeline
        run: pipelinex analyze .github/workflows/ci.yml --format sarif > results.sarif

      - name: Upload to Code Scanning
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: results.sarif
```

**Result:** Issues appear in GitHub's "Security" tab and as PR annotations.

### Option 2: PR Comments

```yaml
name: PR Pipeline Review

on:
  pull_request:
    paths:
      - '.github/workflows/**'

permissions:
  pull-requests: write

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install PipelineX
        run: cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli

      - name: Analyze
        run: |
          pipelinex analyze .github/workflows/ci.yml > analysis.txt
          {
            echo "## 🚀 PipelineX Analysis"
            echo "\`\`\`"
            cat analysis.txt
            echo "\`\`\`"
          } > comment.md

      - uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const comment = fs.readFileSync('comment.md', 'utf8');
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: comment
            });
```

### Option 3: Fail on Critical Issues

```yaml
- name: Check Pipeline Health
  run: |
    pipelinex analyze .github/workflows/ci.yml --format json | \
      jq -e '.findings | map(select(.severity == "Critical")) | length == 0' || \
      (echo "❌ Critical pipeline issues found!" && exit 1)
```

---

## 🦊 GitLab CI Integration

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
  only:
    changes:
      - .gitlab-ci.yml
```

**Custom code quality format:**

```bash
pipelinex analyze .gitlab-ci.yml --format json | jq -r '
  .findings[] | {
    description: .title,
    severity: (.severity | ascii_downcase),
    location: {
      path: ".gitlab-ci.yml",
      lines: {begin: 1}
    }
  }' > gl-code-quality-report.json
```

---

## 🔵 Azure Pipelines Integration

Add to `azure-pipelines.yml`:

```yaml
trigger:
  paths:
    include:
      - azure-pipelines.yml

jobs:
- job: PipelineXAnalysis
  pool:
    vmImage: 'ubuntu-latest'
  steps:
  - script: |
      cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli
      pipelinex analyze azure-pipelines.yml > analysis.txt
    displayName: 'Analyze Pipeline'

  - script: |
      if pipelinex analyze azure-pipelines.yml --format json | jq -e '.findings[] | select(.severity == "Critical")'; then
        echo "##vso[task.logissue type=error]Critical pipeline issues detected"
        exit 1
      fi
    displayName: 'Check for Critical Issues'
```

---

## 🐳 Docker Compose Integration

`docker-compose.yml`:

```yaml
services:
  pipelinex:
    build:
      context: .
      dockerfile: Dockerfile.pipelinex
    volumes:
      - .github/workflows:/pipelines:ro
    command: analyze /pipelines/ci.yml --format html
```

`Dockerfile.pipelinex`:

```dockerfile
FROM rust:1.75-slim
RUN cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli
ENTRYPOINT ["pipelinex"]
```

---

## 🎨 VS Code Integration

### Task Runner

Add to `.vscode/tasks.json`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "PipelineX: Analyze Pipeline",
      "type": "shell",
      "command": "pipelinex analyze .github/workflows/ci.yml",
      "presentation": {
        "reveal": "always",
        "panel": "new"
      },
      "problemMatcher": []
    },
    {
      "label": "PipelineX: Show Optimizations",
      "type": "shell",
      "command": "pipelinex diff .github/workflows/ci.yml",
      "presentation": {
        "reveal": "always",
        "panel": "new"
      }
    },
    {
      "label": "PipelineX: Generate HTML Report",
      "type": "shell",
      "command": "pipelinex analyze .github/workflows/ci.yml --format html > pipelinex-report.html && open pipelinex-report.html",
      "presentation": {
        "reveal": "silent"
      }
    }
  ]
}
```

**Usage:** `Cmd/Ctrl + Shift + P` → "Tasks: Run Task" → Select PipelineX task

### Snippets

Add to `.vscode/pipelinex.code-snippets`:

```json
{
  "PipelineX Analyze": {
    "prefix": "px-analyze",
    "body": [
      "pipelinex analyze ${1:.github/workflows/ci.yml}"
    ]
  },
  "PipelineX Optimize": {
    "prefix": "px-optimize",
    "body": [
      "pipelinex optimize ${1:.github/workflows/ci.yml} -o ${2:ci-optimized.yml}"
    ]
  }
}
```

---

## 📊 Pre-commit Hook

`.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: pipelinex
        name: PipelineX Pipeline Analysis
        entry: pipelinex analyze
        language: system
        files: \.(github/workflows|gitlab-ci|jenkinsfile)\.ya?ml$
        pass_filenames: true
```

Or manual Git hook (`.git/hooks/pre-commit`):

```bash
#!/bin/bash
if git diff --cached --name-only | grep -q '\.github/workflows/'; then
  echo "🔍 Running PipelineX analysis..."
  pipelinex analyze .github/workflows/ci.yml --format json | \
    jq -e '.findings | map(select(.severity == "Critical")) | length == 0' || {
      echo "❌ Critical pipeline issues detected! Run 'pipelinex diff' to see optimizations."
      exit 1
    }
fi
```

---

## 🤖 CI Badge

Add to README.md:

```markdown
[![Pipeline Health](https://img.shields.io/badge/pipeline-optimized-success)](https://github.com/your-org/your-repo/actions/workflows/pipelinex.yml)
```

---

## 📈 Monitoring & Metrics

### Weekly Pipeline Health Report

```yaml
name: Weekly Pipeline Report

on:
  schedule:
    - cron: '0 9 * * 1'  # Monday 9am

jobs:
  report:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Generate Report
        run: |
          pipelinex analyze .github/workflows/*.yml --format json > report.json
          
          cat > weekly-report.md << EOF
          # 📊 Weekly Pipeline Health Report
          
          \`\`\`json
          $(cat report.json | jq '{
            total_pipelines: (.findings | group_by(.file) | length),
            critical_issues: (.findings | map(select(.severity == "Critical")) | length),
            potential_savings: .potential_improvement_pct
          }')
          \`\`\`
          EOF

      - name: Post to Slack
        env:
          SLACK_WEBHOOK: ${{ secrets.SLACK_WEBHOOK }}
        run: |
          curl -X POST $SLACK_WEBHOOK \
            -H 'Content-Type: application/json' \
            -d "{\"text\": \"$(cat weekly-report.md)\"}"
```

---

## 🔐 Security Best Practices

1. **Pin PipelineX version:**
   ```bash
   PIPELINEX_VERSION=0.1.0
   cargo install --git https://github.com/mackeh/PipelineX --tag v${PIPELINEX_VERSION} pipelinex-cli
   ```

2. **Verify checksums:**
   ```bash
   curl -L https://github.com/mackeh/PipelineX/releases/download/v0.1.0/checksums.txt
   ```

3. **Use read-only volumes:**
   ```bash
   docker run --rm -v $(pwd):/workspace:ro mackeh/pipelinex analyze /workspace/.github/workflows/ci.yml
   ```

---

## 💡 Tips & Tricks

### Analyze All Workflows

```bash
find .github/workflows -name "*.yml" -exec pipelinex analyze {} \;
```

### Compare Before/After

```bash
# Before optimization
pipelinex analyze ci.yml --format json | jq '.total_estimated_duration_secs'

# After optimization
pipelinex analyze ci-optimized.yml --format json | jq '.optimized_duration_secs'
```

### Generate Executive Summary

```bash
pipelinex cost .github/workflows/ --format json | jq '{
  monthly_savings: .monthly_compute_cost,
  time_savings: .potential_time_savings_hours,
  developer_cost_savings: .monthly_developer_cost_savings
}'
```

---

## 🆘 Troubleshooting

**Issue:** PipelineX not finding workflows

```bash
# Check file detection
pipelinex analyze . -v
```

**Issue:** SARIF upload fails

Ensure you have `security-events: write` permission in your workflow.

**Issue:** Analysis too slow

```bash
# Analyze specific files only
pipelinex analyze .github/workflows/ci.yml
```

---

## 📚 More Resources

- [CONTRIBUTING.md](../CONTRIBUTING.md) - How to contribute
- [README.md](../README.md) - Main documentation
- [GitHub Discussions](https://github.com/mackeh/PipelineX/discussions) - Community support
