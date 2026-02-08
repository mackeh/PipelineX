# Self-Hosted Deployment

PipelineX supports self-hosted deployment for the dashboard and APIs via Docker Compose and Helm.

## Prerequisites

- Docker 24+ and Docker Compose v2
- Kubernetes 1.25+ and Helm 3.12+ (for Helm option)
- A workspace repo mounted or bootstrapped at `/workspace` containing pipeline files to analyze

## Option 1: Docker Compose

From repository root:

```bash
docker compose -f docker-compose.selfhost.yml up -d --build
```

Open:

- `http://localhost:3000`

Default persisted volumes:

- `pipelinex_workspace` mounted at `/workspace`
- `pipelinex_data` mounted at `/data`

To use your local repository as workspace instead of named volume:

```yaml
services:
  pipelinex-dashboard:
    volumes:
      - ./:/workspace
      - pipelinex_data:/data
```

### Configure API auth

Add environment values to the `pipelinex-dashboard` service:

```yaml
environment:
  PIPELINEX_API_KEY: "replace-me"
  PIPELINEX_API_KEY_SCOPES: "benchmarks:read,benchmarks:write,workflows:read,analysis:run,history:read,history:write,impact:read,impact:write,audit:read"
  PIPELINEX_API_KEY_ROLES: "admin"
```

Optional enterprise auth:

```yaml
environment:
  PIPELINEX_ENTERPRISE_SESSION_SECRET: "replace-me"
  PIPELINEX_SSO_SHARED_SECRET: "replace-me"
```

## Option 2: Helm (Kubernetes)

Chart path:

- `deploy/helm/pipelinex-dashboard`

### Install

```bash
helm upgrade --install pipelinex deploy/helm/pipelinex-dashboard \
  --namespace pipelinex \
  --create-namespace
```

### Common overrides

```bash
helm upgrade --install pipelinex deploy/helm/pipelinex-dashboard \
  --namespace pipelinex \
  --create-namespace \
  --set image.repository=mackeh/pipelinex-dashboard \
  --set image.tag=latest \
  --set ingress.enabled=true \
  --set ingress.className=nginx \
  --set ingress.hosts[0].host=pipelinex.example.com
```

### Workspace bootstrap (optional)

Use an init container to clone a repository into `/workspace` on startup:

```bash
helm upgrade --install pipelinex deploy/helm/pipelinex-dashboard \
  --namespace pipelinex \
  --set workspaceBootstrap.enabled=true \
  --set workspaceBootstrap.repoUrl=https://github.com/your-org/your-repo.git \
  --set workspaceBootstrap.branch=main
```

For private repos, set:

- `workspaceBootstrap.authSecretName`
- `workspaceBootstrap.authSecretKey`

and create the secret in the same namespace.

## Runtime env defaults

These are wired by default in both deployment modes:

- `PIPELINEX_REPO_ROOT=/workspace`
- `PIPELINEX_PUBLIC_API_RATE_LIMIT_FILE=/data/public-api-rate-limits.json`
- `PIPELINEX_PUBLIC_API_AUDIT_LOG_FILE=/data/public-api-audit.log`

## Notes

- `helm lint` was not executed in this environment because Helm is not installed here.
- Docker build validation for `dashboard/Dockerfile` completed successfully.
