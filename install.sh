#!/bin/bash
set -e

# PipelineX Installation Script
# Usage: curl -fsSL https://raw.githubusercontent.com/mackeh/PipelineX/main/install.sh | bash

REPO="mackeh/PipelineX"
BIN_NAME="pipelinex"
INSTALL_DIR="${PIPELINEX_INSTALL_DIR:-/usr/local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔═══════════════════════════════════════╗${NC}"
echo -e "${BLUE}║      PipelineX Installer v2.1.1       ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════╝${NC}"
echo ""

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

echo -e "${BLUE}Detected:${NC} $OS ($ARCH)"

case "$OS" in
    Linux*)
        PLATFORM="x86_64-unknown-linux-gnu"
        ;;
    Darwin*)
        if [ "$ARCH" = "arm64" ]; then
            PLATFORM="aarch64-apple-darwin"
        else
            PLATFORM="x86_64-apple-darwin"
        fi
        ;;
    MINGW*|MSYS*|CYGWIN*)
        PLATFORM="x86_64-pc-windows-msvc"
        BIN_NAME="pipelinex.exe"
        ;;
    *)
        echo -e "${RED}Unsupported operating system: $OS${NC}"
        exit 1
        ;;
esac

echo -e "${BLUE}Platform:${NC} $PLATFORM"
echo ""

# Check if running as root for system install
if [ "$INSTALL_DIR" = "/usr/local/bin" ] && [ "$EUID" -ne 0 ] && [ ! -w "$INSTALL_DIR" ]; then
    echo -e "${YELLOW}Note: Installing to $INSTALL_DIR requires sudo${NC}"
    USE_SUDO="sudo"
else
    USE_SUDO=""
fi

# Installation method selection
echo -e "${BLUE}Choose installation method:${NC}"
echo "  1) Download pre-built binary (fastest)"
echo "  2) Build from source with Cargo (requires Rust)"
echo ""
read -p "Enter choice [1]: " CHOICE
CHOICE=${CHOICE:-1}

if [ "$CHOICE" = "1" ]; then
    echo ""
    echo -e "${BLUE}Downloading latest release...${NC}"

    # Get latest release URL
    LATEST_RELEASE=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep "browser_download_url.*$PLATFORM" | cut -d '"' -f 4)

    if [ -z "$LATEST_RELEASE" ]; then
        echo -e "${RED}Failed to find binary for platform: $PLATFORM${NC}"
        echo -e "${YELLOW}Falling back to source installation...${NC}"
        CHOICE="2"
    else
        TEMP_DIR=$(mktemp -d)
        cd "$TEMP_DIR"

        curl -fsSL "$LATEST_RELEASE" -o "$BIN_NAME"
        chmod +x "$BIN_NAME"

        $USE_SUDO mv "$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"

        rm -rf "$TEMP_DIR"

        echo -e "${GREEN}✓ Binary installed to $INSTALL_DIR/$BIN_NAME${NC}"
    fi
fi

if [ "$CHOICE" = "2" ]; then
    echo ""
    echo -e "${BLUE}Building from source...${NC}"

    # Check for Cargo
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Cargo not found!${NC}"
        echo -e "${YELLOW}Install Rust from: https://rustup.rs${NC}"
        exit 1
    fi

    echo "This may take a few minutes..."
    cargo install --git "https://github.com/$REPO" pipelinex-cli --force

    echo -e "${GREEN}✓ Built and installed via Cargo${NC}"
fi

# Verify installation
echo ""
if command -v pipelinex &> /dev/null; then
    VERSION=$(pipelinex --version 2>/dev/null || echo "unknown")
    echo -e "${GREEN}╔═══════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║   Installation successful! 🚀          ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${BLUE}Version:${NC} $VERSION"
    echo -e "${BLUE}Location:${NC} $(which pipelinex)"
    echo ""
    echo -e "${BLUE}Quick Start:${NC}"
    echo "  pipelinex analyze .github/workflows/ci.yml"
    echo "  pipelinex optimize .github/workflows/ci.yml -o optimized.yml"
    echo "  pipelinex diff .github/workflows/ci.yml"
    echo ""
    echo -e "${BLUE}Documentation:${NC} https://github.com/$REPO"
else
    echo -e "${RED}Installation failed!${NC}"
    echo -e "${YELLOW}Try manual installation:${NC}"
    echo "  cargo install --git https://github.com/$REPO pipelinex-cli"
    exit 1
fi

# Optional: Add shell completion
echo ""
read -p "Install shell completion? [y/N]: " COMPLETION
if [ "$COMPLETION" = "y" ] || [ "$COMPLETION" = "Y" ]; then
    SHELL_NAME=$(basename "$SHELL")
    case "$SHELL_NAME" in
        bash)
            pipelinex --generate-completion bash > /tmp/pipelinex.bash 2>/dev/null || true
            if [ -f /tmp/pipelinex.bash ]; then
                $USE_SUDO mv /tmp/pipelinex.bash /etc/bash_completion.d/pipelinex
                echo -e "${GREEN}✓ Bash completion installed${NC}"
                echo "  Restart your shell or run: source /etc/bash_completion.d/pipelinex"
            fi
            ;;
        zsh)
            echo -e "${YELLOW}Add to ~/.zshrc:${NC}"
            echo "  eval \"\$(pipelinex --generate-completion zsh)\""
            ;;
        fish)
            pipelinex --generate-completion fish > ~/.config/fish/completions/pipelinex.fish 2>/dev/null || true
            echo -e "${GREEN}✓ Fish completion installed${NC}"
            ;;
        *)
            echo -e "${YELLOW}Shell completions not configured for $SHELL_NAME${NC}"
            ;;
    esac
fi

echo ""
echo -e "${GREEN}Happy optimizing! ⚡${NC}"
