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

# Start one websocat bridge per port
for PORT in $PORTS; do
    # Only bridge ports that have a local listener
    if ! nc -z 127.0.0.1 "$PORT" 2>/dev/null; then
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
echo "  Connect using any of:"
echo ""
for PORT in $PORTS; do
    echo "  WebSocket: wss://tunnel.traits.build/port/client?code=${CODE}&port=${PORT}"
    if [ "$PORT" = "22" ]; then
        echo "  SSH:       ssh -o ProxyCommand=\"websocat wss://tunnel.traits.build/port/client?code=${CODE}&port=22\" root@dummy"
    fi
done
echo ""
echo "  Status:  curl -s $TUNNEL_BASE/port/status?code=${CODE}"
echo "  Debug:   curl -s $TUNNEL_BASE/port/debug?code=${CODE}"
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
