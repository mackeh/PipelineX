# Real-World Pipeline Examples

This directory contains before/after examples showing real optimizations performed by PipelineX.

## Node.js Application

**Before**: [`before-node-app.yml`](./before-node-app.yml)
- 31 minutes total runtime
- $248/month in CI costs
- 51.6 developer hours wasted per month

**After**: [`after-node-app.yml`](./after-node-app.yml)
- 6 minutes total runtime (80% improvement)
- $48/month in CI costs (saves $200/month)
- 10 developer hours wasted (saves 41.6 hours/month)

**Annual Savings**: $14,880 ($2,400 compute + $12,480 developer time)

### Key Optimizations

1. **Dependency Caching** (saves 3 min/run)
   - Before: Reinstalling dependencies 5 times per run
   - After: Single install with npm cache

2. **Parallel Execution** (saves 12 min/run)
   - Before: Sequential jobs creating false dependencies
   - After: Jobs run in parallel (lint, build, tests simultaneously)

3. **Smart Test Selection** (saves 6 min/run)
   - Before: Running all 1,200 tests on every commit
   - After: Running only affected tests (typically 15-20%)

4. **Test Sharding** (saves 9 min/run)
   - Before: Single E2E test job taking 12 minutes
   - After: 4 parallel shards, each taking 3 minutes

5. **Docker Layer Caching** (saves 5 min/run)
   - Before: Rebuilding entire image every time (7 min)
   - After: Layer caching reduces to 45 seconds (cache hit)

6. **Shallow Clone** (saves 40 sec/run)
   - Before: Full git history (~2.1 GB repo)
   - After: Shallow clone with `fetch-depth: 1`

7. **Path Filtering** (skips entire runs)
   - Before: Full pipeline on README changes
   - After: Skip CI on docs-only changes

8. **Concurrency Control** (prevents queue pileup)
   - Before: Multiple runs for same PR stacking up
   - After: Cancel outdated runs automatically

## How to Reproduce

1. **Analyze the "before" pipeline:**
   ```bash
   pipelinex analyze examples/real-world/before-node-app.yml
   ```

2. **Generate optimized version:**
   ```bash
   pipelinex optimize examples/real-world/before-node-app.yml \
     --output optimized.yml
   ```

3. **Compare before/after:**
   ```bash
   pipelinex diff examples/real-world/before-node-app.yml \
     examples/real-world/after-node-app.yml
   ```

4. **Estimate cost savings:**
   ```bash
   pipelinex cost examples/real-world/before-node-app.yml \
     --runs-per-month 100 \
     --team-size 10
   ```

## Other Examples Coming Soon

- Python microservices with Docker
- Rust monorepo with cargo workspaces
- Go application with matrix testing
- Ruby on Rails application
- Java/Maven enterprise application

## Contributing Examples

Have a great before/after example? Submit a PR! We'd love to showcase real-world optimizations.

Guidelines:
- Anonymize company-specific details
- Include actual timing data if possible
- Show measurable improvements
- Add comments explaining each optimization
