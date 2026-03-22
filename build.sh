#!/bin/bash
# Build the Traits kernel
# Usage: ./build.sh [--clean]
set -euo pipefail
cd "$(dirname "$0")"

if [[ "${1:-}" == "--clean" ]]; then
    echo "Cleaning build..."
    cargo clean
fi

echo "Building traits kernel..."
cargo build --release

BIN="target/release/traits"
if [[ -f "$BIN" ]]; then
    SIZE=$(du -h "$BIN" | cut -f1)
    echo ""
    echo "Built: $BIN ($SIZE)"
    echo "Traits: $("$BIN" list 2>/dev/null | wc -l | tr -d ' ') registered"
else
    echo "Build failed — no binary produced"
    exit 1
fi
