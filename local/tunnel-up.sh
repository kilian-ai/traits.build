#!/bin/sh
# tunnel-up.sh — Expose local TCP ports through tunnel.traits.build
# Usage: tunnel-up.sh [port1] [port2] ...
# Defaults: sshd(22), syncthing-sync(22000), syncthing-gui(8384)
#
# Requires: websocat, curl, python3
# Install: apk add --no-cache websocat curl python3
#
# After running, SSH access (from any machine with websocat):
#   ssh -o ProxyCommand="websocat wss://tunnel.traits.build/port/client?code=CODE&port=22" root@dummy
#
# Or with raw TCP gateway (Phase 3, ports.traits.build):
#   ssh -p 2222 ports.traits.build  (with PORT_CODE=CODE env on gateway)

set -u

TUNNEL_BASE="https://tunnel.traits.build"
PORTS="${*:-22 22000 8384}"

# Install websocat if missing
if ! command -v websocat >/dev/null 2>&1; then
    echo "[tunnel] installing websocat..."
    apk add --no-cache websocat curl >/dev/null 2>&1 \
        || { echo "[tunnel] apk failed — install websocat manually"; exit 1; }
fi

# Build ports JSON array  e.g. "22 8384" → [22,8384]
PORTS_JSON=$(printf '['; first=1
for p in $PORTS; do
    [ "$first" = 1 ] || printf ','
    printf '%s' "$p"
    first=0
done
printf ']')

echo "[tunnel] registering ports $PORTS_JSON ..."
RESP=$(curl -sS -X POST "$TUNNEL_BASE/port/register" \
    -H 'Content-Type: application/json' \
    -d "{\"ports\":${PORTS_JSON}}")

# Parse code from JSON without python — grep/sed only
CODE=$(printf '%s' "$RESP" | sed -n 's/.*"code"[[:space:]]*:[[:space:]]*"\([A-Z0-9]\{4\}\)".*/\1/p')
if [ -z "$CODE" ]; then
    echo "[tunnel] registration failed: $RESP"
    exit 1
fi

echo "[tunnel] pairing code: $CODE"
echo "[tunnel] relay: $TUNNEL_BASE"
echo ""

# Auto-start sshd if port 22 requested and no listener
auto_start_sshd() {
    if ! command -v sshd >/dev/null 2>&1; then
        echo "[tunnel]   installing openssh-server..."
        apk add --no-cache openssh-server >/dev/null 2>&1 || { echo "[tunnel]   apk add failed"; return 1; }
    fi
    if [ ! -f /etc/ssh/ssh_host_rsa_key ] && [ ! -f /etc/ssh/ssh_host_ed25519_key ]; then
        echo "[tunnel]   generating host keys (ed25519 only for speed)..."
        ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_ed25519_key >/dev/null 2>&1
    fi
    grep -q '^PermitRootLogin yes' /etc/ssh/sshd_config 2>/dev/null \
        || echo 'PermitRootLogin yes' >> /etc/ssh/sshd_config
    if [ ! -s /root/.ssh/authorized_keys ] && ! grep -q '^root:[^*!]' /etc/shadow 2>/dev/null; then
        echo "[tunnel]   WARNING: root has no password/authorized_keys — run 'passwd' first"
    fi
    echo "[tunnel]   launching sshd..."
    /usr/sbin/sshd 2>&1 | head -5
}

# Check if TCP port has a LISTEN socket via /proc/net/tcp (avoids nc hangs)
port_listening() {
    local port_hex
    port_hex=$(printf '%04X' "$1")
    # State 0A = TCP_LISTEN
    grep -qE ":${port_hex} [0-9A-F:]+ 0A " /proc/net/tcp 2>/dev/null \
        || grep -qE ":${port_hex} [0-9A-F:]+ 0A " /proc/net/tcp6 2>/dev/null
}

# Start one websocat bridge per port
for PORT in $PORTS; do
    echo "[tunnel] checking port $PORT ..."
    if ! port_listening "$PORT"; then
        if [ "$PORT" = "22" ]; then
            echo "[tunnel] port 22 — no listener, starting sshd..."
            auto_start_sshd
            # Short retry (~3s)
            i=0
            while [ $i -lt 15 ]; do
                port_listening 22 && break
                i=$((i+1)); sleep 0.2 2>/dev/null || sleep 1
            done
        fi
    fi
    if ! port_listening "$PORT"; then
        echo "[tunnel] port $PORT — no local listener, skipping bridge"
        continue
    fi
    WS_URL="wss://$(printf '%s' "$TUNNEL_BASE" | sed 's|https://||')/port/guest?code=${CODE}&port=${PORT}"
    websocat --binary --ping-interval 30 "$WS_URL" "tcp:127.0.0.1:${PORT}" \
        >> "/tmp/tunnel-${PORT}.log" 2>&1 &
    BPID=$!
    echo "[tunnel] port $PORT → bridge PID $BPID  log: /tmp/tunnel-${PORT}.log"
done

echo ""
echo "──────────────────────────────────────────────────────"
echo "  Connect from any machine with websocat:"
echo ""
for PORT in $PORTS; do
    echo "  WebSocket: 'wss://tunnel.traits.build/port/client?code=${CODE}&port=${PORT}'"
    if [ "$PORT" = "22" ]; then
        echo ""
        echo "  SSH (bash/sh):"
        echo "    ssh -o ProxyCommand='websocat wss://tunnel.traits.build/port/client?code=${CODE}&port=22' root@dummy"
        echo ""
        echo "  SSH (zsh — use noglob):"
        echo "    noglob ssh -o ProxyCommand=\"websocat wss://tunnel.traits.build/port/client?code=${CODE}&port=22\" root@dummy"
    fi
done
echo ""
echo "  Status:  curl -s 'https://tunnel.traits.build/port/status?code=${CODE}'"
echo "  Debug:   curl -s 'https://tunnel.traits.build/port/debug?code=${CODE}'"
echo "──────────────────────────────────────────────────────"
echo ""
echo "[tunnel] Bridges running. Press Ctrl-C to teardown."

# Save code for scripts that need it
printf '%s\n' "$CODE" > /tmp/tunnel.code
export TUNNEL_CODE="$CODE"

# Wait for interrupt, then unregister
trap '
    echo ""
    echo "[tunnel] unregistering $CODE ..."
    curl -sS -X POST "$TUNNEL_BASE/port/unregister" \
        -H "Content-Type: application/json" \
        -d "{\"code\":\"$CODE\"}" > /dev/null
    rm -f /tmp/tunnel.code
    echo "[tunnel] done."
    exit 0
' INT TERM

# Keep alive (background bridges continue)
while :; do sleep 30; done
