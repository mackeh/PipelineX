# PipelineX Dashboard (Phase 3 + Phase 4 Increment)

The dashboard now supports:

- Live pipeline analysis from real `pipelinex` CLI output
- GitHub webhook ingestion for workflow history refresh
- Anonymized community benchmark submissions and cohort comparisons

## Run locally

From repository root:

```bash
cd dashboard
npm run dev
```

Open `http://localhost:3000`.

## API endpoints

### `GET /api/workflows`

Lists discovered pipeline files from `.github/workflows` and `tests/fixtures`.

### `POST /api/analyze`

Runs live analysis for a selected pipeline path.

Request:

```json
{
  "pipelinePath": ".github/workflows/ci.yml"
}
```

### `POST /api/github/webhook`

Accepts GitHub webhook payloads and refreshes history cache when:

- `x-github-event = workflow_run`
- `action = completed`

### `GET /api/history`

Returns cached workflow history snapshots.

### `POST /api/history`

Manual history refresh endpoint.

Request:

```json
{
  "repo": "owner/repo",
  "workflow": ".github/workflows/ci.yml",
  "runs": 100
}
```

### `POST /api/benchmarks/submit`

Stores anonymized benchmark metrics derived from an analysis report and returns cohort stats.

### `GET /api/benchmarks/stats`

Returns benchmark stats for a cohort.

Query params:

- `provider` (required)
- `jobCount` (required)
- `stepCount` (required)

## Environment variables

- `GITHUB_TOKEN`: used for history refresh calls.
- `GITHUB_WEBHOOK_SECRET`: enables webhook signature validation.
- `PIPELINEX_HISTORY_RUNS`: optional lookback window for webhook-triggered refreshes (default `100`).

## Local persistence

- History cache: `.pipelinex/history-cache/`
- Benchmark registry: `.pipelinex/benchmark-registry.json`

No repository names or workflow paths are stored in benchmark entries.
