# Migration Assistant

PipelineX includes a built-in migration assistant for converting GitHub Actions workflows into a GitLab CI pipeline skeleton.

## Supported Migration

- Source: `github-actions`
- Target: `gitlab-ci`

## Usage

```bash
# Print migrated GitLab YAML to stdout
pipelinex migrate .github/workflows/ci.yml --to gitlab-ci --format yaml

# Write migrated config to a file
pipelinex migrate .github/workflows/ci.yml --to gitlab-ci -o .gitlab-ci.yml

# Include migration summary + warnings as JSON
pipelinex migrate .github/workflows/ci.yml --to gitlab-ci --format json
```

## What Gets Converted

- Job graph and dependencies (`needs`)
- Stage ordering based on dependency depth
- Shell commands from `run:` steps
- Top-level and job-level environment variables
- Matrix strategy into GitLab `parallel.matrix` (where possible)
- Core trigger mapping into `workflow.rules`:
  - `push` -> `$CI_PIPELINE_SOURCE == "push"`
  - `pull_request` -> `$CI_PIPELINE_SOURCE == "merge_request_event"`
  - `workflow_dispatch` -> `$CI_PIPELINE_SOURCE == "web"`
  - `schedule` -> `$CI_PIPELINE_SOURCE == "schedule"`

## Manual Follow-Up Required

Some GitHub Actions concepts do not have a 1:1 translation. PipelineX will emit warnings when these appear:

- Third-party `uses:` actions that need manual replacement
- Complex `if:` expressions that need GitLab `rules:` equivalents
- Windows/macOS runner assumptions that need custom GitLab runners

Treat migrated output as a strong starting point, then validate in your target GitLab project.
