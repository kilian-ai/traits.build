#!/bin/bash
# traits.build — Install Helper Permanently
# Usage: curl -fsSL https://traits.build/local/install.sh | bash
#
# Installs the traits binary and optionally sets up auto-start.
set -euo pipefail

REPO="kilian-ai/traits.build"
INSTALL_DIR="${TRAITS_INSTALL_DIR:-$HOME/.local/bin}"
PORT="${TRAITS_PORT:-8090}"

# ── Platform detection ──
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64)        ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
esac

# Normalize to Rust platform names for server matching
RUST_OS="$OS"
RUST_ARCH="$ARCH"
case "$OS" in
    darwin) RUST_OS="macos" ;;
esac
case "$ARCH" in
    amd64) RUST_ARCH="x86_64" ;;
    arm64) RUST_ARCH="aarch64" ;;
esac

echo ""
echo "  traits.build — installer"
echo "  Platform: $OS/$ARCH"
echo "  Target:   $INSTALL_DIR/traits"
echo ""

mkdir -p "$INSTALL_DIR"

# ── 1. Try GitHub Releases (cross-platform binaries from CI) ──
INSTALLED=false

LATEST="$(curl -fsSL --connect-timeout 3 "https://api.github.com/repos/$REPO/tags?per_page=1" 2>/dev/null \
    | grep '"name"' | head -1 | sed -E 's/.*"([^"]+)".*/\1/' || echo "")"

if [ -n "$LATEST" ]; then
    BINARY_NAME="traits-${RUST_OS}-${RUST_ARCH}"
    BINARY_URL="https://github.com/$REPO/releases/download/$LATEST/$BINARY_NAME"
    echo "Downloading traits $LATEST ($RUST_OS/$RUST_ARCH)..."
    if curl -fsSL --connect-timeout 10 "$BINARY_URL" -o "$INSTALL_DIR/traits.tmp" 2>/dev/null && [ -s "$INSTALL_DIR/traits.tmp" ]; then
        mv "$INSTALL_DIR/traits.tmp" "$INSTALL_DIR/traits"
        chmod +x "$INSTALL_DIR/traits"
        INSTALLED=true
        echo "✓ Installed traits $LATEST → $INSTALL_DIR/traits"
    else
        rm -f "$INSTALL_DIR/traits.tmp"
        echo "  (no prebuilt binary for $RUST_OS/$RUST_ARCH)"
    fi
fi

# ── 2. Try Fly.io server binary (fallback) ──
if [ "$INSTALLED" = false ]; then
    FLY_URL="${TRAITS_SERVER:-https://relay.traits.build}"
    HEADERS="$(curl -fsSL --connect-timeout 5 -D - -o "$INSTALL_DIR/traits.tmp" "$FLY_URL/local/binary" 2>/dev/null || true)"
    if [ -f "$INSTALL_DIR/traits.tmp" ] && [ -s "$INSTALL_DIR/traits.tmp" ]; then
        REMOTE_OS="$(echo "$HEADERS" | grep -i 'X-Traits-OS:' | tr -d '\r' | awk '{print $2}')"
        REMOTE_ARCH="$(echo "$HEADERS" | grep -i 'X-Traits-Arch:' | tr -d '\r' | awk '{print $2}')"
        if [ "$REMOTE_OS" = "$RUST_OS" ] && [ "$REMOTE_ARCH" = "$RUST_ARCH" ]; then
            mv "$INSTALL_DIR/traits.tmp" "$INSTALL_DIR/traits"
            chmod +x "$INSTALL_DIR/traits"
            INSTALLED=true
            echo "✓ Installed traits ($REMOTE_OS/$REMOTE_ARCH) → $INSTALL_DIR/traits"
        else
            echo "  Server binary is $REMOTE_OS/$REMOTE_ARCH — need $RUST_OS/$RUST_ARCH"
            rm -f "$INSTALL_DIR/traits.tmp"
        fi
    else
        rm -f "$INSTALL_DIR/traits.tmp"
    fi
fi

# ── 2. Fallback: build from source ──
if [ "$INSTALLED" = false ]; then
    if command -v cargo &>/dev/null; then
        echo "Building from source (this takes 1-2 minutes)..."
        cargo install --git "https://github.com/$REPO" --root "${INSTALL_DIR%/bin}" --locked 2>&1
        if [ -x "$INSTALL_DIR/traits" ]; then
            INSTALLED=true
            echo "✓ Built and installed → $INSTALL_DIR/traits"
        fi
    else
        echo "✗ No prebuilt binary and cargo not found."
        echo "  Install Rust first: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
fi

if [ "$INSTALLED" = false ]; then
    echo "✗ Installation failed."
    exit 1
fi

# ── 3. Add to PATH ──
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    SHELL_RC=""
    case "${SHELL:-}" in
        */zsh)  SHELL_RC="$HOME/.zshrc" ;;
        */bash) SHELL_RC="$HOME/.bashrc" ;;
    esac
    if [ -n "$SHELL_RC" ]; then
        if ! grep -q "$INSTALL_DIR" "$SHELL_RC" 2>/dev/null; then
            echo "" >> "$SHELL_RC"
            echo "# traits.build helper" >> "$SHELL_RC"
            echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$SHELL_RC"
            echo "✓ Added $INSTALL_DIR to PATH in $SHELL_RC"
            echo "  (restart your shell or run: source $SHELL_RC)"
        fi
    fi
fi

# ── 4. Auto-start setup (optional) ──
echo ""
echo "  ✓ Installation complete!"
echo ""
echo "  Quick start:"
echo "    traits serve --port $PORT"
echo ""

# macOS: launchd
if [ "$OS" = "darwin" ]; then
    PLIST_DIR="$HOME/Library/LaunchAgents"
    PLIST="$PLIST_DIR/build.traits.helper.plist"

    if [ ! -f "$PLIST" ]; then
        echo "  Auto-start on login (macOS)?"
        echo "  Run this to enable:"
        echo ""
        echo "    cat > '$PLIST' << 'PLIST'"
        cat << PLIST_CONTENT
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>build.traits.helper</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/traits</string>
        <string>serve</string>
        <string>--port</string>
        <string>$PORT</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$HOME/.traits/helper.log</string>
    <key>StandardErrorPath</key>
    <string>$HOME/.traits/helper.err</string>
</dict>
</plist>
PLIST_CONTENT
        echo "PLIST"
        echo ""
        echo "    launchctl load '$PLIST'"
        echo ""
    fi
fi

# Linux: systemd
if [ "$OS" = "linux" ] && command -v systemctl &>/dev/null; then
    SERVICE="$HOME/.config/systemd/user/traits-helper.service"
    if [ ! -f "$SERVICE" ]; then
        echo "  Auto-start on login (systemd)?"
        echo "  Run this to enable:"
        echo ""
        echo "    mkdir -p ~/.config/systemd/user"
        echo "    cat > '$SERVICE' << 'SERVICE'"
        cat << SERVICE_CONTENT
[Unit]
Description=traits.build local helper
After=network.target

[Service]
ExecStart=$INSTALL_DIR/traits serve --port $PORT
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
SERVICE_CONTENT
        echo "SERVICE"
        echo ""
        echo "    systemctl --user daemon-reload"
        echo "    systemctl --user enable --now traits-helper"
        echo ""
    fi
fi
