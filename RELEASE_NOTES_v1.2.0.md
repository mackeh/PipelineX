# PipelineX v1.2.0

PipelineX v1.2.0 delivers optimization impact tracking so teams can quantify and report monthly time savings from applied pipeline improvements.

## Highlights

- Added persisted optimization impact tracking with monthly savings metrics.
- Added dashboard impact endpoints:
  - `POST /api/impact/track`
  - `GET /api/impact/stats`
- Added public impact endpoints for integrations:
  - `POST /api/public/v1/impact/track`
  - `GET /api/public/v1/impact/stats`
- Extended public/enterprise API scopes and role mappings with:
  - `impact:read`
  - `impact:write`
- Public OpenAPI descriptor now advertises impact endpoints.
- Benchmark submit endpoints can auto-track impact using `runsPerMonth`.

## New Environment Variable

- `PIPELINEX_IMPACT_DEFAULT_RUNS_PER_MONTH`
  - Optional fallback value when impact tracking payload omits `runsPerMonth`.

## Documentation

- Updated:
  - `dashboard/README.md`
  - `docs/REST_API.md`
  - `docs/SELF_HOSTING.md`
  - `README.md`
  - `pipelinex-project.md`

## Install / Upgrade

```bash
cargo install pipelinex-cli --force
```

## Artifact

This release includes a Linux x86_64 CLI artifact:

- `pipelinex-v1.2.0-linux-x86_64.tar.gz`
