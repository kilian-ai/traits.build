#!/bin/sh
# tunnel-listen-ftp.sh — Open local FTP control + passive listeners to a tunnel code.
#
# Usage:
#   tunnel-listen-ftp.sh CODE [local_control_port] [remote_control_port] [pasv_min] [pasv_max]
#
# Defaults:
#   local_control_port = 2121
#   remote_control_port = 21
#   pasv_min = 30000
#   pasv_max = 30010
#
# One-liner:
#   sh <(curl -sS https://www.traits.build/local/tunnel-listen-ftp.sh) CODE
#
# Then configure FTP client:
#   Host: 127.0.0.1
#   Port: 2121
#   Passive mode: ON
#   Passive ports: 30000-30010

set -u

CODE="${1:?usage: tunnel-listen-ftp.sh CODE [local_control_port] [remote_control_port] [pasv_min] [pasv_max]}"
LOCAL_CTRL_PORT="${2:-2121}"
REMOTE_CTRL_PORT="${3:-21}"
PASV_MIN="${4:-30000}"
PASV_MAX="${5:-30010}"

# Override via env: TUNNEL_WS=ws://localhost:8787 sh tunnel-listen-ftp.sh CODE
TUNNEL_WS="${TUNNEL_WS:-wss://traits-build-tunnel.fly.dev}"

if ! command -v websocat >/dev/null 2>&1; then
    echo "[tunnel-listen-ftp] websocat not found — install via: brew install websocat" >&2
    exit 1
fi

is_int() {
    case "$1" in
        ''|*[!0-9]*) return 1 ;;
        *) return 0 ;;
    esac
}

for n in "$LOCAL_CTRL_PORT" "$REMOTE_CTRL_PORT" "$PASV_MIN" "$PASV_MAX"; do
    if ! is_int "$n"; then
        echo "[tunnel-listen-ftp] invalid numeric value: $n" >&2
        exit 1
    fi
done

if [ "$PASV_MIN" -gt "$PASV_MAX" ]; then
    echo "[tunnel-listen-ftp] invalid passive range: $PASV_MIN-$PASV_MAX" >&2
    exit 1
fi

PIDS=""

start_listener() {
    local_port="$1"
    remote_port="$2"
    ws_url="${TUNNEL_WS}/port/client?code=${CODE}&port=${remote_port}"
    # -E exits when one side disconnects; each new local connection gets a fresh WS.
    websocat --binary -E "tcp-l:127.0.0.1:${local_port}" "$ws_url" >/tmp/tunnel-ftp-${local_port}.log 2>&1 &
    pid="$!"
    PIDS="$PIDS $pid"
    echo "[tunnel-listen-ftp] 127.0.0.1:${local_port} -> tunnel:${CODE} remote:${remote_port} (pid $pid)"
}

cleanup() {
    echo ""
    echo "[tunnel-listen-ftp] stopping listeners..."
    for p in $PIDS; do
        kill "$p" 2>/dev/null || true
    done
    wait 2>/dev/null || true
    echo "[tunnel-listen-ftp] done"
}

trap cleanup INT TERM EXIT

echo "──────────────────────────────────────────────────────"
echo "  FTP tunnel code: ${CODE}"
echo "  control: 127.0.0.1:${LOCAL_CTRL_PORT} -> guest:${REMOTE_CTRL_PORT}"
echo "  passive: ${PASV_MIN}-${PASV_MAX}"
echo ""
echo "  FTP client settings:"
echo "    Host: 127.0.0.1"
echo "    Port: ${LOCAL_CTRL_PORT}"
echo "    Passive mode: ON"
echo "    Passive ports: ${PASV_MIN}-${PASV_MAX}"
echo ""
echo "  Ctrl-C to stop all listeners"
echo "──────────────────────────────────────────────────────"

start_listener "$LOCAL_CTRL_PORT" "$REMOTE_CTRL_PORT"

p="$PASV_MIN"
while [ "$p" -le "$PASV_MAX" ]; do
    start_listener "$p" "$p"
    p=$((p + 1))
done

# Keep script alive while child listeners run.
while :; do
    sleep 30
done
