# Plugin System (Scaffold)

PipelineX supports manifest-driven external plugins for:

- Analyzer plugins (executed during `pipelinex analyze`)
- Optimizer plugins (manifest scaffolded for future execution flow)

## Manifest path

Set:

```bash
export PIPELINEX_PLUGIN_MANIFEST=.pipelinex/plugins.json
```

Create starter file:

```bash
pipelinex plugins scaffold .pipelinex/plugins.json
```

List discovered plugins:

```bash
pipelinex plugins list
```

## Manifest format

```json
{
  "analyzers": [
    {
      "id": "example-analyzer",
      "command": "node",
      "args": ["plugins/example-analyzer.js"],
      "timeout_ms": 10000,
      "enabled": true
    }
  ],
  "optimizers": [
    {
      "id": "example-optimizer",
      "command": "node",
      "args": ["plugins/example-optimizer.js"],
      "timeout_ms": 10000,
      "enabled": false
    }
  ]
}
```

## Analyzer plugin protocol

PipelineX executes analyzer plugins as subprocesses and sends JSON on stdin:

```json
{
  "pipeline": {
    "name": "CI",
    "provider": "github-actions",
    "job_count": 5
  }
}
```

Plugin stdout must be JSON as either:

1. Array of findings:

```json
[
  {
    "severity": "high",
    "title": "Custom check",
    "description": "Detected issue",
    "recommendation": "Apply fix"
  }
]
```

2. Object envelope:

```json
{
  "findings": [
    {
      "severity": "medium",
      "title": "Another issue",
      "description": "Details",
      "recommendation": "Action"
    }
  ]
}
```

These findings are merged into the main analysis report as `CustomPlugin` findings.
