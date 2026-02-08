# PipelineX VS Code Extension

Inline optimization hints for CI workflow files using `pipelinex analyze`.

## Features

- Diagnostics on pipeline files with actionable recommendations
- Code lenses with quick hint summaries
- Manual command: `PipelineX: Analyze Current Workflow`
- Auto analysis on open/save (configurable)

## Requirements

- `pipelinex` available in PATH, or set `pipelinex.commandPath`

## Supported file patterns

- `.github/workflows/*.yml`, `.github/workflows/*.yaml`
- `.gitlab-ci.yml`, `.gitlab-ci.yaml`
- `Jenkinsfile`
- `bitbucket-pipelines.yml`
- `.circleci/config.yml`
- `azure-pipelines.yml`
- `.buildkite/pipeline.yml`

## Development

```bash
cd vscode-extension
npm install
npm run build
```

Then open this folder in VS Code and run the `Run Extension` launch config.
