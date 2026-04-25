#!/bin/sh
# tunnel-listen.sh — Open a local TCP port that forwards to a tunnel code.
# Enables plain `ssh -p PORT localhost`, `scp -P PORT ...`, rsync, etc.
#
# Usage: tunnel-listen.sh CODE [local_port] [remote_port]
#   local_port   defaults to 2222
#   remote_port  defaults to 22 (the port registered on the guest)
#
# One-liner:
#   sh <(curl -sS https://www.traits.build/local/tunnel-listen.sh) CODE
#
# Then in another terminal:
#   ssh -p 2222 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null root@localhost
#   scp -P 2222 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null file root@localhost:/tmp/
#
# Requires: websocat (brew install websocat)

set -u

CODE="${1:?usage: tunnel-listen.sh CODE [local_port] [remote_port]}"
LOCAL_PORT="${2:-2222}"
REMOTE_PORT="${3:-22}"

# Override via env: TUNNEL_WS=ws://localhost:8787 sh tunnel-listen.sh CODE
TUNNEL_WS="${TUNNEL_WS:-wss://tunnel.traits.build}"

if ! command -v websocat >/dev/null 2>&1; then
    echo "[tunnel-listen] websocat not found — install via: brew install websocat" >&2
    exit 1
fi

relay_status_json() {
    curl -fsS "https://${TUNNEL_WS#*://}/port/status?code=${CODE}" 2>/dev/null || true
}

has_registered_port() {
    check_port="$1"
    status_json="$2"
    printf '%s' "$status_json" | grep -q "\"registered_ports\"" || return 1
    printf '%s' "$status_json" | grep -q "\[$check_port\(,\|]\)" && return 0
    printf '%s' "$status_json" | grep -q ",$check_port\(,\|]\)" && return 0
    return 1
}

has_guest_port() {
    check_port="$1"
    status_json="$2"
    printf '%s' "$status_json" | grep -q "\"guest_ports\"" || return 1
    printf '%s' "$status_json" | grep -q "\"guest_ports\"[[:space:]]*:[[:space:]]*\[[^]]*$check_port" && return 0
    return 1
}

status_json="$(relay_status_json)"
if [ -z "$status_json" ]; then
    echo "[tunnel-listen] unable to fetch relay status for code ${CODE}" >&2
    echo "[tunnel-listen] verify the code and relay endpoint, then retry" >&2
    exit 1
fi

if printf '%s' "$status_json" | grep -q '"active"[[:space:]]*:[[:space:]]*false'; then
    echo "[tunnel-listen] code ${CODE} is inactive/expired" >&2
    echo "[tunnel-listen] generate a fresh code on the guest via tunnel-up.sh and retry" >&2
    exit 1
fi

if ! has_registered_port "$REMOTE_PORT" "$status_json"; then
    echo "[tunnel-listen] code ${CODE} is missing registered guest port ${REMOTE_PORT}" >&2
    echo "[tunnel-listen] restart guest tunnel-up with port ${REMOTE_PORT} and retry" >&2
    exit 1
fi

if ! has_guest_port "$REMOTE_PORT" "$status_json"; then
    echo "[tunnel-listen] warning: guest is not currently connected on port ${REMOTE_PORT}" >&2
    echo "[tunnel-listen] listener will start, but connections may fail until guest bridge is up" >&2
fi

URL="${TUNNEL_WS}/port/client?code=${CODE}&port=${REMOTE_PORT}"

echo "──────────────────────────────────────────────────────"
echo "  tunnel: $CODE  (guest port $REMOTE_PORT)"
echo "  local listener: 127.0.0.1:${LOCAL_PORT}"
echo ""
echo "  In another terminal:"
if [ "$REMOTE_PORT" = "22" ]; then
    echo "    ssh -p ${LOCAL_PORT} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null root@localhost"
    echo "    scp -P ${LOCAL_PORT} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null FILE root@localhost:/tmp/"
    echo "    sftp -P ${LOCAL_PORT} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null root@localhost"
else
    echo "    nc localhost ${LOCAL_PORT}   # or any client that speaks port ${REMOTE_PORT}"
fi
echo ""
echo "  Ctrl-C to stop."
echo "──────────────────────────────────────────────────────"

# websocat opens a local TCP listener that forwards to the tunnel WS.
# Each client connection spawns a fresh WebSocket. The guest-side bridge
# respawns between connections (see tunnel-up.sh), so serial SSH/SCP
# sessions work without restarting anything.
exec websocat --binary "tcp-l:127.0.0.1:${LOCAL_PORT}" "$URL"
