#!/bin/sh
# tunnel-up.sh — Expose local TCP ports through tunnel.traits.build
# Usage: tunnel-up.sh [port1] [port2] ...
# Defaults: sshd(22), public-http(8080 → ~/public), syncthing-sync(22000),
#           syncthing-gui(8384)
#
# Requires: websocat, curl
# Install: apk add --no-cache websocat curl
#
# After running, SSH access:
#   sh <(curl -sS https://www.traits.build/local/tunnel-ssh.sh) CODE
#
# ~/public browsing (any HTTPS client):
#   curl  https://tunnel.traits.build/port/http/CODE/8080/
#   open  https://www.traits.build/#/viewer?code=CODE

set -u

# Survive shell logout / backgrounding. Without this, putting the script
# under '&' and exiting the parent terminal sends SIGHUP and kills every
# bridge subshell within seconds.
trap '' HUP

# Override via env: TUNNEL_BASE=http://localhost:8787 sh tunnel-up.sh
TUNNEL_BASE="${TUNNEL_BASE:-https://traits-build-tunnel.fly.dev}"
# Derive default WS URL from TUNNEL_BASE (http→ws, https→wss).
if [ -z "${TUNNEL_WS:-}" ]; then
    case "$TUNNEL_BASE" in
        https://*) TUNNEL_WS="wss://${TUNNEL_BASE#https://}" ;;
        http://*)  TUNNEL_WS="ws://${TUNNEL_BASE#http://}" ;;
        *)         TUNNEL_WS="$TUNNEL_BASE" ;;
    esac
fi
PORTS="${*:-22 8080 22000 8384}"

# Install websocat if missing
if ! command -v websocat >/dev/null 2>&1; then
    echo "[tunnel] installing websocat..."
    apk add --no-cache websocat curl >/dev/null 2>&1 \
        || { echo "[tunnel] apk failed — install websocat manually"; exit 1; }
fi

# Ensure loopback is up (websocat → tcp:127.0.0.1:PORT needs it)
if ! ip addr show lo 2>/dev/null | grep -q 'inet 127\.'; then
    ifconfig lo 127.0.0.1 up 2>/dev/null || ip link set lo up 2>/dev/null
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

# Seed ~/public/cgi-bin/ls — JSON listing endpoint for the viewer.
# Must run even when httpd is already up (user may have started it
# before upgrading this script), because BusyBox httpd picks up new
# cgi-bin scripts without a restart.
seed_public_cgi() {
    local dir="${HOME:-/root}/public"
    mkdir -p "$dir/cgi-bin"
    cat > "$dir/cgi-bin/ls" <<'CGI'
#!/bin/sh
printf 'Content-Type: application/json\r\n'
printf 'Access-Control-Allow-Origin: *\r\n'
printf '\r\n'
qs="${QUERY_STRING:-}"
sub=$(printf '%s' "$qs" | awk -v RS='&' -F= '$1=="dir"{print $2; exit}')
sub="${sub:-/}"
dec=$(printf '%s' "$sub" | sed 's/+/ /g; s/%\(..\)/\\x\1/g')
sub=$(printf '%b' "$dec")
case "$sub" in *..*) sub="/" ;; esac
sub=$(printf '%s' "$sub" | sed 's|//*|/|g')
[ "${sub#/}" = "$sub" ] && sub="/$sub"
base="${HOME:-/root}/public"
full="$base${sub%/}"
[ "$sub" = "/" ] && full="$base"
if [ ! -d "$full" ]; then
    printf '{"error":"not a directory","dir":"%s"}' "$sub"
    exit 0
fi
printf '{"dir":"'
printf '%s' "$sub" | sed 's/\\/\\\\/g; s/"/\\"/g'
printf '","entries":['
first=1
for f in "$full"/.[!.]* "$full"/..?* "$full"/*; do
    [ -e "$f" ] || continue
    name="${f##*/}"
    case "$name" in .|..|.\*|\*|\.\[\!\.\]\*|\.\.?\*) continue ;; esac
    if [ -d "$f" ]; then
        t=dir
        sz=0
    else
        t=file
        sz=$(wc -c < "$f" 2>/dev/null | tr -d ' ' || echo 0)
    fi
    nj=$(printf '%s' "$name" | sed 's/\\/\\\\/g; s/"/\\"/g')
    [ $first -eq 1 ] || printf ','
    first=0
    printf '{"name":"%s","type":"%s","size":%s}' "$nj" "$t" "$sz"
done
printf ']}'
CGI
    chmod +x "$dir/cgi-bin/ls"
}

# Auto-start busybox httpd serving ~/public on port 8080 if requested + no listener.
auto_start_httpd() {
    local dir="${HOME:-/root}/public"
    mkdir -p "$dir"
    if ! command -v httpd >/dev/null 2>&1; then
        # BusyBox httpd is usually built-in; if not, install busybox-extras.
        apk add --no-cache busybox-extras >/dev/null 2>&1 || true
    fi
    if ! command -v httpd >/dev/null 2>&1; then
        echo "[tunnel]   httpd not available — skipping public viewer"
        return 1
    fi
    # Seed a minimal viewer + index page if ~/public is empty.
    if [ ! -e "$dir/index.html" ] && [ -z "$(ls -A "$dir" 2>/dev/null)" ]; then
        cat > "$dir/index.html" <<'HTML'
<!DOCTYPE html><meta charset="utf-8"><title>~/public</title>
<style>body{font:14px ui-monospace,monospace;padding:2em;background:#0b0d10;color:#d5d8dc}
a{color:#4ade80;text-decoration:none}a:hover{text-decoration:underline}</style>
<h1>~/public</h1>
<p>Drop files here to share them via the tunnel.</p>
<p>Served from <code>$HOME/public</code> in the guest.</p>
HTML
    fi
    # CGI listing endpoint: BusyBox httpd serves index.html when present, so
    # the client can't get an auto-index of the root. /cgi-bin/ls?dir=/sub
    # returns a JSON listing so the viewer can always show files even when
    # index.html exists.
    seed_public_cgi
    echo "[tunnel]   launching httpd -h $dir -p 8080 ..."
    # -f = foreground mode when backgrounded manually with &
    # -h = home directory (serves static files + auto-index)
    httpd -p 127.0.0.1:8080 -h "$dir" 2>&1 | head -3
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
    # Always (re)seed the CGI listing endpoint when 8080 is requested — even
    # if httpd was already running from a previous tunnel-up.sh session and
    # auto_start_httpd would be skipped, BusyBox httpd picks up new cgi-bin
    # scripts on the next request without needing a restart.
    if [ "$PORT" = "8080" ]; then
        mkdir -p "${HOME:-/root}/public" 2>/dev/null
        seed_public_cgi
        echo "[tunnel] port 8080 — seeded ~/public/cgi-bin/ls (JSON listing endpoint)"
    fi
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
        elif [ "$PORT" = "8080" ]; then
            echo "[tunnel] port 8080 — no listener, starting httpd on ~/public..."
            auto_start_httpd
            i=0
            while [ $i -lt 15 ]; do
                port_listening 8080 && break
                i=$((i+1)); sleep 0.2 2>/dev/null || sleep 1
            done
        fi
    fi
    if ! port_listening "$PORT"; then
        echo "[tunnel] port $PORT — no local listener, skipping bridge"
        continue
    fi
    WS_URL="${TUNNEL_WS}/port/guest?code=${CODE}&port=${PORT}"
    # Respawn loop: each client disconnect tears down the bridge; restart
    # so the next SSH connection gets a fresh TCP to sshd.
    # --ping-interval 25: keep the WS alive across NAT/CDN idle timeouts
    #   (Fly's edge proxy and many corporate NATs drop idle WS at ~60s).
    # trap '' HUP: subshell must independently ignore SIGHUP so backgrounding
    #   tunnel-up.sh & + closing the terminal does not kill bridges.
    (
        trap '' HUP
        while :; do
            websocat --binary --ping-interval 25 "$WS_URL" "tcp:127.0.0.1:${PORT}" </dev/null
            echo "[tunnel] bridge for port ${PORT} exited; respawning in 1s"
            sleep 1
        done
    ) >> "/tmp/tunnel-${PORT}.log" 2>&1 &
    BPID=$!
    echo "[tunnel] port $PORT → bridge PID $BPID  log: /tmp/tunnel-${PORT}.log"
done

echo ""
echo "──────────────────────────────────────────────────────"
echo "  Connect from any machine:"
echo ""
for PORT in $PORTS; do
    if [ "$PORT" = "22" ]; then
        echo "  SSH (one-liner, shell-safe):"
        echo "    sh <(curl -sS https://www.traits.build/local/tunnel-ssh.sh) ${CODE}"
        echo ""
        echo "  Plain ssh/scp/sftp via local TCP listener:"
        echo "    sh <(curl -sS https://www.traits.build/local/tunnel-listen.sh) ${CODE}"
        echo "    ssh  -p 2222 root@localhost"
        echo ""
    fi
    if [ "$PORT" = "8080" ]; then
        echo "  ~/public browser viewer:"
        echo "    https://www.traits.build/#/viewer?code=${CODE}"
        echo ""
        echo "  ~/public direct HTTP proxy:"
        echo "    curl ${TUNNEL_BASE}/port/http/${CODE}/8080/"
        echo ""
    fi
    echo "  WebSocket raw (port ${PORT}): '${TUNNEL_WS}/port/client?code=${CODE}&port=${PORT}'"
done
echo ""
echo "  Status:  curl -s '${TUNNEL_BASE}/port/status?code=${CODE}'"
echo "  Debug:   curl -s '${TUNNEL_BASE}/port/debug?code=${CODE}'"
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
