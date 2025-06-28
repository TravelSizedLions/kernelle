#!/usr/bin/env bash
set -euo pipefail

# Kernelle Installation Script - Phase 1
# This script installs Kernelle and sets up the basic lifecycle infrastructure

# Parse arguments
NON_INTERACTIVE=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --non-interactive)
            NON_INTERACTIVE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--non-interactive]"
            echo ""
            echo "Options:"
            echo "  --non-interactive    Skip interactive prompts (for CI/automation)"
            echo "  --help, -h          Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--non-interactive]"
            exit 1
            ;;
    esac
done

echo "🚀 Installing Kernelle..."

# Configuration
KERNELLE_HOME="${KERNELLE_HOME:-$HOME/.kernelle}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Create directories
echo "📁 Creating directories..."
mkdir -p "$KERNELLE_HOME"
mkdir -p "$INSTALL_DIR"

# For Phase 1, we'll assume we're running from the source directory
# In Phase 2+, this would clone from a repo
# Portable way to get script directory (works in bash and zsh)
if [ -n "${BASH_SOURCE[0]}" ]; then
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
else
    # zsh and other shells
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
fi
REPO_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🔨 Building Kernelle tools..."
echo "Script directory: $SCRIPT_DIR"
echo "Repository root: $REPO_ROOT"
echo "Current directory: $(pwd)"
echo "Looking for Cargo.toml at: $REPO_ROOT/Cargo.toml"

if [ ! -f "$REPO_ROOT/Cargo.toml" ]; then
    echo "❌ Error: Cargo.toml not found at $REPO_ROOT/Cargo.toml"
    echo "Contents of $REPO_ROOT:"
    ls -la "$REPO_ROOT"
    exit 1
fi

cd "$REPO_ROOT"
cargo build --release

echo "📦 Installing binaries..."
# Install all the tools to $INSTALL_DIR (only binaries that exist)
for binary in kernelle jerrod blizz violet adam sentinel; do
    if [ -f "target/release/$binary" ]; then
        cp "target/release/$binary" "$INSTALL_DIR/"
        echo "  Installed: $binary"
    else
        echo "  Skipped: $binary (binary not found)"
    fi
done

echo "📋 Setting up workflows..."
# Copy .cursor rules to ~/.kernelle/.cursor
if [ -d "$REPO_ROOT/.cursor" ]; then
    cp -r "$REPO_ROOT/.cursor" "$KERNELLE_HOME/"
else
    echo "⚠️  No .cursor directory found - workflows will not be available"
fi

echo "🔗 Creating source file..."
# Copy kernelle.source template to ~/.kernelle/
cp "$SCRIPT_DIR/templates/kernelle.source.template" "$HOME/.kernelle.source"

echo "✅ Kernelle installed successfully!"
echo ""
echo "📝 Next steps:"
echo "1. Add the following line to your shell configuration (~/.bashrc, ~/.zshrc, etc.):"
echo "   source ~/.kernelle.source"
echo ""
echo "2. Reload your shell or run: source ~/.kernelle.source"
echo ""
echo "3. Test the installation:"
echo "   kernelle --help"
echo "   kernelle add .  # (in a project directory)"
echo ""
echo "🎉 Welcome to Kernelle!" 