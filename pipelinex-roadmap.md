# PipelineX Roadmap

> Last updated: February 2026

---

## Completed Phases

### ✅ v1.0.x — Foundation (Complete)

- Rust-based CLI (`analyze`, `optimize`, `diff`, `cost`, `graph`)
- Multi-platform pipeline parsing: GitHub Actions, GitLab CI, Jenkins, CircleCI, Bitbucket Pipelines, Azure Pipelines, AWS CodePipeline, Buildkite
- Core antipattern detectors: missing caches, serial bottlenecks, false dependencies, Docker inefficiencies
- Auto-generation of optimized pipeline configs
- Multiple output formats: plain text (coloured), JSON, SARIF (GitHub Code Scanning)
- Critical path analysis with estimated savings per finding
- Confidence scoring and auto-fixable detection

### ✅ v1.x — Intelligence & Ecosystem (Complete)

- 12 antipattern detectors (caches, serial bottlenecks, false dependencies, flaky tests, path filtering, matrix bloat, Docker layer caching, and more)
- `pipelinex flaky` — flaky test detection from test result files
- `pipelinex select-tests` — smart test selection based on changed files
- `pipelinex history` — historical run data analysis from GitHub API
- `pipelinex cost` — cost estimation with runs-per-month projection
- DAG visualisation (`pipelinex graph`)
- One-line install script (`install.sh`)
- Docker image for zero-install usage
- GitHub Actions integration with SARIF upload
- Pre-commit hook support
- VS Code extension with inline diagnostics
- Makefile with developer workflow tasks

### ✅ v2.0.x — Platform & Dashboard (Complete)

- Interactive web dashboard with dark mode
- DAG explorer with visual pipeline graph
- Trends and cost centre analysis views
- HTML report output format (interactive, shareable)
- REST API for programmatic access
- Self-hosted deployment via `docker-compose.selfhost.yml`
- Helm chart for Kubernetes deployment (`deploy/helm/pipelinex-dashboard`)
- `.pipelinex/` project-level configuration directory
- Comprehensive documentation (QUICKSTART, INTEGRATIONS, SELF_HOSTING, REST_API, VS_CODE_EXTENSION)
- Examples directory with integration samples

### ✅ v2.1.x — Polish & Reliability (Complete)

- Stability fixes and edge-case handling across supported CI parsers
- Improved confidence scoring accuracy
- Release checklist and publishing workflow
- Implementation verification documentation

---

## Upcoming Phases

### 🔜 v2.2.x — Usability & Adoption (Q2 2026)

#### Installation & Onboarding

- **Package manager distribution**: `brew install pipelinex`, `cargo install pipelinex-cli` on crates.io, `npm`/`npx` wrapper, `.deb`/`.rpm` packages, and Windows `winget`/`scoop` support
- **`pipelinex init`**: Interactive setup wizard that auto-detects CI platform from repo structure, generates a `.pipelinex/config.toml`, and runs the first analysis with guided walkthrough
- **`pipelinex doctor`**: Diagnostic command that checks CI config syntax, validates platform detection, and reports parser coverage gaps in one pass

#### Day-to-Day Workflow

- **`--watch` mode**: File-watching mode that re-analyses pipeline configs on save — instant feedback during CI config editing
- **PR comment bot**: GitHub App / GitLab integration that posts analysis results as inline PR comments when CI configs change — shows findings, estimated savings, and one-click "apply optimized config"
- **`pipelinex explain <finding-id>`**: Deep-dive command that explains a specific finding with real-world context, benchmarks from similar projects, and step-by-step remediation instructions
- **Monorepo support**: Analyse multiple pipeline files across a monorepo with per-package cost attribution and aggregated reporting
- **Config validation mode**: `pipelinex lint` that checks CI configs for syntax errors, deprecated features, and platform-specific gotchas before pushing — a "CI config linter"

#### Dashboard Enhancements

- **Team/org views**: Multi-repo dashboard aggregating pipeline health, cost trends, and optimisation adoption across an entire organisation
- **Before/after comparison**: Side-by-side visualisation of pipeline DAGs before and after optimisation — animated transition showing parallelisation gains
- **Notification system**: Webhook, Slack, and email alerts when pipeline performance regresses (e.g., build time increases by >20% over baseline)
- **Embeddable widgets**: Iframe-ready charts for CI health that teams can embed in internal wikis or Notion pages

#### CLI & Output

- **Shell completions**: Auto-generated completions for Bash, Zsh, Fish, and PowerShell
- **Markdown output format**: Clean markdown reports suitable for pasting into GitHub issues, PRs, or wiki pages
- **`pipelinex compare <config-a> <config-b>`**: Diff two pipeline configs with annotated optimisation delta and estimated time/cost difference

---

### 🛡️ v2.3.x — Security & Trust (Q3 2026)

#### Pipeline Security Analysis

- **Secret exposure detection**: Flag hardcoded secrets, tokens, and credentials in pipeline configs (environment variables, inline scripts, step arguments)
- **Overprivileged permissions audit**: Detect GitHub Actions workflows with `permissions: write-all` or overly broad token scopes — suggest minimal required permissions per job
- **Supply chain risk scoring**: Analyse third-party actions/orbs/images for pinning practices (tag vs SHA), popularity, maintenance status, and known vulnerabilities
- **Untrusted input injection**: Detect patterns where `github.event` fields, PR titles, or branch names flow into `run:` steps unsanitised — a major GitHub Actions attack vector
- **Self-hosted runner risk assessment**: Flag workflows that run on self-hosted runners without appropriate isolation, network restrictions, or ephemeral configuration

#### Compliance & Audit

- **Signed analysis reports**: Cryptographically signed JSON/SARIF output so teams can prove an analysis was run and results weren't tampered with
- **Pipeline change audit trail**: Track which optimisations were applied, when, and by whom — with before/after snapshots stored in `.pipelinex/history/`
- **Compliance policies**: Define organisational rules in TOML/YAML (e.g., "all workflows must pin actions by SHA", "no workflows may use `ubuntu-latest`", "cache must be configured for npm/yarn") — `pipelinex policy check` enforces them
- **SBOM for CI**: Generate a "CI Bill of Materials" listing every action, orb, image, and tool version used across all pipelines

#### Data Protection

- **Offline-only mode**: Guaranteed no network calls — all analysis runs locally with no telemetry, API calls, or external lookups (important for air-gapped/regulated environments)
- **Redacted reports**: Auto-strip sensitive values (repo names, secret names, internal URLs) from reports before sharing externally
- **RBAC for dashboard**: Role-based access control for the self-hosted dashboard — admin, editor, viewer roles with SSO integration (OIDC, SAML)

---

### ✨ v3.0.x — Woo Factor & Intelligence (Q4 2026)

#### AI-Powered Analysis

- **✅ LLM-powered optimisation explanations (implemented)**: `pipelinex explain` now provides finding-by-finding plain-English remediation with template fallback and optional Anthropic/OpenAI backends.
- **AI config generation**: Describe what your pipeline should do in plain English, get an optimised CI config generated — *"Build a Node.js app, run tests in parallel across Node 18 and 20, deploy to AWS on main branch"*
- **Predictive build time**: ML model trained on historical run data that predicts build time for a given PR before it even runs — *"This PR touches 3 test files, estimated CI time: 12 min (vs 31 min baseline)"*
- **Anomaly detection**: Automatically flag pipeline runs that are significantly slower than usual — distinguish between legitimate slowdowns (new tests added) and regressions (cache miss, flaky infra)

#### Visualisation & Impact

- **Live pipeline monitor**: Real-time dashboard showing active CI runs across all repos with live progress bars, step-level timing, and instant bottleneck highlighting — a "mission control" for your CI fleet
- **"Pipeline Health Score" badge**: Embeddable shields.io-style badge for READMEs (`PipelineX Score: A+ | 94% optimised`) — gamification that drives adoption across open-source projects
- **Cost leaderboard**: Org-wide ranking of repos by CI cost efficiency — *"Team Backend saved $2,400/month after applying PipelineX suggestions. Team Frontend: $890 potential savings remaining."* — turns optimisation into a friendly competition
- **✅ Interactive "what-if" simulator (CLI implemented)**: `pipelinex what-if` supports dependency/cache/runner/duration scenario modeling and reports critical-path and duration deltas.
- **Time-lapse replay**: Animate how a pipeline's performance has evolved over weeks/months — watch the DAG optimise in fast-forward as fixes are applied

#### Ecosystem Expansion

- **✅ Tekton and Argo Workflows support (implemented)**: Kubernetes-native CI/CD systems now parse as first-class analysis targets.
- **✅ Drone CI and Woodpecker CI support (implemented)**: Lightweight/self-hosted pipeline configs now parse natively, including multi-doc workflows.
- **✅ MCP (Model Context Protocol) server (implemented)**: PipelineX can now run as an MCP tool for AI assistant workflows.
- **GitHub Marketplace App**: One-click install GitHub App that automatically analyses PRs touching CI configs and posts optimisation suggestions
- **Terraform CI module**: IaC module that provisions PipelineX dashboard alongside your CI infrastructure
- **JetBrains IDE plugin**: IntelliJ/GoLand/WebStorm plugin with inline pipeline analysis, DAG preview, and quick-fix actions

#### Developer Experience

- **Online playground**: Browser-based "paste your CI config" analyser using a WASM build — zero install, instant demo, shareable results via URL
- **`pipelinex benchmark`**: Run your pipeline N times and produce statistical analysis (p50, p95, p99 build times, variance, flakiness rate) with visualisation
- **Plugin system**: User-extensible antipattern detectors — write custom rules in Rust or WASM, distribute via a plugin registry
- **VS Code extension v2**: Inline DAG preview in the editor, hover cards with cost estimates per job, and "optimise this file" code action

---

## Long-Term Vision (2027+)

- **Cross-pipeline dependency analysis**: Detect bottlenecks that span multiple pipelines (e.g., a deploy pipeline waiting on a build pipeline that's slow because of a test pipeline)
- **Automatic PR generation**: When PipelineX finds optimisations, it opens a PR with the optimised config, a summary of changes, and projected savings — fully automated
- **PipelineX Cloud**: Hosted SaaS with org management, historical analytics, SSO, and managed dashboards — no self-hosting required
- **CI provider cost API integration**: Pull actual billing data from GitHub Actions, GitLab CI, CircleCI, and Buildkite to show real (not estimated) cost savings
- **FinOps dashboard**: Dedicated cost management view with budget alerts, per-team chargeback, and month-over-month spend tracking across all CI providers
- **Pipeline-as-Code testing**: `pipelinex test` that simulates a pipeline run locally (mocked steps) to validate config changes before pushing — a "unit test for your CI"

---

## How to Contribute

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

**High-impact areas right now:**

- 🔍 Adding antipattern detectors for new CI bottleneck patterns
- 🔌 Expanding CI parser depth (Tekton/Argo/Drone edge cases and enterprise variants)
- 🔐 Pipeline security analysis rules (secret exposure, supply chain risks)
- 📊 Dashboard visualisation improvements
- 📚 Documentation, tutorials, and example configs
- 🧪 Test fixtures for edge cases across all 11 supported CI platforms

Report bugs or request features via [GitHub Issues](https://github.com/mackeh/PipelineX/issues).
