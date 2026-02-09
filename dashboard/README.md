# PipelineX Dashboard (Phase 3 + Phase 4 Increment)

The dashboard now supports:

- Live pipeline analysis from real `pipelinex` CLI output
- GitHub webhook ingestion for workflow history refresh
- GitLab webhook ingestion for pipeline history refresh
- Anonymized community benchmark submissions and cohort comparisons

## Run locally

From repository root:

```bash
cd dashboard
npm run dev
```

Open `http://localhost:3000`.

## Self-hosted deploy

- Docker Compose stack: `docker-compose.selfhost.yml` (from repo root)
- Helm chart: `deploy/helm/pipelinex-dashboard`
- Guide: `docs/SELF_HOSTING.md`

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

### `POST /api/gitlab/webhook`

Accepts GitLab webhook payloads and refreshes history cache when:

- `object_kind = pipeline`
- `object_attributes.status` is a completed pipeline state (`success`, `failed`, `canceled`, `skipped`, `manual`)

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

### `POST /api/impact/track`

Tracks optimization impact and computes estimated monthly savings.

Request supports either:

- `{ report: AnalysisReport, runsPerMonth?: number, source?: string }`
- `{ beforeDurationSecs: number, afterDurationSecs: number, runsPerMonth: number, source?: string, provider?: string }`

### `GET /api/impact/stats`

Returns optimization impact summary metrics.

Query params:

- `source` (optional)
- `provider` (optional)

### Public API (keyed)

- `GET /api/public/v1/auth/me`
- `GET /api/public/v1/openapi`
- `GET /api/public/v1/workflows`
- `POST /api/public/v1/analyze`
- `GET /api/public/v1/history`
- `POST /api/public/v1/history`
- `GET /api/public/v1/impact/stats`
- `POST /api/public/v1/impact/track`
- `GET /api/public/v1/audit/logs`
- `GET /api/public/v1/benchmarks/stats`
- `POST /api/public/v1/benchmarks/submit`

Auth header options:

- `Authorization: Bearer <token>`
- `x-api-key: <token>`

Role-aware key config is supported via:

- `PIPELINEX_API_KEY_ROLES` (CSV in single-key mode)
- `roles` array on entries in `PIPELINEX_API_KEYS` / `PIPELINEX_API_KEYS_FILE`

Built-in roles:

- `admin`: full access (`benchmarks`, `audit`, `workflows`, `analysis`, `history`, `impact`)
- `analyst`: `benchmarks:read`, `audit:read`, `workflows:read`, `analysis:run`, `history:read`, `impact:read`
- `ingest`: `benchmarks:write`, `analysis:run`, `history:write`, `impact:write`
- `viewer`: `benchmarks:read`, `workflows:read`, `history:read`, `impact:read`
- `auditor`: `audit:read`

Custom integration scopes:

- `workflows:read`
- `analysis:run`
- `history:read`
- `history:write`
- `impact:read`
- `impact:write`

`GET /api/public/v1/audit/logs` query params:

- `keyId`
- `scope`
- `method`
- `pathContains`
- `status`
- `since` (ISO timestamp)
- `until` (ISO timestamp)
- `limit` (max 1000)

### Enterprise auth

- `POST /api/enterprise/v1/sso/exchange`
- `GET /api/enterprise/v1/auth/me`

`POST /api/enterprise/v1/sso/exchange` accepts an HMAC-signed assertion and returns a short-lived enterprise session token. Enterprise tokens can be sent as:

- `Authorization: Bearer pxe.<payload>.<signature>`
- `x-enterprise-token: pxe.<payload>.<signature>`

## Environment variables

- `GITHUB_TOKEN`: used for history refresh calls.
- `GITHUB_WEBHOOK_SECRET`: enables webhook signature validation.
- `GITLAB_WEBHOOK_TOKEN` or `GITLAB_WEBHOOK_SECRET_TOKEN`: optional shared token validation for GitLab webhooks.
- `PIPELINEX_GITLAB_WORKFLOW_PATH`: optional workflow identifier stored for GitLab snapshots (default `.gitlab-ci.yml`).
- `PIPELINEX_HISTORY_RUNS`: optional lookback window for webhook-triggered refreshes (default `100`).
- `PIPELINEX_API_KEY`: single-key public API mode.
- `PIPELINEX_API_KEY_ROLES`: CSV roles for the single-key public API mode.
- `PIPELINEX_API_KEYS`: JSON array of key configs (supports rotation fields).
- `PIPELINEX_API_KEYS_FILE`: JSON file path for key config (recommended in production).
- `PIPELINEX_API_RATE_LIMIT_PER_MINUTE`: default per-key rate limit.
- `PIPELINEX_PUBLIC_API_RATE_LIMIT_FILE`: override persistent rate-limit store file path.
- `PIPELINEX_PUBLIC_API_AUDIT_LOG_FILE`: override JSONL audit log file path.
- `PIPELINEX_ENTERPRISE_SESSION_SECRET`: required to sign/verify enterprise session tokens.
- `PIPELINEX_ENTERPRISE_SESSION_TTL_SECONDS`: optional enterprise session TTL.
- `PIPELINEX_SSO_SHARED_SECRET`: required to verify inbound SSO assertions.
- `PIPELINEX_ENTERPRISE_RATE_LIMIT_PER_MINUTE`: optional enterprise-token rate limit.
- `PIPELINEX_IMPACT_DEFAULT_RUNS_PER_MONTH`: optional default used for impact tracking when `runsPerMonth` is omitted.

## Local persistence

- History cache: `.pipelinex/history-cache/`
- Benchmark registry: `.pipelinex/benchmark-registry.json`
- Optimization impact registry: `.pipelinex/optimization-impact-registry.json`
- Public API rate-limit store: `.pipelinex/public-api-rate-limits.json`
- Public API audit log: `.pipelinex/public-api-audit.log`

No repository names or workflow paths are stored in benchmark entries.
