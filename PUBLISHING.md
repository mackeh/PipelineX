# Publishing Guide for PipelineX v1.0.0

This document outlines the steps to publish PipelineX v1.0.0 to crates.io and create a GitHub release.

## Pre-Publishing Checklist

- [x] All tests passing (`cargo test --workspace`)
- [x] Clippy clean (`cargo clippy --workspace -- -D warnings`)
- [x] Formatted (`cargo fmt --all -- --check`)
- [x] Version updated to 1.0.0 in `Cargo.toml`
- [x] CHANGELOG.md created
- [x] LICENSE file exists
- [x] README.md updated
- [x] Example pipelines created
- [x] All commits pushed to GitHub

## Step 1: Verify Package Configuration

```bash
# Check that package metadata is correct
cargo package --list -p pipelinex-core
cargo package --list -p pipelinex-cli

# Verify package builds correctly
cargo package --no-verify -p pipelinex-core
cargo package --no-verify -p pipelinex-cli
```

## Step 2: Build Release Binaries

Build optimized binaries for multiple platforms:

### Linux (x86_64)
```bash
cargo build --release --target x86_64-unknown-linux-gnu
cp target/x86_64-unknown-linux-gnu/release/pipelinex pipelinex-linux-x86_64
tar -czf pipelinex-v1.0.0-linux-x86_64.tar.gz pipelinex-linux-x86_64
```

### macOS (Intel)
```bash
cargo build --release --target x86_64-apple-darwin
cp target/x86_64-apple-darwin/release/pipelinex pipelinex-macos-x86_64
tar -czf pipelinex-v1.0.0-macos-x86_64.tar.gz pipelinex-macos-x86_64
```

### macOS (Apple Silicon)
```bash
cargo build --release --target aarch64-apple-darwin
cp target/aarch64-apple-darwin/release/pipelinex pipelinex-macos-aarch64
tar -czf pipelinex-v1.0.0-macos-aarch64.tar.gz pipelinex-macos-aarch64
```

### Windows
```bash
cargo build --release --target x86_64-pc-windows-msvc
cp target/x86_64-pc-windows-msvc/release/pipelinex.exe pipelinex-windows-x86_64.exe
zip pipelinex-v1.0.0-windows-x86_64.zip pipelinex-windows-x86_64.exe
```

**Note**: Cross-compilation may require additional setup. Alternatively, use GitHub Actions to build binaries for all platforms.

## Step 3: Publish to crates.io

### First-time Setup
```bash
# Login to crates.io (you'll need an API token from crates.io/me)
cargo login
```

### Publish Packages
```bash
# Publish core library first (cli depends on it)
cd crates/pipelinex-core
cargo publish

# Wait a few moments for crates.io to index the package
sleep 30

# Publish CLI tool
cd ../pipelinex-cli
cargo publish
```

## Step 4: Create Git Tag

```bash
# Create annotated tag
git tag -a v1.0.0 -m "Release v1.0.0: Production-ready CI/CD analyzer

PipelineX v1.0.0 is the first production-ready release featuring:
- Multi-platform support (GitHub Actions, GitLab CI, Jenkins, CircleCI, Bitbucket)
- 12 antipattern detectors
- 10 CLI commands
- Pipeline Health Score system
- GitHub API integration for historical analysis
- Proven 50-85% pipeline time reduction

Full release notes: https://github.com/mackeh/PipelineX/blob/main/RELEASE_NOTES.md"

# Push tag to GitHub
git push origin v1.0.0
```

## Step 5: Create GitHub Release

### Using GitHub CLI
```bash
gh release create v1.0.0 \
  --title "PipelineX v1.0.0 - Production Ready 🚀" \
  --notes-file RELEASE_NOTES.md \
  --latest \
  pipelinex-v1.0.0-linux-x86_64.tar.gz \
  pipelinex-v1.0.0-macos-x86_64.tar.gz \
  pipelinex-v1.0.0-macos-aarch64.tar.gz \
  pipelinex-v1.0.0-windows-x86_64.zip
```

### Using GitHub Web UI
1. Go to https://github.com/mackeh/PipelineX/releases/new
2. Choose tag: `v1.0.0`
3. Release title: `PipelineX v1.0.0 - Production Ready 🚀`
4. Description: Copy content from `RELEASE_NOTES.md`
5. Upload binary artifacts:
   - `pipelinex-v1.0.0-linux-x86_64.tar.gz`
   - `pipelinex-v1.0.0-macos-x86_64.tar.gz`
   - `pipelinex-v1.0.0-macos-aarch64.tar.gz`
   - `pipelinex-v1.0.0-windows-x86_64.zip`
6. Check "Set as the latest release"
7. Click "Publish release"

## Step 6: Update Documentation

After release:
- [ ] Update README with crates.io badge
- [ ] Update installation instructions to reference v1.0.0
- [ ] Announce release on social media
- [ ] Post to relevant communities (Reddit r/rust, r/devops, Hacker News, dev.to)

## Automated Release Process (Future)

Consider setting up GitHub Actions to automate releases:

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: pipelinex-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/pipelinex*

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
      - uses: softprops/action-gh-release@v1
        with:
          files: pipelinex-*/*
          body_path: RELEASE_NOTES.md
```

## Verification

After publishing, verify:

```bash
# Check crates.io listing
open https://crates.io/crates/pipelinex-cli

# Test installation from crates.io
cargo install pipelinex-cli

# Verify installed version
pipelinex --version
# Should output: pipelinex 1.0.0

# Run smoke test
pipelinex analyze examples/real-world/before-node-app.yml
```

## Troubleshooting

### "Package already published"
- Cannot republish the same version
- Increment version and try again

### "Failed to publish: 403 Forbidden"
- Check that you're logged in: `cargo login`
- Verify you have permissions (if publishing to an organization)

### "Dependency not found"
- Make sure pipelinex-core is published before pipelinex-cli
- Wait a few moments for crates.io to index new packages

### GitHub Release Upload Failed
- Check file sizes (GitHub has limits)
- Ensure binary names don't contain invalid characters
- Try uploading via CLI instead of web UI

## Post-Release

1. **Monitor Issues**: Watch for any installation or usage problems
2. **Update docs.rs**: Check that documentation builds correctly
3. **Announce**: Share release on social media, forums, etc.
4. **Collect Feedback**: Engage with early adopters
5. **Plan Next Release**: Start roadmap for v1.1.0

---

**Congratulations on shipping v1.0.0! 🎉**
