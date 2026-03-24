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
if [[ ! -f "$BIN" ]]; then
    echo "Build failed — no binary produced"
    exit 1
fi

SIZE=$(du -h "$BIN" | cut -f1)
echo ""
echo "Built: $BIN ($SIZE)"

WASM_PKG_DIR="traits/kernel/wasm/pkg"
WASM_INLINE_JS="traits/www/static/wasm-inline.js"

if command -v wasm-pack >/dev/null 2>&1; then
    echo "Building WASM kernel..."
    (
        cd traits/kernel/wasm
        wasm-pack build --target web --release
    )
else
    echo "Skipping WASM build — wasm-pack not found"
fi

if [[ -f "$WASM_PKG_DIR/traits_wasm_bg.wasm" ]]; then
    echo "Generating file:// WASM inline loader..."
    {
        printf "export const WASM_BASE64 = '"
        base64 < "$WASM_PKG_DIR/traits_wasm_bg.wasm" | tr -d '\n'
        printf "';\n"
    } > "$WASM_INLINE_JS"
else
    echo "Skipping inline WASM loader generation — missing $WASM_PKG_DIR/traits_wasm_bg.wasm"
fi

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
        # Re-sign on macOS — cp invalidates the kernel's code signature cache
        [[ "$(uname)" == "Darwin" ]] && codesign -fs - "$dst" 2>/dev/null || true
        echo "  Copied $src → $dst"
    fi
}

echo "Copying dylibs..."
copy_dylib "trait_www_traits_build" "traits/www/traits/build" "build"
copy_dylib "trait_sys_checksum"     "traits/sys/checksum"     "checksum"
copy_dylib "trait_sys_ps"           "traits/sys/ps"           "ps"

echo "Traits: $("$BIN" list 2>/dev/null | wc -l | tr -d ' ') registered"
