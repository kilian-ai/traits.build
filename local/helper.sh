#!/bin/bash
# traits.build — Local Helper Runtime (one-shot)
# Usage: curl -fsSL https://traits.build/local/helper.sh | bash
#
# Downloads and runs the traits helper on localhost.
# The WASM kernel in your browser will auto-discover this on port 8090.
# No package manager required — downloads a single binary to /tmp.
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

# ── Platform detection ──
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64)        ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
esac

# ── 1. Download fresh binary to tmp ──
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
    echo "  (no prebuilt binary for $OS/$ARCH)"
fi

# ── 2. Fallback: use local binary if already installed ──
for bin in \
    "$(command -v traits 2>/dev/null || true)" \
    "$HOME/.local/bin/traits" \
    "$HOME/.traits/bin/traits" \
    "/usr/local/bin/traits"; do
    if [ -n "$bin" ] && [ -x "$bin" ]; then
        banner
        echo "✓ Using local: $bin"
        exec "$bin" serve --port "$PORT"
    fi
done

# ── 3. Last resort: build from source ──
if command -v cargo &>/dev/null; then
    echo "Building from source (1-2 min on first run)..."
    cargo install --git "https://github.com/$REPO" --locked 2>&1
    if command -v traits &>/dev/null; then
        banner
        exec traits serve --port "$PORT"
    fi
fi

echo ""
echo "✗ Could not download or run the traits binary."
echo ""
echo "  Options:"
echo "    1. Download manually:       https://github.com/$REPO/releases"
echo "    2. Install permanently:     curl -fsSL https://traits.build/local/install.sh | bash"
echo "    3. Build from source:       cargo install --git https://github.com/$REPO"
echo ""
exit 1
