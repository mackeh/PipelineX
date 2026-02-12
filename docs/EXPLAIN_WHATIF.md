# Explain & What-If Guide

PipelineX includes two workflow-planning commands:

- `explain`: turns findings into plain-English remediation steps with impact context.
- `what-if`: simulates DAG/config changes without editing your pipeline files.

## `pipelinex explain`

```bash
pipelinex explain .github/workflows/ci.yml
pipelinex explain .github/workflows/ --format json --runs-per-month 800
```

Behavior:
- Runs normal analysis first.
- Produces one explanation per finding (`why it matters`, `impact`, `simplest fix`).
- Uses template mode by default; if `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` is set, it can call an LLM backend.

## `pipelinex what-if`

```bash
pipelinex what-if .github/workflows/ci.yml --modify "add-cache build 120"
pipelinex what-if .github/workflows/ci.yml --modify "remove-dep lint->deploy" --modify "change-runner test ubuntu-latest-16-core"
pipelinex what-if .github/workflows/ci.yml --format json
```

Supported modification commands:

- `add-cache <job> [savings_secs]`
- `remove-cache <job>`
- `add-dep <from>-><to>`
- `remove-dep <from>-><to>`
- `remove-job <job>`
- `set-duration <job> <seconds>`
- `change-runner <job> <runner>`

Output includes:
- original vs modified estimated duration
- critical path delta
- findings delta
- applied modifications and warnings for invalid operations
