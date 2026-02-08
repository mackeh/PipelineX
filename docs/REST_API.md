# REST API for Custom Integrations

PipelineX provides a versioned public REST API at `/api/public/v1` for external automation and integrations.

## Authentication

Send one of:

- `Authorization: Bearer <token>`
- `x-api-key: <token>`

`<token>` can be:

- a configured public API key (`PIPELINEX_API_KEY`, `PIPELINEX_API_KEYS`, or `PIPELINEX_API_KEYS_FILE`)
- an enterprise session token (`pxe.<payload>.<signature>`)

## Endpoint index

- `GET /api/public/v1/openapi`
- `GET /api/public/v1/auth/me`
- `GET /api/public/v1/workflows`
- `POST /api/public/v1/analyze`
- `GET /api/public/v1/history`
- `POST /api/public/v1/history`
- `GET /api/public/v1/benchmarks/stats`
- `POST /api/public/v1/benchmarks/submit`
- `GET /api/public/v1/audit/logs`

## Scopes

- `workflows:read`: list discoverable pipeline files
- `analysis:run`: run analysis for a pipeline file
- `history:read`: read cached workflow history snapshots
- `history:write`: refresh workflow history snapshots
- `benchmarks:read`: query benchmark cohort stats
- `benchmarks:write`: submit benchmark reports
- `audit:read`: query public API audit logs

## Quick examples

```bash
# Who am I?
curl -sS \
  -H "Authorization: Bearer $PIPELINEX_API_KEY" \
  http://localhost:3000/api/public/v1/auth/me

# List workflows
curl -sS \
  -H "Authorization: Bearer $PIPELINEX_API_KEY" \
  http://localhost:3000/api/public/v1/workflows

# Analyze a workflow
curl -sS \
  -H "Authorization: Bearer $PIPELINEX_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"pipelinePath":".github/workflows/ci.yml"}' \
  http://localhost:3000/api/public/v1/analyze

# List cached history snapshots
curl -sS \
  -H "Authorization: Bearer $PIPELINEX_API_KEY" \
  http://localhost:3000/api/public/v1/history

# Refresh a specific history snapshot
curl -sS \
  -H "Authorization: Bearer $PIPELINEX_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"repo":"owner/repo","workflow":".github/workflows/ci.yml","runs":100}' \
  http://localhost:3000/api/public/v1/history
```

## Response metadata

Authenticated responses include rate limit headers:

- `x-ratelimit-limit`
- `x-ratelimit-remaining`
- `x-ratelimit-reset`

Audit records are written to `.pipelinex/public-api-audit.log` by default.
