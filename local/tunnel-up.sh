#!/bin/sh
# tunnel-up.sh — Expose local TCP ports through tunnel.traits.build
# Usage: tunnel-up.sh [port1] [port2] ...
# Defaults: sshd(22), public-http(8080 → ~/public), syncthing-sync(22000),
#           syncthing-gui(8384)
# FTP note: if port 21 is requested, this script auto-adds a passive range
# (default 30000-30010) so FTP data channels can traverse relay too.
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
FTP_PASV_MIN="${FTP_PASV_MIN:-30000}"
FTP_PASV_MAX="${FTP_PASV_MAX:-30010}"

has_port() {
    local target="$1"
    local existing_port
    for existing_port in $PORTS; do
        [ "$existing_port" = "$target" ] && return 0
    done
    return 1
}

append_port() {
    local candidate_port="$1"
    has_port "$candidate_port" || PORTS="$PORTS $candidate_port"
}

add_ftp_passive_ports_if_needed() {
    has_port 21 || return 0
    local pasv_port="$FTP_PASV_MIN"
    while [ "$pasv_port" -le "$FTP_PASV_MAX" ]; do
        append_port "$pasv_port"
        pasv_port=$((pasv_port + 1))
    done
}

add_ftp_passive_ports_if_needed

is_ftp_passive_port() {
    local candidate_port="$1"
    has_port 21 || return 1
    [ "$candidate_port" -ge "$FTP_PASV_MIN" ] && [ "$candidate_port" -le "$FTP_PASV_MAX" ]
}

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
# ~/public is symlinked to /mnt/host/public so files are shared between the
# guest user shell, SFTP uploads (whose home is /root), the v86 host browser
# (which reads fs9p directly), and the public viewer — all looking at the
# same 9P-backed directory.
auto_start_httpd() {
    local home="${HOME:-/root}"
    local link="$home/public"
    local target="/mnt/host/public"
    # Ensure the shared 9P dir exists. If /mnt/host isn't actually a 9P mount
    # (no host bridge), fall back to a plain $HOME/public dir so the viewer
    # still works in headless test runs.
    if grep -q ' /mnt/host ' /proc/mounts 2>/dev/null; then
        mkdir -p "$target" 2>/dev/null
        # Replace $HOME/public with a symlink to /mnt/host/public unless
        # it's already correctly linked. If a real dir exists with content,
        # migrate its files into the shared dir before replacing.
        if [ -L "$link" ]; then
            [ "$(readlink "$link")" = "$target" ] || { rm -f "$link"; ln -s "$target" "$link"; }
        elif [ -d "$link" ]; then
            if [ -n "$(ls -A "$link" 2>/dev/null)" ]; then
                cp -a "$link"/. "$target"/ 2>/dev/null || true
            fi
            rm -rf "$link" 2>/dev/null
            ln -s "$target" "$link"
        else
            ln -s "$target" "$link"
        fi
    else
        mkdir -p "$link"
    fi
    local dir="$link"
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

# Auto-start FTP on port 21 with a fixed passive range so relay can proxy
# both control and data connections.
auto_start_ftpd() {
    if ! command -v vsftpd >/dev/null 2>&1; then
        echo "[tunnel]   installing vsftpd..."
        apk add --no-cache vsftpd >/dev/null 2>&1 || {
            echo "[tunnel]   vsftpd install failed"
            return 1
        }
    fi

    if [ ! -f /etc/vsftpd/vsftpd.conf ] && [ ! -f /etc/vsftpd.conf ]; then
        mkdir -p /etc/vsftpd 2>/dev/null || true
    fi

    cat > /tmp/vsftpd-tunnel.conf <<EOF
listen=YES
listen_address=127.0.0.1
listen_port=21
background=YES
anonymous_enable=NO
local_enable=YES
write_enable=YES
local_umask=022
pasv_enable=YES
pasv_min_port=${FTP_PASV_MIN}
pasv_max_port=${FTP_PASV_MAX}
pasv_address=127.0.0.1
userlist_enable=NO
seccomp_sandbox=NO
xferlog_enable=YES
EOF

    echo "[tunnel]   launching vsftpd (passive ${FTP_PASV_MIN}-${FTP_PASV_MAX})..."
    vsftpd /tmp/vsftpd-tunnel.conf >/tmp/tunnel-ftpd.log 2>&1 || {
        echo "[tunnel]   failed to launch vsftpd (see /tmp/tunnel-ftpd.log)"
        return 1
    }
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
        elif [ "$PORT" = "21" ]; then
            echo "[tunnel] port 21 — no listener, starting ftpd (vsftpd)..."
            auto_start_ftpd
            i=0
            while [ $i -lt 15 ]; do
                port_listening 21 && break
                i=$((i+1)); sleep 0.2 2>/dev/null || sleep 1
            done
        elif is_ftp_passive_port "$PORT"; then
            # FTP passive sockets are opened on demand by vsftpd.
            # Keep relay bridge loops alive even if the port is closed now.
            echo "[tunnel] port $PORT — FTP passive (on-demand), starting bridge loop without pre-listen check"
        fi
    fi
    if ! port_listening "$PORT" && ! is_ftp_passive_port "$PORT"; then
        echo "[tunnel] port $PORT — no local listener, skipping bridge"
        continue
    fi
    WS_URL="${TUNNEL_WS}/port/guest?code=${CODE}&port=${PORT}"
    # POOL_SIZE: how many parallel guest WS bridges to keep open per port.
    #
    # The relay now uses a multi-pair queue: each guest WS sits in a per-port
    # standby pool, and each new TCP client (ssh/sftp/ftp/etc.) pops a fresh
    # bridge from the pool to pair with. This is required for parallel-capable
    # protocols — Filezilla SFTP opens 2+ concurrent SSH connections by
    # default for parallel transfers, and each one needs its own bridge.
    #
    # Default 4 keeps a healthy standby buffer for typical interactive use
    # (ssh + 2 sftp transfers + 1 spare). Each bridge consumed by a client
    # is replaced by the inner respawn loop below within ~100ms.
    #
    # Override with TUNNEL_BRIDGES_PER_PORT=N if you expect many parallel
    # clients per port (HTTP burst, large parallel rsync, many SFTP slots).
    POOL_SIZE="${TUNNEL_BRIDGES_PER_PORT:-4}"
    n=0
    while [ "$n" -lt "$POOL_SIZE" ]; do
        n=$((n+1))
        # --ping-interval 25: keep WS alive across NAT/CDN idle timeouts
        #   (Fly edge proxy and many corporate NATs drop idle WS at ~60s).
        # trap '' HUP: subshell must independently ignore SIGHUP so backgrounding
        #   tunnel-up.sh & + closing the terminal does not kill bridges.
        # No sleep on respawn: lost time = dropped requests during the gap.
        (
            trap '' HUP
            while :; do
                websocat --binary --ping-interval 25 "$WS_URL" "tcp:127.0.0.1:${PORT}" </dev/null
                # tiny jitter to avoid tight loop if TCP refused
                sleep 0.1 2>/dev/null || sleep 1
            done
        ) >> "/tmp/tunnel-${PORT}.log" 2>&1 &
        BPID=$!
        echo "[tunnel] port $PORT → bridge $n/$POOL_SIZE PID $BPID  log: /tmp/tunnel-${PORT}.log"
    done
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
    if [ "$PORT" = "21" ]; then
        echo "  FTP (control):"
        echo "    host: localhost (via local listener helper)"
        echo "    control port: 2121 -> guest:${PORT}"
        echo "    passive range exposed: ${FTP_PASV_MIN}-${FTP_PASV_MAX}"
        echo "    sh <(curl -sS https://www.traits.build/local/tunnel-listen-ftp.sh) ${CODE} 2121 ${PORT} ${FTP_PASV_MIN} ${FTP_PASV_MAX}"
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
    [ -n "${RECLAIM_PID:-}" ] && kill "$RECLAIM_PID" 2>/dev/null
    curl -sS -X POST "$TUNNEL_BASE/port/unregister" \
        -H "Content-Type: application/json" \
        -d "{\"code\":\"$CODE\"}" > /dev/null
    rm -f /tmp/tunnel.code
    echo "[tunnel] done."
    exit 0
' INT TERM

# Reclaim watchdog ────────────────────────────────────────────────────────
# The relay may forget our code at any time:
#   - Fly tunnel-server keeps sessions in memory only; every redeploy
#     wipes them.
#   - Cloudflare Durable Object hibernation can evict idle sessions.
#   - Network partitions / proxy 502s can drop the session view.
# When that happens, the websocat respawn loops keep dialing
# /port/guest?code=... and the relay returns 403 ("code not found"),
# so traffic silently stops.
#
# This loop polls /port/status?code=$CODE and re-POSTs /port/register
# with the SAME {code, ports} body to reclaim it. Both backends accept
# `code` in the register body and idempotently restore the session
# (CF maps code → DO via idFromName(code); Fly stores by the code
# string). The websocat respawn loops then succeed on their next
# iteration without changing $CODE — so SSH/SFTP/HTTP clients holding
# the old code keep working after a Fly redeploy.
#
# Override poll interval with TUNNEL_RECLAIM_INTERVAL=N (seconds, default 20).
RECLAIM_INTERVAL="${TUNNEL_RECLAIM_INTERVAL:-20}"
(
    trap '' HUP
    while :; do
        sleep "$RECLAIM_INTERVAL" 2>/dev/null || sleep 30
        STATUS=$(curl -sS --max-time 5 "$TUNNEL_BASE/port/status?code=$CODE" 2>/dev/null)
        # Both backends report inactive sessions as either:
        #   {"active":false,"error":"code not found"}   (Fly)
        #   {"active":false,...}                         (CF, after eviction)
        # Or status may fail entirely (curl error → empty STATUS) on full
        # relay outage. In all those cases, attempt re-register; if the
        # relay is healthy and we just had a transient curl error, the
        # re-register is a cheap no-op (server returns same code/ports).
        case "$STATUS" in
            *'"active":true'*) continue ;;
        esac
        echo "[tunnel] reclaim: relay forgot $CODE (status=${STATUS:-<no response>}), re-registering ..."
        RECLAIM_RESP=$(curl -sS --max-time 8 -X POST "$TUNNEL_BASE/port/register" \
            -H 'Content-Type: application/json' \
            -d "{\"code\":\"$CODE\",\"ports\":${PORTS_JSON}}" 2>&1)
        case "$RECLAIM_RESP" in
            *'"registered_ports"'*|*'"ok":true'*)
                echo "[tunnel] reclaim: $CODE reclaimed — bridges will reconnect on next respawn"
                ;;
            *)
                echo "[tunnel] reclaim: failed (resp=$RECLAIM_RESP) — will retry in ${RECLAIM_INTERVAL}s"
                ;;
        esac
    done
) &
RECLAIM_PID=$!
echo "[tunnel] reclaim watchdog PID $RECLAIM_PID (every ${RECLAIM_INTERVAL}s)"

# Keep alive (background bridges continue)
while :; do sleep 30; done
