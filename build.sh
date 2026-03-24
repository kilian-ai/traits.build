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
WASM_RUNTIME_JS="traits/www/static/wasm-runtime.js"

if command -v wasm-pack >/dev/null 2>&1; then
    echo "Building WASM kernel..."
    (
        cd traits/kernel/wasm
        wasm-pack build --target web --release
    )
else
    echo "Skipping WASM build — wasm-pack not found"
fi

if [[ -f "$WASM_PKG_DIR/traits_wasm_bg.wasm" && -f "$WASM_PKG_DIR/traits_wasm.js" ]]; then
    echo "Generating static WASM runtime..."
    python3 - "$WASM_PKG_DIR/traits_wasm.js" "$WASM_PKG_DIR/traits_wasm_bg.wasm" "$WASM_RUNTIME_JS" <<'PY'
import base64
import pathlib
import re
import sys

js_path = pathlib.Path(sys.argv[1])
wasm_path = pathlib.Path(sys.argv[2])
out_path = pathlib.Path(sys.argv[3])

js = js_path.read_text()
js = re.sub(r'^/\*.*?\*/\s*', '', js, count=1, flags=re.S)
js = re.sub(r'^export function (\w+)\(', r'function \1(', js, flags=re.M)
js = js.replace('export { initSync, __wbg_init as default };', '')
js = js.replace('import.meta.url', '__traits_runtime_script_url')

exports = [
    'call',
    'callable_traits',
    'cli_input',
    'cli_welcome',
    'get_trait_info',
    'init',
    'initSync',
    'is_callable',
    'is_registered',
    'list_traits',
    'search_traits',
    'version',
]

wasm_b64 = base64.b64encode(wasm_path.read_bytes()).decode('ascii')
api = ',\n'.join(f'  {name}: {name}' for name in exports)
wrapped = (
    '(function () {\n'
    + 'const __traits_runtime_script = typeof document !== "undefined" ? document.currentScript : null;\n'
    + 'const __traits_runtime_script_url = (__traits_runtime_script && __traits_runtime_script.src) ? __traits_runtime_script.src : (typeof location !== "undefined" ? location.href : "");\n'
    + js
    + '\nwindow.TraitsWasm = {\n'
    + api
    + f",\n  WASM_BASE64: '{wasm_b64}'\n"
    + '};\n})();\n'
)
out_path.write_text(wrapped)
PY
else
    echo "Skipping static WASM runtime generation — missing wasm pkg outputs"
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
