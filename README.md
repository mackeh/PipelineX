# PipelineX

**Your pipelines are slow. PipelineX knows why — and fixes them automatically.**

PipelineX is an intelligent CI/CD analysis tool that reads your pipeline configurations, identifies exactly where time and money are wasted, and generates optimized configurations. It works offline, requires no account, and supports **GitHub Actions** and **GitLab CI** with more platforms planned.

[![CI](https://github.com/mackeh/PipelineX/actions/workflows/ci.yml/badge.svg)](https://github.com/mackeh/PipelineX/actions/workflows/ci.yml)

## The Problem

The average developer waits 45-90 minutes per day for CI/CD pipelines. Most of that time is wasted on missing caches, serial jobs that could run in parallel, full test suites on single-file changes, and unoptimized Docker builds. PipelineX detects all of this and generates the fix.

## Quick Start

```bash
# Build from source
cargo install --path crates/pipelinex-cli

# Analyze your pipelines (works offline, no account needed)
pipelinex analyze .github/workflows/

# Generate an optimized config
pipelinex optimize .github/workflows/ci.yml -o ci-optimized.yml

# See the diff between current and optimized
pipelinex diff .github/workflows/ci.yml

# Estimate cost savings
pipelinex cost .github/workflows/ci.yml --runs-per-month 500 --team-size 10

# Visualize the pipeline DAG
pipelinex graph .github/workflows/ci.yml

# Run Monte Carlo simulation
pipelinex simulate .github/workflows/ci.yml --runs 1000

# Analyze a Dockerfile
pipelinex docker Dockerfile
pipelinex docker Dockerfile --optimize
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
 |- Max parallelism: 2
 |- Critical path: setup -> lint -> test -> build -> deploy (31:00)
 |- Provider: github-actions

 ============================================================

  CRITICAL  No dependency caching for npm/yarn/pnpm
   | Job 'setup' runs 'npm ci' without caching node_modules.
   | Estimated savings: 2:30/run
   | Confidence: 95% | Auto-fixable
   | Fix: pipelinex optimize --apply cache

  HIGH  'test' depends on 'lint' unnecessarily
   | These jobs share no artifacts — safe to parallelize
   | Estimated savings: 4:27/run

  MEDIUM  No path-based filtering on triggers
   | Full pipeline runs on docs-only changes.

 ============================================================

 Summary
 |- Current est. pipeline time:    31:00
 |- Optimized projection:          6:12
 |- Potential time savings:        80.0%
 |- Findings: 5 critical, 2 high, 3 medium

 Run pipelinex optimize ci.yml to generate optimized config
 Run pipelinex diff ci.yml to see changes
 Run pipelinex simulate ci.yml to simulate timing
 Run pipelinex graph ci.yml to visualize the DAG
```

### Simulation Output

```
$ pipelinex simulate .github/workflows/ci.yml --runs 500

 PipelineX Simulation — Full Stack CI (500 runs)

 Duration Distribution
   Min:     24:19
   p50:     30:53
   p90:     34:14
   p99:     37:00
   Max:     37:52
   Mean:    31:00 (std dev: 2:32)

 Timing Histogram
    29:04 -  29:45 ######################## 41
    29:45 -  30:25 ######################################## 68
    30:25 -  31:06 ############################### 52
    31:06 -  31:47 ############################ 47

 Job Analysis
   Job                      Mean      p50      p90 Crit.Path%
   setup                    3:25     3:26     4:05       100%
   lint                     4:27     4:26     5:18       100%
   test                     8:22     8:25     9:55       100%
   build                   12:33    12:30    15:09        99%
   e2e                      7:53     7:52     9:18         1%
   deploy                   2:12     2:12     2:37       100%
```

### DAG Visualization (Mermaid)

```
$ pipelinex graph .github/workflows/ci.yml

graph LR
    setup["setup\n3:27"] --> lint["lint\n4:27"]
    lint --> test["test\n8:27"]
    test --> e2e["e2e\n7:57"]
    test --> build["build\n12:27"]
    e2e --> deploy["deploy\n2:12"]
    build --> deploy
```

## Commands

| Command | Description |
|---|---|
| `pipelinex analyze <path>` | Analyze pipeline configs for bottlenecks |
| `pipelinex optimize <file>` | Generate an optimized pipeline config |
| `pipelinex diff <file>` | Show diff between current and optimized |
| `pipelinex cost <path>` | Estimate CI/CD costs and potential savings |
| `pipelinex graph <file>` | Generate a visual pipeline DAG diagram |
| `pipelinex simulate <file>` | Run Monte Carlo simulation of pipeline timing |
| `pipelinex docker <file>` | Analyze a Dockerfile for optimization opportunities |

### Options

```
pipelinex analyze [OPTIONS] <PATH>
  -f, --format <FORMAT>   Output format: text, json, sarif [default: text]

pipelinex optimize [OPTIONS] <PATH>
  -o, --output <FILE>     Write optimized config to file
  --diff                  Show diff between original and optimized

pipelinex diff <PATH>
  Shows colored diff between original and optimized config

pipelinex cost [OPTIONS] <PATH>
  --runs-per-month <N>    Estimated pipeline runs per month [default: 500]
  --team-size <N>         Number of developers [default: 10]
  --hourly-rate <RATE>    Fully-loaded developer hourly rate [default: 150]

pipelinex graph [OPTIONS] <PATH>
  -f, --format <FORMAT>   Output format: mermaid, dot, ascii [default: mermaid]
  -o, --output <FILE>     Write graph to file

pipelinex simulate [OPTIONS] <PATH>
  --runs <N>              Number of simulation runs [default: 1000]
  --variance <FACTOR>     Timing variance (0.0-0.3) [default: 0.15]
  -f, --format <FORMAT>   Output format: text, json [default: text]

pipelinex docker [OPTIONS] <PATH>
  --optimize              Output an optimized Dockerfile
  -o, --output <FILE>     Write optimized Dockerfile to file
```

## Architecture

PipelineX works by parsing CI pipeline configs into a unified **Pipeline DAG** (directed acyclic graph), then running a suite of analyzers against it:

```
Workflow YAML ─── Parser ───> Pipeline DAG ───> Analyzers ───> Report
     |                                              |             |
     |                                              v             v
     |                                        Optimizer     SARIF / JSON
     |                                              |
     v                                              v
Dockerfile ──> Docker Analyzer            Optimized YAML/Dockerfile
```

### Core Components

- **Universal Pipeline Parser** — Parses GitHub Actions and GitLab CI YAML into a normalized DAG with jobs, steps, dependencies, and timing estimates
- **Critical Path Analyzer** — Finds the longest path through the DAG (the theoretical minimum pipeline time)
- **Cache Detector** — Identifies dependency install steps lacking cache actions (npm, pip, cargo, gradle/maven, Docker layers)
- **Parallel Finder** — Detects false dependencies between jobs (e.g., tests depending on lint when they share no artifacts)
- **Waste Detector** — Finds missing path filters, full git clones, redundant steps, missing concurrency controls, bloated matrix strategies
- **Cost Estimator** — Translates pipeline inefficiency into dollars using CI provider pricing models
- **Monte Carlo Simulator** — Runs thousands of simulated pipeline executions with timing variance to show p50/p90/p99 distributions
- **DAG Visualizer** — Generates Mermaid, Graphviz DOT, and ASCII diagrams of your pipeline dependency graph
- **Docker Build Optimizer** — Analyzes Dockerfiles for multi-stage build opportunities, cache-busting COPY patterns, bloated base images, and security issues
- **SARIF Output** — Generates SARIF 2.1.0 reports for GitHub Code Scanning and VS Code integration
- **Optimization Engine** — Generates optimized YAML configs with caching, parallelization, shallow clones, path filters, concurrency controls, and smart matrix reduction

### Tech Stack

| Component | Technology |
|---|---|
| Core Engine | Rust |
| DAG Analysis | petgraph |
| YAML Parsing | serde_yaml |
| CLI | clap |
| Diff Output | similar |
| Terminal Colors | colored |

## Project Structure

```
PipelineX/
├── crates/
│   ├── pipelinex-core/              # Core analysis engine (library crate)
│   │   ├── src/
│   │   │   ├── parser/              # Pipeline config parsers
│   │   │   │   ├── dag.rs           # Unified Pipeline DAG data model
│   │   │   │   ├── github.rs        # GitHub Actions parser
│   │   │   │   └── gitlab.rs        # GitLab CI parser
│   │   │   ├── analyzer/            # Bottleneck detection
│   │   │   │   ├── critical_path.rs # Critical path analysis (longest path)
│   │   │   │   ├── cache_detector.rs# Missing dependency cache detection
│   │   │   │   ├── parallel_finder.rs# False dependency & parallelization
│   │   │   │   ├── waste_detector.rs # Waste detection (path filters, etc.)
│   │   │   │   ├── sarif.rs         # SARIF 2.1.0 output for code scanning
│   │   │   │   └── report.rs        # Report data structures
│   │   │   ├── optimizer/           # Config generation
│   │   │   │   ├── cache_gen.rs     # Cache step injection
│   │   │   │   ├── parallel_gen.rs  # Dependency removal
│   │   │   │   ├── shard_gen.rs     # Matrix optimization & test sharding
│   │   │   │   └── docker_opt.rs    # Dockerfile analysis & optimization
│   │   │   ├── simulator/           # Monte Carlo simulation engine
│   │   │   ├── graph/               # DAG visualization (Mermaid, DOT, ASCII)
│   │   │   └── cost/                # CI/CD cost estimation
│   │   └── tests/
│   │       └── integration_tests.rs # 17 integration tests
│   └── pipelinex-cli/               # CLI interface (binary crate)
│       └── src/
│           ├── main.rs              # 7 subcommands
│           └── display.rs           # Terminal output formatting
├── tests/
│   └── fixtures/
│       ├── github-actions/          # 8 GitHub Actions workflow samples
│       ├── gitlab-ci/               # 3 GitLab CI pipeline samples
│       └── dockerfiles/             # 4 Dockerfile samples
├── .github/
│   └── workflows/
│       └── ci.yml                   # PipelineX's own CI pipeline
├── pipelinex-project.md             # Full project specification
├── LICENSE
└── README.md
```

## Roadmap

### Phase 1 (Complete)
- [x] Rust workspace with core engine and CLI
- [x] GitHub Actions YAML parser with Pipeline DAG
- [x] Critical path analysis
- [x] Cache detection (npm, pip, cargo, gradle, maven, Docker)
- [x] Parallelization opportunity finder
- [x] Waste detection (path filters, shallow clones, concurrency, matrix bloat)
- [x] Optimization config generator
- [x] CLI with `analyze`, `optimize`, `diff`, and `cost` commands
- [x] JSON output format

### Phase 2 (Complete)
- [x] GitLab CI parser (stages, needs, parallel keyword, rules, hidden jobs)
- [x] Monte Carlo simulation engine (xorshift64 RNG, per-job stats, histograms)
- [x] Pipeline DAG visualization (Mermaid, Graphviz DOT, ASCII)
- [x] Dockerfile analysis and optimization (multi-stage builds, cache busting, base images)
- [x] Matrix/shard optimizer (combinatorial explosion reduction, smart test sharding)
- [x] SARIF 2.1.0 output for GitHub Code Scanning / IDE integration
- [x] GitHub Actions CI pipeline for PipelineX itself
- [x] 40 tests (23 unit + 17 integration) across all modules
- [x] 15 test fixtures (GitHub Actions, GitLab CI, Dockerfiles)

### Phase 3 (Planned)
- [ ] GitHub API integration for historical run data
- [ ] Flaky test detector
- [ ] Smart test selection
- [ ] GitHub App with automatic PR comments
- [ ] Web dashboard with interactive DAG visualization
- [ ] Slack/Teams weekly digest reports

### Phase 4 (Planned)
- [ ] Jenkins, Bitbucket Pipelines, CircleCI support
- [ ] VS Code extension
- [ ] Community benchmark registry
- [ ] Azure Pipelines, AWS CodePipeline, Buildkite support
- [ ] CI provider migration assistant
- [ ] Plugin system for custom analyzers

## Supported CI Platforms

| Platform | Status |
|---|---|
| GitHub Actions | Supported |
| GitLab CI | Supported |
| Jenkins | Planned |
| Bitbucket Pipelines | Planned |
| CircleCI | Planned |
| Azure Pipelines | Planned |
| AWS CodePipeline | Planned |
| Buildkite | Planned |

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
cargo run -- graph tests/fixtures/github-actions/unoptimized-fullstack.yml
cargo run -- simulate tests/fixtures/github-actions/unoptimized-fullstack.yml
cargo run -- docker tests/fixtures/dockerfiles/unoptimized-node.Dockerfile
```

## License

MIT License. See [LICENSE](LICENSE) for details.
