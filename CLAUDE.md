# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Build
cargo build --release          # Release binary → target/release/pipelinex
cargo build                    # Debug build

# Test
cargo test --all               # All tests (unit + integration)
cargo test --all -- --nocapture  # Tests with stdout
cargo test test_name           # Single test by name
cargo test --test integration_tests  # Integration tests only (in pipelinex-core)

# Lint & Format
cargo clippy --all-targets -- -D warnings  # Clippy (CI enforces -D warnings)
cargo fmt --all                # Format
cargo fmt --all -- --check     # Check formatting without changing

# Install locally
cargo install --path crates/pipelinex-cli --force
```

There is also a `Makefile` with shortcuts (`make test`, `make lint`, `make fmt`, `make all`).

## Architecture

### Workspace Layout

Two crates in a Cargo workspace:
- **`pipelinex-core`** — Library crate with all analysis logic
- **`pipelinex-cli`** — Binary crate (`pipelinex`) that wires CLI args to core functions

A **`dashboard/`** directory contains a separate Next.js (React) web app for visualization — it is not part of the Rust workspace.

### Core Data Flow

All CI configs (8 providers) are normalized into one shared type:

```
CI Config File → Parser → PipelineDag → Analyzer → AnalysisReport → Optimizer/Output
```

1. **Parsers** (`parser/*.rs`) — Each CI provider has a `parse_file(path) -> Result<PipelineDag>` function. Provider detection is done by filename/path matching in `pipelinex-cli/src/main.rs:parse_pipeline()`.

2. **`PipelineDag`** (`parser/dag.rs`) — The universal pipeline representation. Uses `petgraph::DiGraph<JobNode, DagEdge>` with a `node_map: HashMap<String, NodeIndex>` for ID-based lookup. **Cannot derive Serialize** because `petgraph::Graph` and `NodeIndex` don't implement it.

3. **Analyzers** (`analyzer/mod.rs`) — `analyze(&PipelineDag) -> AnalysisReport` runs all detectors in sequence: critical path, cache, parallelization, waste, runner sizing, plus external plugins. Each detector module returns `Vec<Finding>`.

4. **Optimizer** (`optimizer/mod.rs`) — Takes the original YAML file + `AnalysisReport` and applies fixes by mutating a `serde_yaml::Value` tree. Sub-modules handle cache injection, parallelization, sharding, and Docker optimizations.

5. **CLI display** (`pipelinex-cli/src/display.rs`) — All terminal formatting is here. Output formats: text (colored), JSON, SARIF, HTML.

### Key Types

- `PipelineDag` — Core DAG with `graph`, `node_map`, `triggers`, `provider`
- `JobNode` — A CI job: steps, dependencies, caches, matrix, conditions, env
- `AnalysisReport` — Full analysis output with findings, health score, timing
- `Finding` — Single issue with severity, category, recommendation, estimated savings
- `Severity` — Critical > High > Medium > Low > Info (sorted by `priority()`)

### Adding a New CI Parser

Each parser lives in `parser/<provider>.rs` and exposes a struct with `parse_file(path) -> Result<PipelineDag>`. Add the new module to `parser/mod.rs`, re-export from `lib.rs`, add filename detection logic in `main.rs:parse_pipeline()`, and add fixtures under `tests/fixtures/<provider>/`.

### Adding a New Analyzer

Create a module in `analyzer/` that takes `&PipelineDag` and returns `Vec<Finding>`. Wire it into `analyzer::analyze()` in `analyzer/mod.rs`. Add a `FindingCategory` variant in `report.rs`.

## Test Fixtures

Test fixtures live at `tests/fixtures/` (workspace root, not inside any crate). Integration tests in `pipelinex-core/tests/integration_tests.rs` reference them via `CARGO_MANIFEST_DIR` walking up to workspace root. Fixture directories: `github-actions/`, `gitlab-ci/`, `jenkins/`, `circleci/`, `bitbucket/`, `azure-pipelines/`, `aws-codepipeline/`, `buildkite/`, `dockerfiles/`, `junit/`.

## Known Gotchas

- `serde_yaml::Value::get()` does not accept `bool` as an index — the YAML key `on: true` (GitHub Actions trigger) needs special handling
- `PipelineDag` uses `petgraph::DiGraph` which doesn't implement `Serialize`, so the DAG itself can't be serialized directly
- The optimizer mutates `serde_yaml::Value` in-place rather than building from the DAG, to preserve original YAML structure and comments as much as possible
