# PipelineX v2.4.1 - Simulation UX & Output Polish

**Release Date**: February 13, 2026  
**Status**: Production Ready  
**Breaking Changes**: None

---

## Highlights

- Improved `pipelinex simulate` usability for large runs with progress feedback and bounded text output.
- Added explicit JSON guidance for large text outputs to improve scriptability and automation workflows.
- Updated CLI/docs to make machine-readable output paths clearer and reduce terminal noise.

---

## Added

- `pipelinex simulate --top-jobs <N>` to limit per-job rows in text mode.
- `pipelinex simulate --no-progress` to disable progress updates in CI/non-interactive logs.

---

## Changed

- `pipelinex simulate` now shows progress for long text-mode runs (5,000+ runs on TTY), including total runtime when complete.
- Text simulation output now truncates long job tables by default and points users to `--format json` for complete data.
- `pipelinex analyze` now suggests `--format json` when many findings are printed in text mode.

---

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `cargo build --release`
- `dashboard`: `npm run lint`, `npm run build`
- `vscode-extension`: `npm run build`
