# PipelineX v1.1.0

PipelineX v1.1.0 expands platform capabilities across providers, integrations, deployment, and developer workflow tooling.

## Highlights

- Added parser support for **Azure Pipelines**, **AWS CodePipeline**, and **Buildkite**.
- Added **public REST API endpoints for custom integrations**:
  - `GET /api/public/v1/workflows`
  - `POST /api/public/v1/analyze`
  - `GET/POST /api/public/v1/history`
  - `GET /api/public/v1/openapi`
- Added enterprise auth/governance upgrades:
  - role-based scopes (RBAC)
  - persistent public API rate limits
  - public API audit logs + query endpoint
  - enterprise SSO assertion exchange and session auth endpoints
- Added **self-hosted deployment** support:
  - `docker-compose.selfhost.yml`
  - Helm chart: `deploy/helm/pipelinex-dashboard`
- Added plugin extensibility scaffold:
  - `pipelinex plugins list`
  - `pipelinex plugins scaffold`
- Added a local **VS Code extension** with inline workflow optimization hints (`vscode-extension/`).

## Documentation

- New guides:
  - `docs/REST_API.md`
  - `docs/SELF_HOSTING.md`
  - `docs/VS_CODE_EXTENSION.md`
  - `docs/PLUGINS.md`

## Install / Upgrade

```bash
cargo install pipelinex-cli --force
```

## Artifact

This release includes a Linux x86_64 CLI artifact:

- `pipelinex-v1.1.0-linux-x86_64.tar.gz`
