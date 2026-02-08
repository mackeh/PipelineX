# Integration Examples

This directory contains ready-to-use integration scripts and configurations for PipelineX.

---

## 📁 Available Integrations

### 🔔 Notifications

#### Slack
**File:** `slack-notification.sh`

Send pipeline analysis to Slack channel:

```bash
# Set webhook URL
export SLACK_WEBHOOK_URL="https://hooks.slack.com/services/YOUR/WEBHOOK/URL"

# Run analysis and notify
./slack-notification.sh .github/workflows/ci.yml

# Or specify webhook directly
./slack-notification.sh .github/workflows/ci.yml https://hooks.slack.com/...
```

**Features:**
- Color-coded alerts (🔴 Critical, 🟠 High, ✅ Good)
- Key metrics summary
- Direct link to GitHub Actions
- Improvement percentage calculation

#### Discord (Create this)

```bash
#!/bin/bash
# discord-notification.sh
WEBHOOK_URL="$1"
ANALYSIS=$(pipelinex analyze .github/workflows/ci.yml --format json)

CRITICAL=$(echo "$ANALYSIS" | jq '[.findings[] | select(.severity == "Critical")] | length')
IMPROVEMENT=$(echo "$ANALYSIS" | jq -r '.potential_improvement_pct // "N/A"')

curl -H "Content-Type: application/json" \
  -d "{
    \"content\": \"🚀 **Pipeline Analysis**\",
    \"embeds\": [{
      \"title\": \"CI Pipeline Health Check\",
      \"color\": $( [ "$CRITICAL" -gt 0 ] && echo 15158332 || echo 3066993 ),
      \"fields\": [
        {\"name\": \"Critical Issues\", \"value\": \"$CRITICAL\", \"inline\": true},
        {\"name\": \"Improvement\", \"value\": \"$IMPROVEMENT%\", \"inline\": true}
      ]
    }]
  }" \
  "$WEBHOOK_URL"
```

#### Microsoft Teams (Create this)

```bash
#!/bin/bash
# teams-notification.sh
WEBHOOK_URL="$1"
ANALYSIS=$(pipelinex analyze .github/workflows/ci.yml)

curl -H "Content-Type: application/json" \
  -d "{
    \"@type\": \"MessageCard\",
    \"@context\": \"https://schema.org/extensions\",
    \"summary\": \"Pipeline Analysis\",
    \"themeColor\": \"0078D7\",
    \"title\": \"🚀 PipelineX Analysis\",
    \"text\": \"$(echo "$ANALYSIS" | head -20)\",
    \"potentialAction\": [{
      \"@type\": \"OpenUri\",
      \"name\": \"View on GitHub\",
      \"targets\": [{
        \"os\": \"default\",
        \"uri\": \"https://github.com/${GITHUB_REPOSITORY}/actions\"
      }]
    }]
  }" \
  "$WEBHOOK_URL"
```

---

### 📊 Monitoring

#### Prometheus Exporter (Create this)

```python
#!/usr/bin/env python3
# prometheus-exporter.py
import json
import subprocess
from prometheus_client import start_http_server, Gauge
import time

# Define metrics
pipeline_duration = Gauge('pipelinex_estimated_duration_seconds',
                          'Estimated pipeline duration', ['workflow'])
critical_issues = Gauge('pipelinex_critical_issues',
                        'Number of critical issues', ['workflow'])
improvement_pct = Gauge('pipelinex_improvement_percentage',
                        'Potential improvement percentage', ['workflow'])

def analyze_pipeline(workflow_file):
    result = subprocess.run(
        ['pipelinex', 'analyze', workflow_file, '--format', 'json'],
        capture_output=True, text=True
    )
    return json.loads(result.stdout)

def update_metrics():
    workflows = ['.github/workflows/ci.yml']
    for workflow in workflows:
        data = analyze_pipeline(workflow)

        pipeline_duration.labels(workflow=workflow).set(
            data.get('total_estimated_duration_secs', 0)
        )
        critical_count = len([f for f in data.get('findings', [])
                             if f.get('severity') == 'Critical'])
        critical_issues.labels(workflow=workflow).set(critical_count)
        improvement_pct.labels(workflow=workflow).set(
            data.get('potential_improvement_pct', 0)
        )

if __name__ == '__main__':
    start_http_server(8000)
    print("Prometheus exporter listening on :8000")

    while True:
        update_metrics()
        time.sleep(300)  # Update every 5 minutes
```

Run with: `python3 prometheus-exporter.py`

**Grafana Dashboard Config:**

```json
{
  "panels": [
    {
      "title": "Pipeline Duration",
      "targets": [{
        "expr": "pipelinex_estimated_duration_seconds"
      }]
    },
    {
      "title": "Critical Issues",
      "targets": [{
        "expr": "pipelinex_critical_issues"
      }]
    }
  ]
}
```

---

### 🔄 CI/CD Platforms

#### Jenkins Pipeline

```groovy
// Jenkinsfile.pipelinex
pipeline {
    agent any

    stages {
        stage('Analyze Pipeline') {
            steps {
                sh '''
                    cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli
                    pipelinex analyze Jenkinsfile --format json > analysis.json
                '''

                script {
                    def analysis = readJSON file: 'analysis.json'
                    def critical = analysis.findings.count { it.severity == 'Critical' }

                    if (critical > 0) {
                        error "Found ${critical} critical pipeline issues!"
                    }
                }

                archiveArtifacts artifacts: 'analysis.json'
            }
        }
    }

    post {
        always {
            publishHTML([
                reportName: 'PipelineX Report',
                reportDir: '.',
                reportFiles: 'analysis.html',
                keepAll: true
            ])
        }
    }
}
```

#### Travis CI

```yaml
# .travis.yml
language: rust
rust: stable

before_script:
  - cargo install --git https://github.com/mackeh/PipelineX pipelinex-cli

script:
  - pipelinex analyze .travis.yml

after_success:
  - pipelinex analyze .travis.yml --format json |
    jq -e '.findings | map(select(.severity == "Critical")) | length == 0'
```

---

### 🤖 Automation Scripts

#### Automated PR Comment (Create this)

```bash
#!/bin/bash
# auto-pr-comment.sh
# Runs in GitHub Actions to comment on PRs

set -e

PR_NUMBER="$1"
WORKFLOW_FILE="${2:-.github/workflows/ci.yml}"

# Generate analysis
pipelinex analyze "$WORKFLOW_FILE" > /tmp/analysis.txt
pipelinex diff "$WORKFLOW_FILE" > /tmp/diff.txt 2>&1 || echo "No optimizations" > /tmp/diff.txt

# Create comment body
cat > /tmp/comment.md <<EOF
## 🚀 PipelineX Analysis

### Current Analysis
\`\`\`
$(cat /tmp/analysis.txt)
\`\`\`

### Suggested Optimizations
\`\`\`diff
$(cat /tmp/diff.txt)
\`\`\`

---
📊 [Full report available in workflow artifacts](https://github.com/$GITHUB_REPOSITORY/actions/runs/$GITHUB_RUN_ID)
💡 Run \`make optimize\` locally to generate optimized pipeline
EOF

# Post comment using gh CLI
gh pr comment "$PR_NUMBER" --body-file /tmp/comment.md
```

Usage in GitHub Actions:

```yaml
- name: Comment on PR
  run: |
    ./examples/integrations/auto-pr-comment.sh ${{ github.event.pull_request.number }}
  env:
    GH_TOKEN: ${{ github.token }}
```

#### Nightly Pipeline Audit (Create this)

```bash
#!/bin/bash
# nightly-audit.sh
# Run via cron: 0 2 * * * /path/to/nightly-audit.sh

REPORT_DIR="/var/log/pipelinex"
DATE=$(date +%Y-%m-%d)
REPO_DIR="$HOME/projects/myapp"

cd "$REPO_DIR"

# Generate reports
pipelinex analyze .github/workflows/*.yml --format json > "$REPORT_DIR/analysis-$DATE.json"
pipelinex cost .github/workflows/ --format json > "$REPORT_DIR/cost-$DATE.json"

# Check for regressions
CRITICAL_COUNT=$(jq '[.findings[] | select(.severity == "Critical")] | length' \
  "$REPORT_DIR/analysis-$DATE.json")

if [ "$CRITICAL_COUNT" -gt 0 ]; then
    echo "⚠️ $CRITICAL_COUNT critical issues detected!" | \
      mail -s "Pipeline Health Alert" devops@example.com
fi

# Cleanup old reports (keep 30 days)
find "$REPORT_DIR" -name "*.json" -mtime +30 -delete
```

---

### 🐳 Container Orchestration

#### Kubernetes CronJob

```yaml
# kubernetes/pipelinex-cronjob.yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: pipelinex-audit
spec:
  schedule: "0 2 * * *"  # 2am daily
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: pipelinex
            image: ghcr.io/mackeh/pipelinex:latest
            command:
            - /bin/bash
            - -c
            - |
              cd /workspace
              pipelinex analyze .github/workflows/*.yml --format json > /reports/analysis.json
            volumeMounts:
            - name: workspace
              mountPath: /workspace
            - name: reports
              mountPath: /reports
          volumes:
          - name: workspace
            gitRepo:
              repository: https://github.com/yourorg/yourrepo
          - name: reports
            persistentVolumeClaim:
              claimName: pipelinex-reports
          restartPolicy: OnFailure
```

Deploy: `kubectl apply -f kubernetes/pipelinex-cronjob.yaml`

---

### 📈 Dashboards

#### Datadog Integration (Create this)

```python
#!/usr/bin/env python3
# datadog-integration.py
import json
import subprocess
from datadog import initialize, api

options = {
    'api_key': 'YOUR_DATADOG_API_KEY',
    'app_key': 'YOUR_DATADOG_APP_KEY'
}
initialize(**options)

def send_metrics():
    result = subprocess.run(
        ['pipelinex', 'analyze', '.github/workflows/ci.yml', '--format', 'json'],
        capture_output=True, text=True
    )
    data = json.loads(result.stdout)

    # Send metrics
    api.Metric.send(
        metric='pipelinex.duration',
        points=data.get('total_estimated_duration_secs', 0),
        tags=['env:production', 'workflow:ci']
    )

    critical_count = len([f for f in data.get('findings', [])
                         if f.get('severity') == 'Critical'])
    api.Metric.send(
        metric='pipelinex.critical_issues',
        points=critical_count,
        tags=['env:production', 'workflow:ci']
    )

if __name__ == '__main__':
    send_metrics()
```

---

### 🔐 Security Integration

#### Snyk Integration

```bash
#!/bin/bash
# snyk-integration.sh
# Integrate PipelineX with Snyk for combined security + pipeline analysis

pipelinex analyze .github/workflows/ci.yml --format sarif > pipelinex.sarif
snyk test --sarif-file-output=snyk.sarif

# Merge SARIF files
jq -s '.[0].runs[0].results += .[1].runs[0].results | .[0]' \
  pipelinex.sarif snyk.sarif > combined.sarif

# Upload to GitHub
gh api repos/:owner/:repo/code-scanning/sarifs \
  -f sarif=@combined.sarif \
  -f commit_sha="$GITHUB_SHA" \
  -f ref="$GITHUB_REF"
```

---

## 🛠️ Helper Utilities

### Batch Analysis

```bash
#!/bin/bash
# batch-analyze.sh
# Analyze all workflows in a directory

WORKFLOWS_DIR="${1:-.github/workflows}"
OUTPUT_DIR="${2:-./pipelinex-reports}"

mkdir -p "$OUTPUT_DIR"

for workflow in "$WORKFLOWS_DIR"/*.yml; do
    filename=$(basename "$workflow" .yml)
    echo "Analyzing $workflow..."

    pipelinex analyze "$workflow" --format json > "$OUTPUT_DIR/$filename.json"
    pipelinex analyze "$workflow" --format html > "$OUTPUT_DIR/$filename.html"
    pipelinex diff "$workflow" > "$OUTPUT_DIR/$filename-diff.txt" 2>&1 || true
done

echo "✓ Reports saved to $OUTPUT_DIR"
```

### Cost Trend Tracker

```bash
#!/bin/bash
# cost-tracker.sh
# Track cost metrics over time

HISTORY_FILE="$HOME/.pipelinex-history.jsonl"

COST_DATA=$(pipelinex cost .github/workflows/ --format json)

# Append to history with timestamp
echo "{\"timestamp\": \"$(date -Iseconds)\", \"data\": $COST_DATA}" >> "$HISTORY_FILE"

# Generate trend report (requires Python + pandas)
python3 <<EOF
import json
import pandas as pd

with open('$HISTORY_FILE') as f:
    data = [json.loads(line) for line in f]

df = pd.DataFrame(data)
df['date'] = pd.to_datetime(df['timestamp'])
df.set_index('date', inplace=True)

print("\n📈 Cost Trend (Last 30 Days)")
print(df.tail(30)[['data.monthly_compute_cost', 'data.monthly_savings']])
EOF
```

---

## 📚 More Resources

- [Integration Guide](../../docs/INTEGRATIONS.md) - Comprehensive integration documentation
- [Quick Start](../../docs/QUICKSTART.md) - Get started in 5 minutes
- [Contributing](../../CONTRIBUTING.md) - Add your own integration examples

---

**Have a useful integration?** Submit a PR to share it with the community!
