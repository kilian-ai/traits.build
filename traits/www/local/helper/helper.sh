#!/bin/bash
# traits.build — Local Helper Runtime (one-shot)
# Usage: curl -fsSL https://traits.build/local/helper.sh | bash
#
# Starts the traits helper on localhost for browser dispatch.
# The WASM kernel in your browser will auto-discover this on port 8090.
set -euo pipefail

PORT="${TRAITS_PORT:-8090}"
REPO="kilian-ai/traits.build"

banner() {
    echo ""
    echo "  ┌────────────────────────────────────┐"
    echo "  │  traits.build — local helper        │"
    echo "  │  http://localhost:$PORT              │"
    echo "  └────────────────────────────────────┘"
    echo ""
}

# ── 1. Check if traits binary already in PATH ──
if command -v traits &>/dev/null; then
    banner
    echo "✓ Found: $(which traits)"
    exec traits serve --port "$PORT"
fi

# ── 2. Check common install locations ──
for dir in "$HOME/.local/bin" "$HOME/.traits/bin" "/usr/local/bin"; do
    if [ -x "$dir/traits" ]; then
        banner
        echo "✓ Found: $dir/traits"
        exec "$dir/traits" serve --port "$PORT"
    fi
done

# ── 3. Download from GitHub releases ──
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64)        ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
esac

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Fetching latest release..."
LATEST="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
    | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/' || echo "")"

if [ -n "$LATEST" ]; then
    BINARY_URL="https://github.com/$REPO/releases/download/$LATEST/traits-$OS-$ARCH"
    echo "Downloading traits $LATEST ($OS/$ARCH)..."
    if curl -fsSL "$BINARY_URL" -o "$TMPDIR/traits" 2>/dev/null; then
        chmod +x "$TMPDIR/traits"
        banner
        echo "✓ Downloaded traits $LATEST"
        echo "  (run 'curl -fsSL https://traits.build/local/install.sh | bash' to install permanently)"
        echo ""
        exec "$TMPDIR/traits" serve --port "$PORT"
    fi
fi

# ── 4. Fallback: build from source ──
if command -v cargo &>/dev/null; then
    echo "No prebuilt binary found. Building from source..."
    echo "  (this takes 1-2 minutes on first run)"
    cargo install --git "https://github.com/$REPO" --locked 2>&1
    if command -v traits &>/dev/null; then
        banner
        exec traits serve --port "$PORT"
    fi
fi

echo ""
echo "✗ Could not find, download, or build the traits binary."
echo ""
echo "  Options:"
echo "    1. Install Rust and retry:  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
echo "    2. Download manually:       https://github.com/$REPO/releases"
echo "    3. Install permanently:     curl -fsSL https://traits.build/local/install.sh | bash"
echo ""
exit 1
