#!/bin/bash
# fast-deploy.sh — Deploy to Fly.io without rebuilding a Docker image.
#
# Compiles for linux/amd64 inside a Docker container with persistent caches,
# then uploads the binary via `fly ssh sftp` and restarts the machine.
#
# First run:  ~6 min (compiles all deps, saved in Docker volumes)
# Code-only:  ~1-2 min (deps cached, only your code recompiles)
# Upload:     ~15 sec
#
# Usage:
#   ./scripts/fast-deploy.sh           # build + upload + restart
#   ./scripts/fast-deploy.sh --upload  # upload last binary + restart (skip build)
#
# Prerequisites: Docker running, fly CLI authenticated

set -euo pipefail
cd "$(dirname "$0")/.."

APP="${FLY_APP:-polygrait-api}"
REMOTE_BIN="/data/traits"
TMP_BIN="/tmp/traits-linux-amd64"
BUILD_VOL="traits-cargo-target"
REG_VOL="traits-cargo-registry"

# ── Resolve machine ──────────────────────────────────────────────────────
MACHINE_ID=$(fly machines list -a "$APP" --json 2>/dev/null \
    | python3 -c "import sys,json; m=json.load(sys.stdin); print(m[0]['id'] if m else '')" 2>/dev/null)

[ -z "$MACHINE_ID" ] && { echo "Error: no machines for $APP"; exit 1; }
echo "Machine: $MACHINE_ID"

# ── Build ────────────────────────────────────────────────────────────────
if [ "${1:-}" != "--upload" ]; then
    echo "==> Building linux/amd64 binary..."
    docker volume create "$BUILD_VOL" >/dev/null 2>&1 || true
    docker volume create "$REG_VOL"   >/dev/null 2>&1 || true

    docker run --rm --platform linux/amd64 \
        -v "$(pwd):/src:ro" \
        -v "$BUILD_VOL:/cargo-target" \
        -v "$REG_VOL:/usr/local/cargo/registry" \
        -v "/tmp:/out" \
        -e CARGO_TARGET_DIR=/cargo-target \
        rust:latest \
        sh -c 'echo "[copy] Copying source (excluding target/)..." && mkdir -p /build && tar -C /src --exclude=./target -cf - . | tar -C /build -xf - && echo "[build] Compiling..." && cd /build && cargo build --release 2>&1 && echo "[done] Copying binary..." && cp /cargo-target/release/traits /out/traits-linux-amd64'

    echo "==> Built: $(du -h "$TMP_BIN" | cut -f1)"
fi

[ ! -f "$TMP_BIN" ] && { echo "Error: no binary at $TMP_BIN"; exit 1; }

# ── Upload + restart ─────────────────────────────────────────────────────
echo "==> Uploading to $APP..."
fly ssh console -a "$APP" -C "rm -f $REMOTE_BIN" 2>/dev/null || true
echo "put $TMP_BIN $REMOTE_BIN" | fly ssh sftp shell -a "$APP"

echo "==> Restarting machine..."
fly ssh console -a "$APP" -C "chmod +x $REMOTE_BIN"
fly machines restart "$MACHINE_ID" -a "$APP" --skip-health-checks 2>/dev/null || true

echo "==> Waiting for health..."
sleep 5
for i in $(seq 1 12); do
    H=$(curl -sf "https://traits.build/health" 2>/dev/null || true)
    if [ -n "$H" ]; then
        echo "==> Healthy!"
        echo "$H" | python3 -m json.tool
        exit 0
    fi
    echo "    retry $i/12..."
    sleep 5
done
echo "Warning: health check didn't pass after 60s. Check: fly logs -a $APP"
