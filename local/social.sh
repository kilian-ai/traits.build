#!/bin/sh
# social.sh — Nostr-backed public folder follow/sync for v86 Alpine guests.
#
# Lays out the canonical guest convention:
#   /mnt/host/public/                     ← what YOU publish
#   /mnt/host/.nsec                       ← your private key (hex)
#   /mnt/host/.npub                       ← cached bech32 pubkey
#   /mnt/host/.social.tunnel              ← cached base URL for serving public/
#   /mnt/host/following/.list             ← npubs you follow (one per line)
#   /mnt/host/following/<npub>/           ← mirrored content from each followed user
#
# Crypto goes through traits-build.fly.dev REST (social.nostr trait).
# Relay I/O via websocat. File mirror via wget.
#
# Usage:
#   social.sh init                  # generate keypair (or import: social.sh init <nsec1...>)
#   social.sh pubkey                # show your npub
#   social.sh tunnel-up [ports...]  # start tunnel-up.sh (default: 8080); cache base_url
#   social.sh publish               # build manifest of /mnt/host/public, sign, push to relays
#   social.sh follow <npub>         # add to follow list, create dir
#   social.sh unfollow <npub> [--purge]
#   social.sh list                  # show follow list
#   social.sh sync                  # one-shot: pull each followed user's manifest + mirror files
#   social.sh sync --watch [N]      # loop forever, default 60s
#   social.sh search <query>        # NIP-50 keyword search across relays (kind 0 profiles)
#
# Config (env overrides):
#   SOCIAL_API     default: https://traits-build.fly.dev/traits/social/nostr
#   SOCIAL_RELAYS  default: wss://relay.damus.io wss://nos.lol wss://relay.nostr.band
#   SOCIAL_HOME    default: /mnt/host

set -eu

API="${SOCIAL_API:-https://traits-build.fly.dev/traits/social/nostr}"
RELAYS="${SOCIAL_RELAYS:-wss://relay.damus.io wss://nos.lol wss://relay.nostr.band}"
HOME_DIR="${SOCIAL_HOME:-/mnt/host}"
PUBLIC_DIR="$HOME_DIR/public"
FOLLOW_DIR="$HOME_DIR/following"
NSEC_FILE="$HOME_DIR/.nsec"
NPUB_FILE="$HOME_DIR/.npub"
TUNNEL_FILE="$HOME_DIR/.social.tunnel"
FOLLOW_LIST="$FOLLOW_DIR/.list"

die() { echo "social: $*" >&2; exit 1; }
log() { echo "[social] $*" >&2; }

need() {
    for tool in "$@"; do
        command -v "$tool" >/dev/null 2>&1 || die "missing tool: $tool (try: apk add $tool)"
    done
}

ensure_dirs() {
    mkdir -p "$PUBLIC_DIR" "$FOLLOW_DIR"
    [ -f "$FOLLOW_LIST" ] || : > "$FOLLOW_LIST"
}

# Call social.nostr trait via REST. Args: action arg1 arg2 ...
api_call() {
    need curl
    action="$1"; shift
    # Build args array as JSON
    args="\"$action\""
    for a in "$@"; do
        # JSON-escape: backslash, quote, control chars
        esc=$(printf '%s' "$a" | sed 's/\\/\\\\/g; s/"/\\"/g')
        args="$args,\"$esc\""
    done
    body="{\"args\":[$args]}"
    curl -sS -X POST "$API" -H 'Content-Type: application/json' -d "$body"
}

# Extract a JSON field via jq if available, else crude grep fallback.
json_field() {
    if command -v jq >/dev/null 2>&1; then
        jq -r "$1 // empty" 2>/dev/null
    else
        # crude: only handles top-level "key":"value" strings
        key=$(printf '%s' "$1" | sed 's/^\.//')
        grep -o "\"$key\":\"[^\"]*\"" | head -1 | sed 's/^[^:]*:"//; s/"$//'
    fi
}

cmd_init() {
    ensure_dirs
    if [ -n "${1:-}" ]; then
        # Import provided nsec
        log "importing nsec..."
        resp=$(api_call import_nsec "$1") || die "API error: $resp"
    else
        log "generating new keypair..."
        resp=$(api_call keygen) || die "API error: $resp"
    fi
    nsec=$(printf '%s' "$resp" | json_field '.nsec')
    npub=$(printf '%s' "$resp" | json_field '.npub')
    [ -n "$nsec" ] || die "no nsec in response: $resp"
    [ -n "$npub" ] || die "no npub in response: $resp"
    printf '%s\n' "$nsec" > "$NSEC_FILE"
    chmod 600 "$NSEC_FILE"
    printf '%s\n' "$npub" > "$NPUB_FILE"
    echo "npub: $npub"
    echo "nsec stored at: $NSEC_FILE"
}

cmd_pubkey() {
    [ -f "$NPUB_FILE" ] && { cat "$NPUB_FILE"; return; }
    [ -f "$NSEC_FILE" ] || die "no identity yet — run 'social.sh init'"
    nsec=$(cat "$NSEC_FILE")
    resp=$(api_call pubkey "$nsec")
    npub=$(printf '%s' "$resp" | json_field '.npub')
    printf '%s\n' "$npub" > "$NPUB_FILE"
    echo "$npub"
}

cmd_tunnel_up() {
    need curl
    ensure_dirs
    ports="${*:-8080}"
    log "starting tunnel for ports: $ports"
    # tunnel-up.sh prints CODE; capture it
    tmp=$(mktemp /tmp/social-tunnel.XXXXXX)
    sh -c "curl -sS https://www.traits.build/local/tunnel-up.sh | sh -s -- $ports" 2>&1 | tee "$tmp" &
    sleep 6
    code=$(grep -oE 'CODE[: =]+[A-Z0-9]{4}' "$tmp" | head -1 | grep -oE '[A-Z0-9]{4}$' || true)
    if [ -z "$code" ]; then
        log "could not parse tunnel code; check output above"
        rm -f "$tmp"
        return 1
    fi
    base_url="https://tunnel.traits.build/port/http/$code/8080"
    printf '%s\n' "$base_url" > "$TUNNEL_FILE"
    rm -f "$tmp"
    echo "tunnel code: $code"
    echo "base_url:    $base_url"
}

# Walk PUBLIC_DIR, emit JSON manifest content.
build_manifest_content() {
    need sha256sum
    base="${1:-}"
    printf '{"v":1'
    [ -n "$base" ] && printf ',"base":"%s"' "$base"
    printf ',"files":['
    first=1
    if [ -d "$PUBLIC_DIR" ]; then
        # POSIX find + sort for stable order
        find "$PUBLIC_DIR" -type f 2>/dev/null | LC_ALL=C sort | while IFS= read -r f; do
            rel=${f#"$PUBLIC_DIR"/}
            sz=$(wc -c < "$f" | tr -d ' ')
            sh=$(sha256sum < "$f" | cut -d' ' -f1)
            [ "$first" -eq 0 ] && printf ','
            # JSON-escape rel path (basic: backslash + quote)
            relesc=$(printf '%s' "$rel" | sed 's/\\/\\\\/g; s/"/\\"/g')
            printf '{"p":"%s","h":"sha256:%s","s":%s}' "$relesc" "$sh" "$sz"
            first=0
        done
    fi
    printf ']}'
}

cmd_publish() {
    need websocat
    [ -f "$NSEC_FILE" ] || die "no identity yet — run 'social.sh init'"
    nsec=$(cat "$NSEC_FILE")
    base=""
    [ -f "$TUNNEL_FILE" ] && base=$(cat "$TUNNEL_FILE")
    log "building manifest for $PUBLIC_DIR ${base:+(base=$base)}"
    content=$(build_manifest_content "$base")
    bytes=$(printf '%s' "$content" | wc -c | tr -d ' ')
    log "manifest: $bytes bytes"

    # Build tags: [["d","public-folder"]] + optional ["r", base_url]
    if [ -n "$base" ]; then
        tags='[["d","public-folder"],["r","'"$base"'"]]'
    else
        tags='[["d","public-folder"]]'
    fi
    now=$(date +%s)
    log "signing event (kind 30000)..."
    resp=$(api_call sign_event "$nsec" "30000" "$content" "$tags" "$now")
    event=$(printf '%s' "$resp" | (command -v jq >/dev/null 2>&1 && jq -c '.event' || sed -n 's/.*"event":\({.*}\).*/\1/p'))
    [ -n "$event" ] && [ "$event" != "null" ] || die "sign failed: $resp"

    msg='["EVENT",'"$event"']'
    for relay in $RELAYS; do
        log "→ $relay"
        ( printf '%s\n' "$msg"; sleep 2 ) | websocat -n0 - "$relay" 2>&1 | head -3 || true
    done
    npub=$(cmd_pubkey)
    echo "published as $npub"
}

cmd_follow() {
    npub="${1:?usage: follow <npub>}"
    case "$npub" in npub1*) ;; *) die "expected npub1...";; esac
    ensure_dirs
    if grep -qx "$npub" "$FOLLOW_LIST" 2>/dev/null; then
        echo "already following: $npub"
    else
        printf '%s\n' "$npub" >> "$FOLLOW_LIST"
        mkdir -p "$FOLLOW_DIR/$npub"
        echo "now following: $npub"
    fi
}

cmd_unfollow() {
    npub="${1:?usage: unfollow <npub> [--purge]}"
    purge=0
    [ "${2:-}" = "--purge" ] && purge=1
    if [ -f "$FOLLOW_LIST" ]; then
        tmp=$(mktemp); grep -vx "$npub" "$FOLLOW_LIST" > "$tmp" || true
        mv "$tmp" "$FOLLOW_LIST"
    fi
    [ "$purge" -eq 1 ] && rm -rf "$FOLLOW_DIR/$npub"
    echo "unfollowed: $npub"
}

cmd_list() {
    [ -f "$FOLLOW_LIST" ] || { echo "(empty)"; return; }
    n=$(wc -l < "$FOLLOW_LIST" | tr -d ' ')
    echo "following $n:"
    cat "$FOLLOW_LIST"
}

# Query a relay for the latest kind-30000 d=public-folder event from <hex_pk>.
# Echoes the matching event JSON or empty.
relay_fetch_manifest() {
    relay="$1"
    hex_pk="$2"
    sub="sub-$$"
    filter='{"kinds":[30000],"authors":["'"$hex_pk"'"],"#d":["public-folder"],"limit":1}'
    req='["REQ","'"$sub"'",'"$filter"']'
    ( printf '%s\n' "$req"; sleep 4 ) | websocat -n0 - "$relay" 2>/dev/null \
        | grep -E '^\["EVENT",' | head -1
}

cmd_sync_one() {
    npub="$1"
    log "sync: $npub"
    # Decode npub → hex
    resp=$(api_call decode_npub "$npub")
    hex=$(printf '%s' "$resp" | json_field '.pubkey_hex')
    [ -n "$hex" ] || { log "  decode failed: $resp"; return 1; }

    # Try each relay until we get an event
    raw=""
    for relay in $RELAYS; do
        log "  query $relay"
        raw=$(relay_fetch_manifest "$relay" "$hex" || true)
        [ -n "$raw" ] && break
    done
    [ -n "$raw" ] || { log "  no manifest event found"; return 1; }

    if ! command -v jq >/dev/null 2>&1; then
        log "  jq required for sync (apk add jq)"; return 1
    fi
    content=$(printf '%s' "$raw" | jq -r '.[2].content')
    base=$(printf '%s' "$raw" | jq -r '.[2].tags[] | select(.[0]=="r") | .[1]' | head -1)
    if [ -z "$base" ] || [ "$base" = "null" ]; then
        # Try base from manifest content itself
        base=$(printf '%s' "$content" | jq -r '.base // empty')
    fi
    if [ -z "$base" ]; then
        log "  manifest has no base URL — skipping file mirror"
        return 0
    fi
    log "  base_url: $base"

    dest="$FOLLOW_DIR/$npub"
    mkdir -p "$dest"
    # Save the raw manifest event for inspection
    printf '%s\n' "$raw" > "$dest/.manifest.json"

    # Iterate files
    file_count=$(printf '%s' "$content" | jq '.files | length')
    log "  $file_count files"
    i=0
    while [ "$i" -lt "$file_count" ]; do
        p=$(printf '%s' "$content" | jq -r ".files[$i].p")
        h=$(printf '%s' "$content" | jq -r ".files[$i].h")
        local_path="$dest/$p"
        local_dir=$(dirname "$local_path")
        mkdir -p "$local_dir"
        # Skip if already correct hash
        if [ -f "$local_path" ]; then
            existing="sha256:$(sha256sum < "$local_path" | cut -d' ' -f1)"
            if [ "$existing" = "$h" ]; then
                i=$((i+1)); continue
            fi
        fi
        url="$base/$p"
        log "    fetch $p"
        wget -qO "$local_path.tmp" "$url" && mv "$local_path.tmp" "$local_path" \
            || { log "    failed: $url"; rm -f "$local_path.tmp"; }
        i=$((i+1))
    done
    log "  done: $npub"
}

cmd_sync() {
    need websocat
    ensure_dirs
    watch=0; interval=60
    if [ "${1:-}" = "--watch" ]; then
        watch=1
        [ -n "${2:-}" ] && interval="$2"
    fi
    while :; do
        if [ -s "$FOLLOW_LIST" ]; then
            while IFS= read -r npub; do
                [ -z "$npub" ] && continue
                cmd_sync_one "$npub" || true
            done < "$FOLLOW_LIST"
        else
            log "follow list empty"
        fi
        [ "$watch" -eq 0 ] && break
        log "sleep ${interval}s..."
        sleep "$interval"
    done
}

cmd_search() {
    need websocat
    q="${1:?usage: search <query>}"
    sub="search-$$"
    # NIP-50 search filter (relays that support it: relay.nostr.band)
    filter='{"kinds":[0],"search":"'"$q"'","limit":10}'
    req='["REQ","'"$sub"'",'"$filter"']'
    log "searching for: $q"
    for relay in $RELAYS; do
        ( printf '%s\n' "$req"; sleep 3 ) | websocat -n0 - "$relay" 2>/dev/null \
            | grep -E '^\["EVENT",' | while IFS= read -r line; do
                if command -v jq >/dev/null 2>&1; then
                    pk=$(printf '%s' "$line" | jq -r '.[2].pubkey')
                    name=$(printf '%s' "$line" | jq -r '.[2].content' | jq -r '.name // .display_name // "?"' 2>/dev/null || echo "?")
                    npub=$(api_call encode_npub "$pk" | json_field '.npub')
                    echo "$npub  $name"
                else
                    echo "$line"
                fi
            done
    done | sort -u
}

cmd_help() {
    sed -n '2,40p' "$0"
}

case "${1:-help}" in
    init)        shift; cmd_init "$@";;
    pubkey)      shift; cmd_pubkey;;
    tunnel-up)   shift; cmd_tunnel_up "$@";;
    publish)     shift; cmd_publish;;
    follow)      shift; cmd_follow "$@";;
    unfollow)    shift; cmd_unfollow "$@";;
    list|ls)     shift; cmd_list;;
    sync)        shift; cmd_sync "$@";;
    search)      shift; cmd_search "$@";;
    help|-h|--help) cmd_help;;
    *) die "unknown command: $1 (try: social.sh help)";;
esac
