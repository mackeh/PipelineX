# PipelineX v2.4.0 - Expansion Release

**Release Date**: February 12, 2026  
**Status**: Production Ready  
**Breaking Changes**: None

---

## Highlights

- Added first-class parser support for **Argo Workflows**, **Tekton Pipelines**, and **Drone/Woodpecker CI**.
- Added `pipelinex explain` for actionable, plain-English finding explanations.
- Added `pipelinex what-if` for impact simulation before editing CI config files.
- Expanded integration coverage with new fixtures/tests for all newly supported providers.

---

## New Commands

### `pipelinex explain`
Explain findings with remediation and impact context:

```bash
pipelinex explain .github/workflows/ci.yml --runs-per-month 800
```

### `pipelinex what-if`
Simulate modifications and inspect duration/critical-path deltas:

```bash
pipelinex what-if .github/workflows/ci.yml \
  --modify "add-cache build 120" \
  --modify "remove-dep lint->deploy"
```

---

## Platform Expansion

Newly supported providers:
- Argo Workflows (`Workflow`, `WorkflowTemplate`, DAG/steps templates)
- Tekton (`Pipeline`, `PipelineRun`, `Task`, `runAfter`, `finally`)
- Drone CI / Woodpecker (`depends_on`, multi-document pipelines)

PipelineX now supports **11 CI systems**.

---

## Reliability Improvements

- Multi-document YAML handling improved for Argo and Tekton.
- Auto-detection tightened to avoid false Argo detection on paths such as `Cargo.toml`.
- Monorepo discovery now includes `.drone.yml`, `.woodpecker.yml`, and common Argo/Tekton folders.

---

## Documentation Updates

- `README.md` updated for platform and command coverage.
- `docs/QUICKSTART.md` updated with Argo/Tekton/Drone examples.
- New `docs/EXPLAIN_WHATIF.md` usage guide.
- `pipelinex-roadmap.md` updated with implemented v3 expansion items.
