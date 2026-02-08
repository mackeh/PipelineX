# GitHub API Integration

PipelineX can fetch and analyze historical workflow run data from GitHub to provide accurate timing statistics, detect performance degradation, and identify flaky jobs.

## Features

- **Historical Timing Analysis**: Fetch actual run durations from GitHub instead of relying on estimates
- **Statistical Insights**: Calculate p50, p90, p99 percentiles and variance for each job
- **Flaky Job Detection**: Identify jobs with inconsistent behavior across runs
- **Success Rate Tracking**: Monitor pipeline reliability over time
- **Trend Analysis**: Spot performance degradation or improvements

## Usage

### Basic Analysis

```bash
pipelinex history \
  --repo microsoft/vscode \
  --workflow ci.yml \
  --runs 100 \
  --token $GITHUB_TOKEN
```

### Without Token (Rate Limited)

```bash
# GitHub allows 60 requests/hour without authentication
pipelinex history --repo owner/repo --workflow ci.yml --runs 50
```

### With Environment Variable

```bash
# Set token via environment variable
export GITHUB_TOKEN=ghp_your_token_here
pipelinex history --repo owner/repo --workflow ci.yml
```

### JSON Output

```bash
pipelinex history \
  --repo owner/repo \
  --workflow ci.yml \
  --runs 200 \
  --format json > history.json
```

## Example Output

```
🔍 Analyzing workflow run history...
   Repository: mackeh/PipelineX
   Workflow: ci.yml
   Runs to analyze: 100

Fetching 100 workflow runs from GitHub...
Fetched 100 runs, analyzing jobs...
Analyzing run 10/100...
Analyzing run 20/100...
...

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 Pipeline History: CI
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

 Overall Statistics
   Total runs analyzed:  100
   Success rate:         94.5%

 Duration Statistics
   Average:   4m 32s
   Median:    4m 18s
   P90:       6m 45s
   P99:       9m 12s

 Job Performance

   ✅ check
      Average: 45s | P50: 42s | P90: 58s
      Runs: 100 | Success rate: 100.0%

   ✅ test
      Average: 3m 24s | P50: 3m 12s | P90: 4m 30s
      Runs: 100 | Success rate: 98.0%

   🟡 build
      Average: 2m 15s | P50: 2m 05s | P90: 3m 45s
      Runs: 100 | Success rate: 94.0%
      ⚠️  Unstable timing (high variance detected)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 💡 Insights
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

   🔴 P90 is significantly higher than average
      This indicates high variance in pipeline duration.
      Consider investigating slow runs for bottlenecks.

   🟡 1 potentially flaky jobs detected
      Run 'pipelinex flaky' with JUnit reports to analyze test-level flakiness.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

 Use this data to:
   • Identify the slowest jobs for optimization
   • Spot flaky or unstable jobs
   • Track performance trends over time
   • Validate that optimizations reduced duration
```

## Getting a GitHub Token

1. Go to https://github.com/settings/tokens
2. Click "Generate new token" → "Generate new token (classic)"
3. Give it a descriptive name (e.g., "PipelineX Analysis")
4. Select scopes:
   - `repo` (for private repos) OR
   - `public_repo` (for public repos only)
   - `workflow` (to read Actions data)
5. Click "Generate token"
6. Copy the token and either:
   - Pass it with `--token ghp_...` flag
   - Set `export GITHUB_TOKEN=ghp_...`

## Rate Limits

- **Authenticated**: 5,000 requests/hour
- **Unauthenticated**: 60 requests/hour

For analyzing 100 runs with ~10 jobs each, you'll make:
- 1 request to list runs
- 100 requests for job details
- **Total: ~101 requests**

Always use a token for analyzing more than 50 runs.

## Use Cases

### 1. Baseline Your Pipeline

Before optimizing, understand current performance:

```bash
pipelinex history --repo your/repo --workflow ci.yml --runs 200 > baseline.txt
```

### 2. Validate Optimizations

After applying optimizations, compare:

```bash
# After optimization
pipelinex history --repo your/repo --workflow ci.yml --runs 100 > after.txt

# Compare
diff baseline.txt after.txt
```

### 3. Monitor Performance Over Time

Run weekly and track trends:

```bash
#!/bin/bash
DATE=$(date +%Y-%m-%d)
pipelinex history \
  --repo your/repo \
  --workflow ci.yml \
  --runs 100 \
  --format json > "history_$DATE.json"
```

### 4. Identify Slow Runs

```bash
# Get p99 duration to find outliers
pipelinex history --repo your/repo --workflow ci.yml --format json | \
  jq '.p99_duration_sec'
```

### 5. Calculate Monthly CI Cost

```bash
# Average duration × runs per day × 30 days × runner cost
pipelinex history --repo your/repo --workflow ci.yml --format json | \
  jq '.avg_duration_sec'

# Then use with cost command:
pipelinex cost .github/workflows/ --runs-per-month 1500
```

## JSON Schema

```json
{
  "workflow_name": "CI",
  "total_runs": 100,
  "success_rate": 0.945,
  "avg_duration_sec": 272.0,
  "p50_duration_sec": 258.0,
  "p90_duration_sec": 405.0,
  "p99_duration_sec": 552.0,
  "job_timings": [
    {
      "job_name": "test",
      "durations_sec": [180.0, 195.0, ...],
      "success_count": 98,
      "failure_count": 2,
      "avg_duration_sec": 204.0,
      "p50_duration_sec": 192.0,
      "p90_duration_sec": 270.0,
      "p99_duration_sec": 325.0,
      "variance": 12.5
    }
  ],
  "flaky_jobs": ["build"]
}
```

## Troubleshooting

### "Failed to fetch workflow runs"

- Check repository format: `owner/repo` (not full URL)
- Verify workflow file name (just the filename, e.g., `ci.yml`)
- Ensure token has correct permissions

### "Rate limit exceeded"

- Use a GitHub token (see above)
- Reduce `--runs` value
- Wait for rate limit reset (check `X-RateLimit-Reset` header)

### "Workflow not found"

- Workflow file must exist in `.github/workflows/`
- Use just the filename, not the full path
- Check spelling and case sensitivity

## Next Steps

- Combine with `pipelinex analyze` for comprehensive insights
- Use `pipelinex cost` with real duration data
- Track history over time to measure improvement
- Integrate into CI for continuous monitoring

---

**Pro Tip**: Run history analysis monthly to catch performance regression early!
