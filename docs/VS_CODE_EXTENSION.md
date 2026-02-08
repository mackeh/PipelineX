# VS Code Extension

PipelineX includes a local VS Code extension under `vscode-extension/` that surfaces inline workflow optimization hints.

## What it does

- Runs `pipelinex analyze <file> --format json` for supported pipeline files
- Publishes findings as diagnostics inline in the editor
- Adds code lenses with short optimization summaries
- Supports manual and automatic analysis triggers

## Supported pipeline files

- `.github/workflows/*.yml`, `.github/workflows/*.yaml`
- `.gitlab-ci.yml`, `.gitlab-ci.yaml`
- `Jenkinsfile`
- `bitbucket-pipelines.yml`
- `.circleci/config.yml`
- `azure-pipelines.yml`
- `.buildkite/pipeline.yml`

## Run in development

```bash
cd vscode-extension
npm install
npm run build
```

Then in VS Code:

1. Open `vscode-extension/`
2. Press `F5` (Run Extension)
3. In the Extension Development Host, open a repository with workflow files
4. Run command: `PipelineX: Analyze Current Workflow`

## Settings

- `pipelinex.commandPath`: path to `pipelinex` binary
- `pipelinex.autoAnalyzeOnSave`: analyze on save
- `pipelinex.autoAnalyzeOnOpen`: analyze on open
- `pipelinex.showCodeLens`: show inline code lens hints
- `pipelinex.maxHints`: max hints per file
- `pipelinex.severityThreshold`: minimum severity (`info|low|medium|high|critical`)
- `pipelinex.commandTimeoutMs`: analyze command timeout

## Commands

- `PipelineX: Analyze Current Workflow`
- `PipelineX: Show Hint Details`
- `PipelineX: Clear Hints`
