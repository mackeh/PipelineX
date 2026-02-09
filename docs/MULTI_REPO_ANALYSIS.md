# Multi-Repo Analysis

PipelineX can analyze CI orchestration patterns across many repositories and detect monorepo fanout risks.

## Command

```bash
# Analyze a root directory containing many repos
pipelinex multi-repo /path/to/repos

# JSON output for automation
pipelinex multi-repo /path/to/repos --format json
```

## What It Detects

- Cross-repo trigger edges (dispatch/workflow trigger patterns)
- Orchestration fan-out hubs (one repo triggering many repos)
- Orchestration fan-in (one repo receiving triggers from many upstream repos)
- Repeated command patterns that should be centralized into reusable templates
- Monorepo orchestration risk (multiple push/PR workflows without path scoping)
- Duration skew across repositories (one repo significantly slower than peers)

## Supported Config Discovery

Per repository directory, PipelineX scans common CI config locations:

- `.github/workflows/*.yml|*.yaml`
- `.gitlab-ci.yml|.gitlab-ci.yaml`
- `Jenkinsfile`
- `.circleci/config.yml|config.yaml`
- `bitbucket-pipelines.yml|.yaml`
- `azure-pipelines.yml|.yaml`
- `.buildkite/pipeline.yml|.yaml`
- `codepipeline.json|.yml|.yaml`

## Notes

- Treat orchestration edges as inferred relationships and review before rollout.
- If a file is not parseable as a supported CI config, it is skipped and reported.
