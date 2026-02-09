# PipelineX v1.4.0

PipelineX v1.4.0 advances the Phase 3 platform backlog with richer dashboard intelligence and weekly team reporting.

## Added

- **Interactive DAG explorer (D3)** in the dashboard:
  - Visual graph of critical-path jobs and bottleneck categories
  - Drag/zoom exploration for faster triage
- **Trend analysis charts** in the dashboard:
  - Duration trend over snapshot history
  - Failure-rate trend over snapshot history
  - Cost-per-run trend derived from duration and labor-rate assumptions
- **Weekly digest reporting API**:
  - `GET /api/digest/weekly` to generate digest summaries
  - `POST /api/digest/weekly` to generate and optionally deliver digests
  - Optional delivery channels:
    - Slack incoming webhook
    - Microsoft Teams incoming webhook
    - Email outbox queue (`.pipelinex/digest-email-outbox.jsonl`)

## Documentation and Roadmap

- Updated `pipelinex-project.md` to mark completed Phase 3 backlog items:
  - Interactive DAG visualization
  - Trend analysis charts
  - Slack/Teams/email weekly digest reports
- Updated dashboard docs and project status summaries.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `cd dashboard && npm run lint`
- `cd dashboard && npm run build`

## Install / Upgrade

```bash
cargo install pipelinex-cli --force
```

## Artifact

This release includes a Linux x86_64 CLI artifact:

- `pipelinex-v1.4.0-linux-x86_64.tar.gz`
