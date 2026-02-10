# PipelineX v2.1.1 - Polish Release ✨

**Release Date**: February 10, 2026
**Status**: Production Ready
**Breaking Changes**: None

---

## 🔧 Patch Release: v2.1.1

This patch release addresses version synchronization issues across the ecosystem components and resolves dashboard code quality issues identified post-v2.1.0 release.

### Fixed
- **VS Code Extension**: Corrected version mismatch (was `0.1.0`, now aligned to `2.1.1`).
- **Dashboard**: Resolved critical ESLint errors in `lib/pipelinex.ts` and removed unused state/imports in `app/page.tsx`.
- **Documentation**: Updated integration examples and README demos to reflect current version.
- **Installer**: Bumped version in `install.sh` to correctly point to the latest stable release.

---

## 🆕 Recapping v2.1.0 (Released Earlier Today)

**Modern Dashboard Overhaul**
- 🌙 **Dark Mode**: Full system-aware dark theme support.
- 🧊 **Glassmorphism**: Modern translucent UI elements for a professional look.
- 📱 **Responsive Design**: Improved usability across all device types.
- ⚡ **Performance**: Faster rendering of complex DAG visualizations.

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

## 🔄 Upgrade Guide

### To v2.1.1

```bash
cd PipelineX
git pull origin main
cargo build --release
```

---

**Make your pipelines fast. Your future self will thank you.** ⚡

Questions? Open an issue or start a discussion on GitHub!
