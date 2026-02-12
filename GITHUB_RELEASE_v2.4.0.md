# PipelineX v2.4.0 - Expansion Release

**Pipeline optimization now covers 11 CI ecosystems with simulation and explainability built in.**

---

## 🎊 New Release: v2.4.0

This release expands parser coverage and developer workflow tooling with new explain and what-if capabilities.

---

## ✨ What's New

### 1. 🌐 CI Platform Expansion
- Added first-class support for:
  - **Argo Workflows**
  - **Tekton Pipelines**
  - **Drone CI / Woodpecker CI**
- PipelineX now supports **11 CI systems**.

### 2. 🧠 Explainability Command
- New `pipelinex explain` command to generate finding-by-finding guidance.
- Includes impact context and simple remediation steps.
- Supports template mode and optional LLM backends via env keys.

### 3. 🔬 What-If Simulation
- New `pipelinex what-if` command for scenario planning before YAML edits.
- Simulate cache/dependency/runner/duration/job changes.
- Outputs duration, critical-path, and findings delta.

### 4. 🛡️ Robustness Improvements
- Better multi-document YAML handling for Argo and Tekton configs.
- Safer provider auto-detection (avoids false Argo matches on `Cargo.*` paths).
- Monorepo discovery now includes Drone/Woodpecker and common Argo/Tekton folders.

---

## 📦 Installation

### Quick Install
```bash
curl -fsSL https://raw.githubusercontent.com/mackeh/PipelineX/main/install.sh | bash
```

### From Source
```bash
git clone https://github.com/mackeh/PipelineX.git
cd PipelineX
cargo build --release
```

---

## 📝 Full Release Notes

See [RELEASE_NOTES_v2.4.0.md](https://github.com/mackeh/PipelineX/blob/main/RELEASE_NOTES_v2.4.0.md) for full details.
