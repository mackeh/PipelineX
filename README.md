# PipelineX

**Your pipelines are slow. PipelineX knows why — and fixes them automatically.**

PipelineX is an intelligent CI/CD analysis tool that reads your pipeline configurations, identifies exactly where time and money are wasted, and generates optimized configurations. It works offline, requires no account, and currently supports GitHub Actions with more CI platforms planned.

## The Problem

The average developer waits 45-90 minutes per day for CI/CD pipelines. Most of that time is wasted on missing caches, serial jobs that could run in parallel, full test suites on single-file changes, and unoptimized Docker builds. PipelineX detects all of this and generates the fix.

## Quick Start

```bash
# Build from source
cargo install --path crates/pipelinex-cli

# Analyze your pipelines (works offline, no account needed)
pipelinex analyze .github/workflows/

# Generate an optimized config
pipelinex optimize .github/workflows/ci.yml --output ci-optimized.yml

# See the diff between current and optimized
pipelinex optimize .github/workflows/ci.yml --diff

# Estimate cost savings
pipelinex cost .github/workflows/ci.yml --runs-per-month 500 --team-size 10
```

## What It Detects

PipelineX identifies **12 pipeline antipatterns** that cause slow, expensive CI/CD:

| # | Antipattern | Typical Waste |
|---|---|---|
| 1 | Missing dependency caching (npm, pip, cargo, gradle) | 2-8 min/run |
| 2 | Serial jobs that could run in parallel | 5-20 min/run |
| 3 | Running all tests on every commit | 3-30 min/run |
| 4 | No Docker layer caching | 3-12 min/run |
| 5 | Redundant checkout/setup steps across jobs | 1-3 min/run |
| 6 | Flaky tests causing retries | 5-15 min/run |
| 7 | Over-provisioned or under-provisioned runners | $$ waste |
| 8 | No build artifact reuse between jobs | 2-8 min/run |
| 9 | Unnecessary full git clones | 30s-3 min/run |
| 10 | Missing concurrency controls | Queue pileup |
| 11 | Unoptimized matrix strategies | 10-40 min/run |
| 12 | No path-based filtering (full pipeline on docs changes) | Full run wasted |

## Example Output

```
$ pipelinex analyze .github/workflows/ci.yml

 PipelineX v0.1.0 — Analyzing ci.yml

 Pipeline Structure
 |- 6 jobs, 23 steps
 |- Max parallelism: 2 (could be 4)
 |- Critical path: setup -> lint -> test -> e2e -> deploy (31:00)

 ============================================================

  CRITICAL  No dependency caching for npm/yarn/pnpm
   | Job 'setup' runs 'npm ci' without caching node_modules.
   | Estimated savings: 2:30/run
   | Fix: pipelinex optimize --apply cache

  HIGH  'test' depends on 'lint' unnecessarily
   | These jobs share no artifacts — safe to parallelize
   | Estimated savings: 4:27/run

  HIGH  Docker build has no layer caching
   | Every build starts from scratch.
   | Estimated savings: 4:00/run

  MEDIUM  No path-based filtering on triggers
   | Full pipeline runs on docs-only changes.

 ============================================================

 Summary
 |- Current est. pipeline time:    31:00
 |- Optimized projection:          6:12
 |- Potential time savings:        80.0%
```

## Commands

| Command | Description |
|---|---|
| `pipelinex analyze <path>` | Analyze pipeline configs for bottlenecks |
| `pipelinex optimize <file>` | Generate an optimized pipeline config |
| `pipelinex optimize <file> --diff` | Show diff between current and optimized |
| `pipelinex cost <path>` | Estimate CI/CD costs and potential savings |

### Options

```
pipelinex analyze [OPTIONS] <PATH>
  -f, --format <FORMAT>   Output format: text, json [default: text]

pipelinex optimize [OPTIONS] <PATH>
  -o, --output <FILE>     Write optimized config to file
  --diff                  Show diff between original and optimized

pipelinex cost [OPTIONS] <PATH>
  --runs-per-month <N>    Estimated pipeline runs per month [default: 500]
  --team-size <N>         Number of developers [default: 10]
  --hourly-rate <RATE>    Fully-loaded developer hourly rate [default: 150]
```

## Architecture

PipelineX works by parsing CI pipeline configs into a unified **Pipeline DAG** (directed acyclic graph), then running a series of analyzers against it:

```
Workflow YAML --> Parser --> Pipeline DAG --> Analyzers --> Report
                                                |
                                                v
                                          Optimizer --> Optimized YAML
```

### Core Components

- **Universal Pipeline Parser** — Parses GitHub Actions YAML into a normalized DAG with jobs, steps, dependencies, and timing estimates
- **Critical Path Analyzer** — Finds the longest path through the DAG (the theoretical minimum pipeline time)
- **Cache Detector** — Identifies dependency install steps lacking cache actions (npm, pip, cargo, gradle/maven, Docker layers)
- **Parallel Finder** — Detects false dependencies between jobs (e.g., tests depending on lint when they share no artifacts)
- **Waste Detector** — Finds missing path filters, full git clones, redundant steps, missing concurrency controls, bloated matrix strategies
- **Cost Estimator** — Translates pipeline inefficiency into dollars using CI provider pricing models
- **Optimization Engine** — Generates optimized YAML configs with caching, parallelization, shallow clones, path filters, and concurrency controls

### Tech Stack

| Component | Technology |
|---|---|
| Core Engine | Rust |
| DAG Analysis | petgraph |
| YAML Parsing | serde_yaml |
| CLI | clap |
| Diff Output | similar |

## Project Structure

```
pipelinex/
├── crates/
│   ├── pipelinex-core/           # Core analysis engine
│   │   └── src/
│   │       ├── parser/           # Pipeline config parsers
│   │       │   ├── dag.rs        # Unified Pipeline DAG data model
│   │       │   └── github.rs     # GitHub Actions YAML parser
│   │       ├── analyzer/         # Bottleneck detection
│   │       │   ├── critical_path.rs
│   │       │   ├── cache_detector.rs
│   │       │   ├── parallel_finder.rs
│   │       │   ├── waste_detector.rs
│   │       │   └── report.rs
│   │       ├── optimizer/        # Config generation
│   │       │   ├── cache_gen.rs
│   │       │   └── parallel_gen.rs
│   │       └── cost/             # Cost estimation
│   └── pipelinex-cli/            # CLI interface
│       └── src/
│           ├── main.rs
│           └── display.rs        # Terminal output formatting
├── tests/
│   └── fixtures/
│       └── github-actions/       # Sample workflow files
├── pipelinex-project.md          # Full project specification
├── LICENSE
└── README.md
```

## Roadmap

### Phase 1 (Current)
- [x] Rust workspace with core engine and CLI
- [x] GitHub Actions YAML parser with Pipeline DAG
- [x] Critical path analysis
- [x] Cache detection (npm, pip, cargo, gradle, maven, Docker)
- [x] Parallelization opportunity finder
- [x] Waste detection (path filters, shallow clones, concurrency, matrix bloat)
- [x] Optimization config generator
- [x] CLI with `analyze`, `optimize`, `diff`, and `cost` commands
- [x] 9 unit tests across all analyzers

### Phase 2 (Planned)
- [ ] GitLab CI and Jenkins parsers
- [ ] GitHub API integration for historical run data
- [ ] Monte Carlo simulation engine
- [ ] Flaky test detector
- [ ] Smart test selection
- [ ] Docker build optimizer (Dockerfile analysis)

### Phase 3 (Planned)
- [ ] GitHub App with automatic PR comments
- [ ] Web dashboard with interactive DAG visualization
- [ ] Slack/Teams weekly digest reports
- [ ] Bitbucket Pipelines and CircleCI support

### Phase 4 (Planned)
- [ ] VS Code extension
- [ ] Community benchmark registry
- [ ] Azure Pipelines, AWS CodePipeline, Buildkite support
- [ ] CI provider migration assistant
- [ ] Plugin system for custom analyzers

## Supported CI Platforms

| Platform | Status |
|---|---|
| GitHub Actions | Supported |
| GitLab CI | Planned (Phase 2) |
| Jenkins | Planned (Phase 2) |
| Bitbucket Pipelines | Planned (Phase 3) |
| CircleCI | Planned (Phase 3) |
| Azure Pipelines | Planned (Phase 4) |
| AWS CodePipeline | Planned (Phase 4) |
| Buildkite | Planned (Phase 4) |

## Contributing

Contributions are welcome! See the [project specification](pipelinex-project.md) for the full vision and architecture details.

```bash
# Clone and build
git clone https://github.com/mackeh/PipelineX.git
cd PipelineX
cargo build

# Run tests
cargo test

# Try it on the included fixtures
cargo run -- analyze tests/fixtures/github-actions/unoptimized-fullstack.yml
```

## License

MIT License. See [LICENSE](LICENSE) for details.
