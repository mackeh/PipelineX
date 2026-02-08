# PipelineX Dashboard (Phase 3 Starter)

This dashboard is now wired to the real PipelineX CLI analyzer.

## What it does

- Discovers pipeline configs from:
  - `.github/workflows`
  - `tests/fixtures`
- Runs live analysis via:
  - `POST /api/analyze`
  - `GET /api/workflows`
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
