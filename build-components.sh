#!/usr/bin/env bash
# Build all WebAssembly Component Model components and transpile them
# for browser use via jco.
#
# Requirements:
#   - cargo-component  (cargo install cargo-component --locked)
#   - jco              (npm i -g @bytecodealliance/jco)
#   - rust target wasm32-wasip1 (auto-installed by cargo-component)
#
# Output goes to: traits/www/static/components/<name>/
# Each output dir contains the transpiled ESM that loads via importmap with
# `@bytecodealliance/preview2-shim` (resolved from a CDN by the demo page).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_BASE="$ROOT/traits/www/static/components"

# (component_crate_dir, output_subdir, wasm_file_name)
COMPONENTS=(
  "traits/sys/checksum-component sys-checksum sys_checksum_component.wasm"
)

mkdir -p "$OUT_BASE"

for entry in "${COMPONENTS[@]}"; do
  read -r CRATE_DIR OUT_NAME WASM_NAME <<<"$entry"
  echo "==> Building $CRATE_DIR"
  (cd "$ROOT/$CRATE_DIR" && cargo component build --release)

  WASM_PATH="$ROOT/$CRATE_DIR/target/wasm32-wasip1/release/$WASM_NAME"
  OUT_DIR="$OUT_BASE/$OUT_NAME"

  echo "==> Transpiling to $OUT_DIR"
  rm -rf "$OUT_DIR"
  jco transpile "$WASM_PATH" -o "$OUT_DIR" --no-typescript
done

echo "Done. Demo page: http://localhost:8092/components-demo"
