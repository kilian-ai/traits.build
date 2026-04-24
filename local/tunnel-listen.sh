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

if ! command -v websocat >/dev/null 2>&1; then
    echo "[tunnel-listen] websocat not found — install via: brew install websocat" >&2
    exit 1
fi

URL="wss://tunnel.traits.build/port/client?code=${CODE}&port=${REMOTE_PORT}"

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
