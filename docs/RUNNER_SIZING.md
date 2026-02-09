# Runner Right-Sizing

PipelineX can infer resource pressure from workflow jobs and recommend runner size changes.

## Command

```bash
# Analyze one workflow
pipelinex right-size .github/workflows/ci.yml

# Analyze a directory of workflow files
pipelinex right-size .github/workflows/

# JSON output for automation/reporting
pipelinex right-size .github/workflows/ --format json
```

## What It Uses

The right-sizing engine builds an inferred profile per job:

- CPU pressure (build/test/compile workloads)
- Memory pressure (e2e/integration/docker-heavy patterns)
- I/O pressure (dependency installs, artifact transfer, registry traffic)
- Matrix size and estimated job duration

It then compares:

- Current runner class (`small`, `medium`, `large`, `xlarge`) inferred from `runs-on`
- Recommended class from pressure heuristics

## Interpreting Results

- **UPSCALE** suggests probable under-provisioning (performance risk).
- **DOWNSIZE** suggests probable over-provisioning (cost waste risk).
- Confidence reflects number and strength of profiling signals.

Use recommendations as a starting point, then validate with historical p90 duration and failure rates before enforcing globally.
