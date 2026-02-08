# PipelineX v1.2.1

PipelineX v1.2.1 is a patch release focused on CI reliability.

## Fixed

- Resolved rustfmt check failures on parser/plugin files.
- Resolved Clippy `-D warnings` failures:
  - `clippy::question_mark` in Buildkite parser
  - `clippy::len_zero` in integration tests
- Restored green CI on `main`.

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

- `pipelinex-v1.2.1-linux-x86_64.tar.gz`
