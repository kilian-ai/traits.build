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
cargo build --release --workspace

BIN="target/release/traits"
if [[ ! -f "$BIN" ]]; then
    echo "Build failed — no binary produced"
    exit 1
fi

SIZE=$(du -h "$BIN" | cut -f1)
echo ""
echo "Built: $BIN ($SIZE)"

# ── Copy cdylib outputs to trait directories ──
# The dylib_loader expects lib<dirname>.dylib next to each .trait.toml
EXT="dylib"
[[ "$(uname)" == "Linux" ]] && EXT="so"

copy_dylib() {
    local crate_name="$1" trait_dir="$2" dir_name="$3"
    local src="target/release/lib${crate_name}.${EXT}"
    local dst="${trait_dir}/lib${dir_name}.${EXT}"
    if [[ -f "$src" ]]; then
        cp "$src" "$dst"
        echo "  Copied $src → $dst"
    fi
}

echo "Copying dylibs..."
copy_dylib "trait_www_traits_build" "traits/www/traits/build" "build"
copy_dylib "trait_sys_checksum"     "traits/sys/checksum"     "checksum"
copy_dylib "trait_sys_ps"           "traits/sys/ps"           "ps"

echo "Traits: $("$BIN" list 2>/dev/null | wc -l | tr -d ' ') registered"
