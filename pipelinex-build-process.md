# PipelineX — Detailed Build Process

> Implementation guide for Roadmap v2.2.x–v3.0.x and Long-Term Vision

---

## Table of Contents

1. [v2.2.x: Usability & Adoption](#v22x-usability--adoption)
2. [v2.3.x: Security & Trust](#v23x-security--trust)
3. [v3.0.x: Woo Factor & Intelligence](#v30x-woo-factor--intelligence)
4. [Long-Term Vision](#long-term-vision)
5. [Infrastructure & DevOps Requirements](#infrastructure--devops-requirements)
6. [Dependency Map](#dependency-map)

---

## v2.2.x: Usability & Adoption

### 2.2.1 — Package Manager Distribution

**Goal:** Install PipelineX without building from source.

**Steps:**

1. **Publish to crates.io** — clean up `Cargo.toml` metadata, run `cargo publish -p pipelinex-cli`
2. **Pre-built binaries** — extend GitHub Actions release workflow with `cross` for 6 platform targets (linux x86/arm, macOS x86/arm, Windows, musl static), upload with SHA-256 checksums
3. **Homebrew tap** — create `mackeh/homebrew-pipelinex`, formula downloads pre-built binary
4. **npm wrapper** — small npm package with `postinstall` binary downloader, enables `npx pipelinex analyze`
5. **Windows** — `winget` manifest and `scoop` bucket
6. **Linux** — `.deb` via `cargo-deb`, `.rpm` via `cargo-rpm`

**Estimated effort:** 2–3 weeks
**Key files:** `.github/workflows/release.yml`, `install.sh`, `npm/`, `homebrew-tap/`

---

### 2.2.2 — `pipelinex init` Wizard

**Goal:** Auto-detect CI platform and generate config.

**Steps:**

1. Add `dialoguer` crate for interactive prompts
2. Auto-detect CI configs by scanning for `.github/workflows/`, `.gitlab-ci.yml`, `Jenkinsfile`, `.circleci/`, `bitbucket-pipelines.yml`, `azure-pipelines.yml`, `buildspec.yml`, `.buildkite/`
3. Prompt for severity threshold, output format, runs-per-month estimate
4. Generate `.pipelinex/config.toml` with selections
5. Generate CI workflow template pre-filled with user choices

**Estimated effort:** 1–2 weeks
**Key files:** `crates/pipelinex-cli/src/commands/init.rs`

---

### 2.2.3 — Watch Mode

**Goal:** Re-analyse pipeline configs on save.

**Steps:**

1. Add `notify` crate for filesystem events
2. Watch known CI config paths (auto-detected)
3. Debounce changes (500ms window)
4. On change: clear terminal, re-run analysis on changed file, print results with timestamp
5. Integrate as VS Code background task via `.vscode/tasks.json`

**Estimated effort:** 1–2 weeks
**Key files:** `crates/pipelinex-cli/src/commands/watch.rs`

---

### 2.2.4 — PR Comment Bot (GitHub Action)

**Goal:** Post inline PR comments with analysis results.

**Steps:**

1. **Create GitHub Action** (`integrations/github-action/`):
   - Trigger on `pull_request` when CI config files change
   - Run PipelineX analysis
   - Post summary comment (findings count, estimated savings, cost impact)
   - Post inline review comments on specific lines with finding details

2. **Comment template:**
   ```markdown
   ## 🔍 PipelineX Analysis
   **3 findings** | Savings: **14 min/run** (~$67/month at 500 runs)
   | Severity | Finding | Savings | Auto-fixable |
   |----------|---------|---------|-------------|
   | 🔴 CRITICAL | No dependency caching | 2:30/run | ✅ |
   | 🟡 HIGH | Serial bottleneck | 8:00/run | ✅ |
   ```

3. **Include expandable optimized config** in `<details>` block
4. **Publish** to GitHub Marketplace

**Estimated effort:** 2–3 weeks
**Key files:** `integrations/github-action/action.yml`, `integrations/github-action/index.ts`

---

### 2.2.5 — Monorepo Support

**Goal:** Analyse all pipeline files across a monorepo.

**Steps:**

1. Add `--monorepo` flag that recursively discovers CI configs up to 5 levels deep
2. Infer package names from directory structure
3. Aggregate findings with per-package cost attribution
4. Output includes `package` field in JSON/SARIF results
5. Dashboard shows per-package breakdown

**Estimated effort:** 2 weeks
**Key files:** `crates/pipelinex-core/src/discovery.rs`

---

### 2.2.6 — Config Linter (`pipelinex lint`)

**Goal:** Validate CI configs for syntax errors and deprecated features.

**Steps:**

1. **YAML syntax validation** — catch malformed YAML before CI provider does
2. **Schema validation** — validate against official CI platform JSON schemas (GitHub Actions, GitLab CI)
3. **Deprecation rules:**
   - `actions/checkout@v2` → suggest v4
   - `ubuntu-latest` → suggest pinned version
   - GitLab `only/except` → suggest `rules`
   - Jenkins scripted → suggest declarative
4. **Typo detection** — fuzzy-match unknown keys against known keys (e.g., `neeed` → `needs`)
5. **Exit codes:** 0 (clean), 1 (warnings), 2 (errors)

**Estimated effort:** 2–3 weeks
**Key files:** `crates/pipelinex-lint/`, `crates/pipelinex-lint/src/rules/`

---

### 2.2.7 — Dashboard Enhancements

**Goal:** Team views, notifications, embeddable widgets.

**Steps:**

1. **Multi-repo dashboard:**
   - Extend DB schema with `repo` and `team` columns
   - API: `GET /api/v1/dashboard/org`, `GET /api/v1/dashboard/team/{team}`
   - Frontend: repo selector, team filter, aggregated charts

2. **Before/after DAG comparison:**
   - Store both original and optimized DAGs in analysis results
   - Split-screen React Flow visualisation with animated transitions
   - Show jobs moving from serial to parallel

3. **Notification system:**
   - Webhook transport (HMAC-signed POST)
   - Slack transport (Block Kit formatted messages)
   - Email transport (SMTP/SES)
   - Trigger on: regression detected (build time >20% above baseline), new critical finding
   - Config in `.pipelinex/config.toml`

4. **Embeddable widgets:**
   - `/embed/{repo}/health` endpoint returning iframe-friendly HTML
   - Minimal build-time trend chart
   - CORS headers for cross-origin embedding

**Estimated effort:** 4–5 weeks
**Key files:** `dashboard/src/views/`, `crates/pipelinex-api/src/`

---

### 2.2.8 — CLI Additions

**Goal:** Markdown output, shell completions, config comparison.

**Steps:**

1. **Markdown output** (`--format markdown`):
   - Clean markdown tables for findings
   - Suitable for pasting into GitHub issues or wiki pages
   - Include summary, findings table, and optimized config in code fences

2. **Shell completions:**
   - Use `clap_complete` crate to generate completions:
     ```rust
     clap_complete::generate(Shell::Bash, &mut cmd, "pipelinex", &mut io::stdout());
     ```
   - Generate for Bash, Zsh, Fish, PowerShell
   - Distribute in release archives

3. **`pipelinex compare <a> <b>`:**
   - Analyse both configs independently
   - Diff findings: new, removed, changed
   - Show estimated time/cost delta
   - Output as table or JSON

**Estimated effort:** 1–2 weeks
**Key files:** `crates/pipelinex-cli/src/output/markdown.rs`, `crates/pipelinex-cli/src/commands/compare.rs`

---

## v2.3.x: Security & Trust

### 2.3.1 — Secret Exposure Detection

**Goal:** Flag hardcoded secrets in CI configs.

**Steps:**

1. **Build pattern library** for CI-specific secrets:
   ```rust
   const SECRET_PATTERNS: &[SecretPattern] = &[
       // API keys in env blocks
       SecretPattern { id: "PLX-SEC-001", regex: r#"(?i)(api[_-]?key|secret|token|password)\s*[:=]\s*['"][^'"]{8,}['"]"#, severity: Critical },
       // AWS keys in run steps
       SecretPattern { id: "PLX-SEC-002", regex: r#"AKIA[0-9A-Z]{16}"#, severity: Critical },
       // GitHub PATs
       SecretPattern { id: "PLX-SEC-003", regex: r#"ghp_[A-Za-z0-9]{36}"#, severity: Critical },
       // Docker login with inline password
       SecretPattern { id: "PLX-SEC-004", regex: r#"docker\s+login.*-p\s+\S+"#, severity: Critical },
       // Base64-encoded secrets
       SecretPattern { id: "PLX-SEC-005", regex: r#"echo\s+[A-Za-z0-9+/=]{40,}\s*\|\s*base64"#, severity: High },
   ];
   ```

2. **Shannon entropy analysis** for generic high-entropy string detection near sensitive variable names
3. **Integration:** Run as part of `pipelinex analyze` by default, suppressible with `--no-secrets`
4. **Output:** Findings include redacted secret preview (first 4 chars + `****`)

**Estimated effort:** 1–2 weeks
**Key files:** `crates/pipelinex-security/src/secrets.rs`

---

### 2.3.2 — Overprivileged Permissions Audit

**Goal:** Detect overly broad permissions in GitHub Actions.

**Steps:**

1. **Parse `permissions` block** in GitHub Actions workflows
2. **Detection rules:**
   - `permissions: write-all` → Critical: reduce to minimum required
   - `permissions: { contents: write }` when only `read` is needed → High
   - Missing `permissions` block entirely → Medium (defaults to broad)
   - `GITHUB_TOKEN` passed to third-party actions → Medium
3. **Suggest minimal permissions** based on actions used:
   ```rust
   fn suggest_permissions(steps: &[Step]) -> Permissions {
       let mut perms = Permissions::default();
       for step in steps {
           match step.uses.as_deref() {
               Some("actions/checkout@v4") => perms.contents = Permission::Read,
               Some("github/codeql-action/upload-sarif@v3") => perms.security_events = Permission::Write,
               Some(s) if s.contains("create-release") => perms.contents = Permission::Write,
               _ => {}
           }
       }
       perms
   }
   ```
4. **Output:** Include suggested `permissions` block in findings

**Estimated effort:** 1–2 weeks
**Key files:** `crates/pipelinex-security/src/permissions.rs`

---

### 2.3.3 — Supply Chain Risk Scoring

**Goal:** Assess third-party actions/orbs/images for risk.

**Steps:**

1. **Extract all third-party references:**
   - GitHub Actions: `uses: owner/action@ref`
   - GitLab CI: `image:`, `include:`
   - CircleCI: `orbs:`
   - Docker: `FROM` in referenced Dockerfiles

2. **Risk scoring criteria:**
   ```rust
   struct SupplyChainRisk {
       pinning: PinningRisk,       // SHA vs tag vs branch vs "latest"
       popularity: PopularityRisk, // stars, downloads, known publisher
       maintenance: MaintenanceRisk, // last commit, open issues
       known_vulns: bool,          // known compromises (e.g., tj-actions/changed-files)
   }
   
   enum PinningRisk {
       SHA,      // ✅ Pinned to full SHA — minimal risk
       Tag,      // ⚠️ Tag can be moved — medium risk
       Branch,   // 🔴 Branch ref — high risk
       Latest,   // 🔴 No version — critical risk
   }
   ```

3. **GitHub API queries** (optional, with token):
   - Repo metadata (stars, last commit, archived status)
   - Known security advisories
   - Verify SHA exists on the expected tag

4. **Offline mode** — risk score based on pinning practice alone (no API calls)
5. **Output:** Per-action risk summary with remediation (e.g., "Pin `actions/checkout@v4` to SHA `abc123...`")

**Estimated effort:** 2–3 weeks
**Key files:** `crates/pipelinex-security/src/supply_chain.rs`

---

### 2.3.4 — Untrusted Input Injection Detection

**Goal:** Detect GitHub Actions expression injection vulnerabilities.

**Steps:**

1. **Catalogue dangerous contexts:**
   ```rust
   const DANGEROUS_CONTEXTS: &[&str] = &[
       "github.event.issue.title",
       "github.event.issue.body",
       "github.event.pull_request.title",
       "github.event.pull_request.body",
       "github.event.comment.body",
       "github.event.review.body",
       "github.event.head_commit.message",
       "github.head_ref",
   ];
   ```

2. **Detect injection patterns:**
   ```rust
   // Pattern: dangerous context used directly in `run:` step
   fn check_injection(step: &Step) -> Option<Finding> {
       if let Some(run) = &step.run {
           for ctx in DANGEROUS_CONTEXTS {
               if run.contains(&format!("${{{{ {} }}}}", ctx)) {
                   return Some(Finding {
                       id: "PLX-SEC-INJ-001",
                       severity: Critical,
                       message: format!("{} is used directly in a `run:` step — attacker-controlled input", ctx),
                       fix: "Use an intermediate environment variable or input validation",
                   });
               }
           }
       }
       None
   }
   ```

3. **Safe pattern detection** — don't flag when context is used in `if:` conditions (not injectable) or assigned to an env var first
4. **Cover all 8 platforms** where applicable (GitHub Actions is primary)

**Estimated effort:** 1–2 weeks
**Key files:** `crates/pipelinex-security/src/injection.rs`

---

### 2.3.5 — Compliance Policies

**Goal:** Define and enforce organisational CI security rules.

**Steps:**

1. **Policy file format** (`.pipelinex/policy.toml`):
   ```toml
   [rules]
   # All actions must be pinned by SHA
   require_sha_pinning = true
   
   # No workflows may use ubuntu-latest
   banned_runners = ["ubuntu-latest", "windows-latest"]
   
   # Cache must be configured for package managers
   require_cache = ["npm", "yarn", "pip", "cargo"]
   
   # Maximum allowed pipeline duration
   max_duration_minutes = 30
   
   # All workflows must have explicit permissions
   require_permissions_block = true
   ```

2. **`pipelinex policy check`:**
   - Load policy file
   - Run all policy rules against detected CI configs
   - Report violations with references to the specific policy rule
   - Exit code 1 on any violation

3. **Pre-built policy packs:**
   - `strict-security.toml` — maximum security
   - `cost-optimized.toml` — focus on efficiency
   - `balanced.toml` — security + performance

4. **CI integration:** Run `pipelinex policy check` as a CI step

**Estimated effort:** 2–3 weeks
**Key files:** `crates/pipelinex-policy/`, `.pipelinex/policy.toml`

---

### 2.3.6 — Signed Reports & SBOM

**Goal:** Tamper-proof reports and CI bill of materials.

**Steps:**

1. **Signed reports:**
   - Generate Ed25519 keypair: `pipelinex keys generate`
   - Sign JSON/SARIF output with Ed25519 (`ed25519-dalek` crate)
   - Embed signature in output metadata
   - Verify: `pipelinex verify report.json --key public.pem`

2. **CI SBOM:**
   - Parse all actions, images, orbs, and tool versions from CI configs
   - Generate CycloneDX JSON listing every component:
     ```json
     {
       "bomFormat": "CycloneDX",
       "components": [
         { "type": "application", "name": "actions/checkout", "version": "v4", "purl": "pkg:github/actions/checkout@v4" },
         { "type": "container", "name": "node", "version": "20-slim" }
       ]
     }
     ```
   - Command: `pipelinex sbom .github/workflows/ > ci-sbom.json`

3. **RBAC for dashboard:**
   - Add user model with roles (admin, editor, viewer)
   - JWT-based authentication
   - OIDC/SAML SSO integration
   - Role-based API middleware

**Estimated effort:** 3–4 weeks
**Key files:** `crates/pipelinex-cli/src/commands/keys.rs`, `crates/pipelinex-sbom/`

---

### 2.3.7 — Offline Mode & Redacted Reports

**Goal:** Support air-gapped environments and safe external sharing.

**Steps:**

1. **Offline mode:**
   - `--offline` flag disables all network calls
   - No GitHub API queries, no telemetry, no update checks
   - Supply chain scoring uses pinning-only heuristics
   - Document as default for regulated environments

2. **Redacted reports:**
   - `--redact` flag strips sensitive values:
     - Repo names → `repo-1`, `repo-2`
     - Secret names → `SECRET_***`
     - Internal URLs → `https://internal/***`
     - File paths → relative only, no absolute
   - Useful for sharing reports with external auditors or vendors

**Estimated effort:** 1 week
**Key files:** `crates/pipelinex-cli/src/output/redact.rs`

---

## v3.0.x: Woo Factor & Intelligence

### 3.0.1 — LLM-Powered Optimisation Explanations

**Goal:** Natural language explanations of findings and fixes.

**Steps:**

1. **LLM integration module:**
   ```rust
   pub struct LLMExplainer {
       provider: LLMProvider, // Anthropic, OpenAI, or local
       model: String,
   }
   
   impl LLMExplainer {
       pub async fn explain(&self, finding: &Finding, context: &PipelineContext) -> String {
           let prompt = format!(
               "Explain this CI/CD finding to a developer in 2-3 sentences:\n\
                Finding: {}\n\
                Severity: {}\n\
                Estimated savings: {}\n\
                Context: {} jobs, {} steps, provider: {}\n\
                Include: why it matters, what it costs, and the simplest fix.",
               finding.message, finding.severity, finding.savings,
               context.job_count, context.step_count, context.provider
           );
           self.call_llm(&prompt).await
       }
   }
   ```

2. **Configuration:**
   ```toml
   [ai]
   provider = "anthropic"
   model = "claude-sonnet-4-20250514"
   api_key_env = "ANTHROPIC_API_KEY"
   ```

3. **CLI usage:** `pipelinex analyze ci.yml --explain`
4. **Fallback:** If no API key configured, use template-based explanations (no LLM)

**Estimated effort:** 2 weeks
**Key files:** `crates/pipelinex-ai/src/explainer.rs`

---

### 3.0.2 — Predictive Build Time

**Goal:** Predict CI duration for a PR before it runs.

**Steps:**

1. **Collect training data:**
   - Use `pipelinex history` to gather historical run data
   - Features: changed files count, file types, test count, branch, day of week, cache hit rate
   - Target: total pipeline duration

2. **Train a simple model:**
   - Start with linear regression or gradient boosted trees
   - Use `smartcore` Rust crate for in-process ML:
     ```rust
     use smartcore::linear::linear_regression::LinearRegression;
     
     let model = LinearRegression::fit(&x_train, &y_train, Default::default())?;
     let prediction = model.predict(&x_new)?;
     ```
   - Store trained model as serialized binary in `.pipelinex/model.bin`

3. **CLI usage:**
   ```
   $ pipelinex predict --diff HEAD~1 HEAD
   
   🔮 Predicted build time: 12 min (±3 min)
   Baseline: 31 min | This PR touches 3 test files
   Confidence: 78%
   ```

4. **PR integration:** Include prediction in PR comment bot output

**Estimated effort:** 3–4 weeks
**Key files:** `crates/pipelinex-predict/`

---

### 3.0.3 — Pipeline Health Score Badge

**Goal:** Embeddable badge for READMEs.

**Steps:**

1. **Scoring algorithm:**
   ```rust
   fn health_score(analysis: &AnalysisResult) -> (u8, &str) {
       let base = 100;
       let deductions = analysis.findings.iter().map(|f| match f.severity {
           Critical => 25,
           High => 10,
           Medium => 3,
           Low => 1,
       }).sum::<u8>();
       
       let score = base.saturating_sub(deductions);
       let pct_optimized = (analysis.total_savings.as_secs_f64() / analysis.total_duration.as_secs_f64() * 100.0) as u8;
       
       let grade = match score {
           95..=100 => "A+", 85..=94 => "A", 70..=84 => "B",
           50..=69 => "C", 25..=49 => "D", _ => "F",
       };
       (score, grade)
   }
   ```

2. **CLI:** `pipelinex badge ci.yml` → outputs markdown badge syntax
3. **API endpoint:** `GET /api/v1/badge/{repo}` → shields.io redirect
4. **Include optimization percentage:** `PipelineX: A+ | 94% optimized`

**Estimated effort:** 1 week
**Key files:** `crates/pipelinex-cli/src/commands/badge.rs`

---

### 3.0.4 — Live Pipeline Monitor

**Goal:** Real-time dashboard for active CI runs.

**Steps:**

1. **Data source:** GitHub Actions API (or equivalent per platform):
   ```
   GET /repos/{owner}/{repo}/actions/runs?status=in_progress
   GET /repos/{owner}/{repo}/actions/runs/{run_id}/jobs
   ```

2. **WebSocket streaming** from API server to dashboard frontend

3. **Dashboard view:**
   - Active runs as cards with progress bars
   - Per-job status (queued, running, completed)
   - Live duration counter
   - Instant bottleneck highlighting (current step is the bottleneck if it's the longest)
   - Historical comparison overlay (is this run slower than usual?)

4. **Polling interval:** 10 seconds for active runs, 60 seconds for idle

**Estimated effort:** 3–4 weeks
**Key files:** `crates/pipelinex-api/src/routes/live.rs`, `dashboard/src/views/LiveMonitor.tsx`

---

### 3.0.5 — Interactive What-If Simulator

**Goal:** Browser-based DAG editor to explore optimisation impact.

**Steps:**

1. **Frontend application** (React + React Flow):
   - Load pipeline DAG from analysis results
   - Drag-and-drop job reordering
   - Toggle dependencies (add/remove `needs:` edges)
   - Toggle caching on/off per job
   - Enable/disable path filtering per job
   - Matrix strategy editor

2. **Real-time cost/time recalculation:**
   - On each change, recalculate critical path and estimated duration
   - Show delta from original: `Original: 31 min → Modified: 14 min (55% faster)`
   - Update cost estimate: `$156/month → $70/month`

3. **Export modified config:**
   - "Generate YAML" button produces the optimized CI config matching the visual layout
   - Copy-to-clipboard or download

4. **No backend required** — all calculations run client-side using WASM or JS reimplementation of the scheduling algorithm

**Estimated effort:** 4–6 weeks
**Key files:** `dashboard/src/views/Simulator.tsx`, `dashboard/src/lib/scheduler.ts`

---

### 3.0.6 — Cost Leaderboard

**Goal:** Org-wide ranking by CI cost efficiency.

**Steps:**

1. **Data aggregation:**
   - Collect cost data per repo from analysis history
   - Calculate: actual cost, potential savings, optimisation adoption rate

2. **Leaderboard API:**
   ```
   GET /api/v1/leaderboard?org={org}&period=month
   → [
       { repo: "backend", team: "Platform", savings_applied: "$2,400", remaining: "$200", score: 92 },
       { repo: "frontend", team: "Product", savings_applied: "$300", remaining: "$890", score: 41 },
     ]
   ```

3. **Dashboard view:**
   - Ranked table with team/repo, savings achieved, potential remaining
   - Trend arrows (improving/declining)
   - "Apply suggestions" CTA for repos with high potential savings

4. **Gamification:** Monthly email digest with top improvers

**Estimated effort:** 2–3 weeks
**Key files:** `dashboard/src/views/Leaderboard.tsx`, `crates/pipelinex-api/src/routes/leaderboard.rs`

---

### 3.0.7 — Online Playground (WASM)

**Goal:** Browser-based CI config analyser.

**Steps:**

1. **Compile core analyser to WASM:**
   - Create `crates/pipelinex-wasm` crate
   - Expose: `pub fn analyze(yaml: &str, platform: &str) -> String`
   - Use `wasm-bindgen` for JS interop
   - Strip file I/O and network features with `#[cfg(not(target_arch = "wasm32"))]`
   - Build: `wasm-pack build --target web`

2. **Frontend:**
   - Code editor (Monaco/CodeMirror) with YAML syntax highlighting
   - Platform selector dropdown
   - "Analyze" button
   - Results panel: findings, DAG visualization, cost estimate
   - "Optimize" button generates fixed YAML

3. **Shareable URLs:** Encode config in URL hash (compressed)
4. **Host** on GitHub Pages or Vercel

**Estimated effort:** 3–4 weeks
**Key files:** `crates/pipelinex-wasm/`, `playground/`

---

### 3.0.8 — Ecosystem Expansion

**Goal:** More CI platforms, MCP, GitHub Marketplace.

**Steps:**

1. **Tekton parser:**
   - Parse `Task`, `Pipeline`, `PipelineRun` CRDs
   - Map to internal DAG representation
   - Apply existing antipattern detectors

2. **Argo Workflows parser:**
   - Parse `Workflow` and `WorkflowTemplate` CRDs
   - Handle DAG and Steps templates

3. **Drone CI / Woodpecker CI parser:**
   - Parse `.drone.yml` / `.woodpecker.yml`
   - Simpler YAML format, faster to implement

4. **MCP Server:**
   ```rust
   // Expose PipelineX as MCP tools
   let tools = vec![
       MCPTool { name: "pipelinex_analyze", description: "Analyze a CI config", handler: handle_analyze },
       MCPTool { name: "pipelinex_optimize", description: "Generate optimized config", handler: handle_optimize },
       MCPTool { name: "pipelinex_cost", description: "Estimate CI costs", handler: handle_cost },
   ];
   ```
   - Transport: stdio (Claude Code, Cursor) and SSE (web)
   - Command: `pipelinex mcp-server`

5. **GitHub Marketplace App:**
   - Register GitHub App with `pull_requests: write`, `checks: write`
   - Webhook handler for `pull_request` events
   - Auto-analyse PRs that touch CI configs
   - One-click install from marketplace.github.com

**Estimated effort:** 6–8 weeks (all platforms combined)
**Key files:** `crates/pipelinex-parsers/src/tekton.rs`, `crates/pipelinex-parsers/src/argo.rs`, `crates/pipelinex-mcp/`

---

## Long-Term Vision

### Automatic PR Generation
- When findings are auto-fixable, open a PR with the optimized config
- Include analysis summary, before/after DAG, and estimated savings in PR description
- Use GitHub API: `POST /repos/{owner}/{repo}/pulls`
- **Effort:** 2–3 weeks

### PipelineX Cloud (SaaS)
- Multi-tenant API with org/team hierarchy
- Historical analytics with unlimited retention
- SSO (SAML/OIDC), RBAC, audit logging
- Managed dashboards — no self-hosting
- **Effort:** 3–6 months (separate project)

### CI Provider Cost API Integration
- Pull actual billing data from GitHub Actions, GitLab CI, CircleCI, Buildkite
- Show real cost savings (not estimates)
- Requires OAuth integration per provider
- **Effort:** 4–6 weeks

### Pipeline-as-Code Testing
- `pipelinex test` simulates a pipeline run locally
- Mocked steps, validated dependencies, estimated timing
- Catches config errors before pushing
- **Effort:** 6–8 weeks

---

## Infrastructure & DevOps Requirements

| Component | Technology | Purpose |
|-----------|-----------|---------|
| Build/Release | GitHub Actions, `cross` | Cross-platform binary builds |
| WASM | `wasm-pack`, `wasm-bindgen` | Playground and browser tools |
| Package registries | crates.io, npm, Homebrew | Distribution |
| Dashboard | React, React Flow, D3.js | Visualisation |
| API server | Rust (actix-web or axum) | REST API, WebSocket |
| LLM integration | Anthropic/OpenAI API | AI explanations |
| ML | `smartcore` crate | Predictive build time |
| Deployment | Docker, Helm, Vercel | Self-hosted and cloud |

---

## Dependency Map

```
2.2.1 (Distribution) ──→ 3.0.7 (Playground uses WASM build)
2.2.2 (Init wizard) ──→ 2.2.6 (Lint integrated into init validation)
2.2.4 (PR Bot) ──→ 3.0.3 (Badge included in PR comments)
               ──→ 3.0.2 (Prediction included in PR comments)

2.3.1 (Secrets) ─┐
2.3.2 (Perms)   ─┤──→ 2.3.5 (Policy enforces all security rules)
2.3.3 (Supply)  ─┤
2.3.4 (Injection)─┘

2.3.6 (Signed reports) — independent
2.3.6 (SBOM) ──→ reuses supply chain parsing from 2.3.3

3.0.1 (LLM explain) — independent, needs API key
3.0.4 (Live monitor) ──→ 3.0.6 (Leaderboard uses same data)
3.0.5 (Simulator) — independent frontend work
3.0.8 (MCP) — independent, can be built anytime
```
