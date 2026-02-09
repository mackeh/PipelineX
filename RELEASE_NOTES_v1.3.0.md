# PipelineX v1.3.0

PipelineX v1.3.0 delivers two major Phase 4 capabilities: CI provider migration and multi-repo orchestration analysis.

## Added

- **CI provider migration assistant**:
  - Converts GitHub Actions workflows into GitLab CI skeletons.
  - New command:
    - `pipelinex migrate <workflow> --to gitlab-ci`
  - Supports `text`, `json`, and `yaml` output.
  - Emits conversion warnings for steps needing manual follow-up.

- **Multi-repo pipeline analysis**:
  - Detects cross-repo orchestration edges.
  - Identifies fan-out hubs, fan-in risk, repeated command patterns, and monorepo orchestration risk.
  - New command:
    - `pipelinex multi-repo <root-dir> [--format json]`

## Documentation

- Added:
  - `docs/MIGRATION_ASSISTANT.md`
  - `docs/MULTI_REPO_ANALYSIS.md`
- Updated roadmap/status tracking for completed Phase 4 items.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --workspace`

## Install / Upgrade

```bash
cargo install pipelinex-cli --force
```

## Artifact

This release includes a Linux x86_64 CLI artifact:

- `pipelinex-v1.3.0-linux-x86_64.tar.gz`
