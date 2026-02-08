# PipelineX Dashboard (Phase 3 Starter)

This dashboard is now wired to the real PipelineX CLI analyzer.

## What it does

- Discovers pipeline configs from:
  - `.github/workflows`
  - `tests/fixtures`
- Runs live analysis via:
  - `POST /api/analyze`
  - `GET /api/workflows`
- Ingests GitHub Actions webhooks and auto-refreshes historical stats:
  - `POST /api/github/webhook`
  - `GET /api/history`
- Renders real metrics, severity breakdown, critical path, and recommendations.

## Run locally

From repository root:

```bash
cd dashboard
npm run dev
```

Open `http://localhost:3000`.

## API contract

### `GET /api/workflows`

Response:

```json
{
  "files": [
    ".github/workflows/ci.yml",
    "tests/fixtures/github-actions/simple-ci.yml"
  ]
}
```

### `POST /api/analyze`

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

Refreshes by executing:

```bash
pipelinex history --repo <owner/repo> --workflow <workflow-file> --runs <N> --format json
```

### `GET /api/history`

Returns cached snapshots:

```json
{
  "snapshots": [
    {
      "repo": "owner/repo",
      "workflow": ".github/workflows/ci.yml",
      "source": "webhook"
    }
  ]
}
```

## Environment variables

- `GITHUB_TOKEN`: token used for history refresh calls (recommended).
- `GITHUB_WEBHOOK_SECRET`: if set, webhook signatures are required and validated.
- `PIPELINEX_HISTORY_RUNS`: optional lookback run count for auto-refresh (default `100`).

Response:

```json
{
  "report": {
    "pipeline_name": "CI",
    "provider": "github-actions",
    "findings": []
  }
}
```

## Notes

- The API runs only on Node.js runtime (not Edge).
- Paths are validated to stay inside repo root.
- The route prefers local `target/debug/pipelinex` or `target/release/pipelinex`; otherwise it falls back to `cargo run -p pipelinex-cli`.
- History snapshots are persisted under `.pipelinex/history-cache/`.
