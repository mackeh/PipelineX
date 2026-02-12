# Repository Guidelines

## Project Structure & Module Organization
PipelineX is a Rust workspace with supporting web and editor tooling.

- `crates/pipelinex-core/`: core analysis engine (parsers, analyzers, optimizers, security checks).
- `crates/pipelinex-cli/`: `pipelinex` binary entrypoint and terminal display logic.
- `crates/pipelinex-core/tests/integration_tests.rs`: cross-module integration tests.
- `tests/fixtures/`: provider-specific CI samples (`github-actions/`, `gitlab-ci/`, `jenkins/`, etc.) used by tests.
- `dashboard/`: Next.js dashboard and API routes.
- `vscode-extension/`: VS Code extension source and build config.
- `docs/`, `examples/`, `deploy/helm/`: docs, integration examples, and deployment assets.

## Build, Test, and Development Commands
- `make help`: list all common tasks.
- `make all`: run format, lint, tests, then release build.
- `make build`, `make test`, `make lint`, `make fmt`: standard Rust development cycle.
- `cargo run -- analyze tests/fixtures/github-actions/simple-ci.yml`: run CLI locally against a fixture.
- `cd dashboard && npm install && npm run dev`: run dashboard at `http://localhost:3000`.
- `cd dashboard && npm run lint`: run dashboard ESLint checks.
- `cd vscode-extension && npm install && npm run build`: compile extension.

## Coding Style & Naming Conventions
- Rust: format with `cargo fmt --all`; lint with `cargo clippy --all-targets -- -D warnings`.
- Keep Rust modules/functions idiomatic: `snake_case` files/functions, `PascalCase` types, focused functions.
- TypeScript/Next.js: follow `dashboard/eslint.config.mjs` and strict TS settings in `dashboard/tsconfig.json`.
- React components use `PascalCase` filenames (for example, `dashboard/components/DagExplorer.tsx`).

## Testing Guidelines
- Primary framework is Rust’s built-in test harness (`#[test]`, integration tests via Cargo).
- Run full suite with `cargo test --all`; run integration-only with `cargo test --test integration_tests`.
- Add/extend fixtures under `tests/fixtures/<provider>/` when adding parser or analyzer behavior.
- No fixed coverage percentage is enforced, but new logic should include unit + integration coverage.

## Commit & Pull Request Guidelines
- Follow Conventional Commit style seen in history: `feat:`, `fix:`, `docs:`, `chore:`, optional scope (`fix(cli): ...`).
- Use imperative, specific subjects (example: `feat(parser): add Tekton matrix dependency parsing`).
- PRs should include: what changed, why, testing performed, and screenshots/GIFs for dashboard UI changes.
- Link relevant issues and call out breaking changes explicitly.

## Security & Configuration Tips
- Never commit secrets or private keys; pre-commit hooks include secret detection.
- Validate workflow changes with `make analyze` or `pipelinex analyze .github/workflows/ci.yml`.
