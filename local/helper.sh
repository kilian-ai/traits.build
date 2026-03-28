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

run_traits() {
    local bin="$1"
    shift

    if [ "${1:-}" = "serve" ]; then
        if [ -z "${RELAY_URL:-}" ] || [ "$RELAY_URL" = "https://traits-build.fly.dev" ]; then
            RELAY_URL="https://relay.traits.build"
        fi
        export RELAY_URL
        echo "↳ Relay URL: $RELAY_URL"
    fi

    # When launched via curl | bash, stdin is usually a closed pipe.
    # Reattach /dev/tty for `serve` so the interactive REPL remains usable.
    if [ "${1:-}" = "serve" ] && [ ! -t 0 ] && [ -r /dev/tty ]; then
        echo "↳ Reattaching terminal for REPL (/dev/tty)"
        exec env RELAY_URL="$RELAY_URL" "$bin" "$@" < /dev/tty > /dev/tty 2>&1
    fi

    if [ "${1:-}" = "serve" ]; then
        exec env RELAY_URL="$RELAY_URL" "$bin" "$@"
    fi

    exec "$bin" "$@"
}

# ── Default command: serve (with relay) ──
if [ $# -eq 0 ]; then
    PORT="${TRAITS_PORT:-8090}"
    if [ -z "${RELAY_URL:-}" ] || [ "$RELAY_URL" = "https://traits-build.fly.dev" ]; then
        RELAY_URL="https://relay.traits.build"
    fi
    export RELAY_URL
    set -- serve --port "$PORT"
fi

# Ensure `serve` gets a relay default even when args were provided explicitly.
if [ "${1:-}" = "serve" ] && { [ -z "${RELAY_URL:-}" ] || [ "$RELAY_URL" = "https://traits-build.fly.dev" ]; }; then
    RELAY_URL="https://relay.traits.build"
    export RELAY_URL
fi

# For piped startup (`curl ... | bash`) in serve mode, re-exec via a real script file
# on /dev/tty so terminal input behaves like a normal local script launch.
if [ "${1:-}" = "serve" ] && [ ! -t 0 ] && [ -r /dev/tty ] && [ "${TRAITS_HELPER_REEXEC:-0}" != "1" ]; then
    HELPER_URL="${TRAITS_HELPER_URL:-https://traits.build/local/helper.sh}"
    HELPER_DIR="$(mktemp -d)"
    trap 'rm -rf "$HELPER_DIR"' EXIT
    HELPER_FILE="$HELPER_DIR/helper.sh"
    if curl -fsSL --connect-timeout 10 "$HELPER_URL" -o "$HELPER_FILE" 2>/dev/null; then
        chmod +x "$HELPER_FILE"
        echo "↳ Switching to file-mode helper for interactive REPL"
        exec env TRAITS_HELPER_REEXEC=1 RELAY_URL="$RELAY_URL" TRAITS_REPL_LINE_MODE="${TRAITS_REPL_LINE_MODE:-}" bash "$HELPER_FILE" "$@" < /dev/tty > /dev/tty 2>&1
    fi
    echo "↳ Could not switch to file-mode helper; continuing in pipe mode"
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

# ── Find local binary ──
LOCAL_BIN=""
for bin in \
    "$(command -v traits 2>/dev/null || true)" \
    "$HOME/.local/bin/traits" \
    "$HOME/.traits/bin/traits" \
    "/usr/local/bin/traits"; do
    if [ -n "$bin" ] && [ -x "$bin" ]; then
        LOCAL_BIN="$bin"
        break
    fi
done

LOCAL_VERSION=""
if [ -n "$LOCAL_BIN" ]; then
    LOCAL_VERSION="$("$LOCAL_BIN" version </dev/null 2>/dev/null | grep -oE 'v[0-9]{6,}\.[0-9]+' | head -1 || true)"
fi

# ── Check latest remote version (via git tags, not GitHub Releases) ──
echo "Checking for updates..."
LATEST=""
LATEST="$(curl -fsSL --connect-timeout 3 "https://api.github.com/repos/$REPO/tags?per_page=1" 2>/dev/null \
    | grep '"name"' | head -1 | sed -E 's/.*"([^"]+)".*/\1/' || echo "")"

# ── Use local binary if it's up-to-date ──
# Versions are vYYMMDD.HHMMSS — lexicographic >= works correctly.
# If we can't reach GitHub (offline), fall back to local binary.
if [ -n "$LOCAL_BIN" ] && [ -n "$LOCAL_VERSION" ]; then
    if [ -n "$LATEST" ]; then
        if [ "$LOCAL_VERSION" = "$LATEST" ] || [[ "$LOCAL_VERSION" > "$LATEST" ]]; then
            banner "$@"
            echo "✓ Using local: $LOCAL_BIN ($LOCAL_VERSION)"
            echo ""
            run_traits "$LOCAL_BIN" "$@"
        else
            echo "  Local $LOCAL_VERSION → remote $LATEST (updating...)"
        fi
    else
        # Offline — use local if available
        banner "$@"
        echo "✓ Using local (offline): $LOCAL_BIN ($LOCAL_VERSION)"
        echo ""
        run_traits "$LOCAL_BIN" "$@"
    fi
fi

# ── 1. Try GitHub Releases (cross-platform binaries from CI) ──
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

if [ -n "$LATEST" ]; then
    # GitHub Actions uploads: traits-linux-x86_64, traits-linux-aarch64, traits-macos-aarch64, traits-macos-x86_64
    BINARY_NAME="traits-${RUST_OS}-${RUST_ARCH}"
    BINARY_URL="https://github.com/$REPO/releases/download/$LATEST/$BINARY_NAME"
    echo "Downloading traits $LATEST ($RUST_OS/$RUST_ARCH)..."
    if curl -fsSL --connect-timeout 10 "$BINARY_URL" -o "$TMPDIR/traits" 2>/dev/null; then
        if [ -s "$TMPDIR/traits" ]; then
            chmod +x "$TMPDIR/traits"
            banner "$@"
            echo "✓ Downloaded traits $LATEST"
            echo ""
            run_traits "$TMPDIR/traits" "$@"
        fi
    fi
    echo "  (no prebuilt binary for $RUST_OS/$RUST_ARCH)"
fi

# ── 2. Build from source ──
if command -v cargo &>/dev/null; then
    echo "Building from source..."
    cargo install --git "https://github.com/$REPO" --locked 2>&1
    if command -v traits &>/dev/null; then
        banner "$@"
        run_traits traits "$@"
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
