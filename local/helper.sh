#!/bin/bash
# traits.build — Run any traits command (one-shot)
# Usage:
#   curl -fsSL https://traits.build/local/traits.sh | bash
#   curl -fsSL https://traits.build/local/traits.sh | bash -s -- serve --port 9090
#   curl -fsSL https://traits.build/local/traits.sh | bash -s -- list
#   curl -fsSL https://traits.build/local/traits.sh | bash -s -- checksum hash "hello"
#
# Downloads the traits binary and runs it. Default: serve --port 8090
# No package manager required — downloads a single binary to /tmp.
set -euo pipefail

REPO="kilian-ai/traits.build"

# ── Default command: serve (with relay) ──
if [ $# -eq 0 ]; then
    PORT="${TRAITS_PORT:-8090}"
    RELAY_URL="${RELAY_URL:-https://traits-build.fly.dev}"
    export RELAY_URL
    set -- serve --port "$PORT"
fi

banner() {
    echo ""
    echo "  ┌────────────────────────────────────┐"
    echo "  │  traits.build                       │"
    echo "  │  → traits $*"
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

# Normalize OS names to match Rust std::env::consts::OS / ARCH
RUST_OS="$OS"
RUST_ARCH="$ARCH"
case "$OS" in
    darwin) RUST_OS="macos" ;;
esac
case "$ARCH" in
    amd64) RUST_ARCH="x86_64" ;;
    arm64) RUST_ARCH="aarch64" ;;
esac

# ── 1. Try traits.build server binary (fastest — serves its own binary) ──
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Checking traits.build for $OS/$ARCH binary..."
HEADERS="$(curl -fsSL -D - -o "$TMPDIR/traits" "https://traits.build/local/binary" 2>/dev/null || true)"
if [ -f "$TMPDIR/traits" ] && [ -s "$TMPDIR/traits" ]; then
    # Check platform match via response headers
    REMOTE_OS="$(echo "$HEADERS" | grep -i 'X-Traits-OS:' | tr -d '\r' | awk '{print $2}')"
    REMOTE_ARCH="$(echo "$HEADERS" | grep -i 'X-Traits-Arch:' | tr -d '\r' | awk '{print $2}')"
    if [ "$REMOTE_OS" = "$RUST_OS" ] && [ "$REMOTE_ARCH" = "$RUST_ARCH" ]; then
        chmod +x "$TMPDIR/traits"
        banner "$@"
        echo "✓ Downloaded traits binary ($REMOTE_OS/$REMOTE_ARCH)"
        echo ""
        exec "$TMPDIR/traits" "$@"
    else
        echo "  Server binary is $REMOTE_OS/$REMOTE_ARCH — need $RUST_OS/$RUST_ARCH"
        rm -f "$TMPDIR/traits"
    fi
fi

# ── 2. Try GitHub Releases ──
echo "Checking GitHub releases..."
LATEST="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
    | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/' || echo "")"

if [ -n "$LATEST" ]; then
    BINARY_URL="https://github.com/$REPO/releases/download/$LATEST/traits-$OS-$ARCH"
    echo "Downloading traits $LATEST ($OS/$ARCH)..."
    if curl -fsSL "$BINARY_URL" -o "$TMPDIR/traits" 2>/dev/null; then
        chmod +x "$TMPDIR/traits"
        banner "$@"
        echo "✓ Downloaded traits $LATEST"
        echo ""
        exec "$TMPDIR/traits" "$@"
    fi
    echo "  (no prebuilt binary for $OS/$ARCH)"
fi

# ── 3. Build from source (always fresh) ──
if command -v cargo &>/dev/null; then
    echo "Building from source (1-2 min on first run)..."
    cargo install --git "https://github.com/$REPO" --locked 2>&1
    if command -v traits &>/dev/null; then
        banner "$@"
        exec traits "$@"
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
