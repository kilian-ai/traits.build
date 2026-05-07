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

# (component_crate_dir, output_subdir, wasm_file_name, host_trait_dir)
# - component_crate_dir: Rust crate that builds the component
# - output_subdir:       directory under traits/www/static/components/ for jco output
# - wasm_file_name:      compiled .wasm under target/wasm32-wasip1/release/
# - host_trait_dir:      where to copy `<name>.component.wasm` so the host
#                        ComponentLoader can discover it
COMPONENTS=(
  "traits/sys/checksum-component        sys-checksum     sys_checksum_component.wasm           traits/sys/checksum"
  "traits/sys/echo-component            sys-echo         sys_echo_component.wasm               traits/sys/echo"
  "traits/www/local/helper-component    www-local-helper www_local_helper_component.wasm       traits/www/local/helper"
  "traits/www/local/install-component   www-local-install www_local_install_component.wasm     traits/www/local/install"
)

mkdir -p "$OUT_BASE"

for entry in "${COMPONENTS[@]}"; do
  read -r CRATE_DIR OUT_NAME WASM_NAME HOST_DIR <<<"$entry"
  echo "==> Building $CRATE_DIR"
  (cd "$ROOT/$CRATE_DIR" && cargo component build --release)

  WASM_PATH="$ROOT/$CRATE_DIR/target/wasm32-wasip1/release/$WASM_NAME"
  OUT_DIR="$OUT_BASE/$OUT_NAME"

  echo "==> Transpiling to $OUT_DIR"
  rm -rf "$OUT_DIR"
  jco transpile "$WASM_PATH" -o "$OUT_DIR" --no-typescript

  # Copy native component .wasm next to the trait so the host loader can find it.
  HOST_NAME="$(basename "$HOST_DIR").component.wasm"
  cp "$WASM_PATH" "$ROOT/$HOST_DIR/$HOST_NAME"
  echo "==> Host component:    $HOST_DIR/$HOST_NAME"
done

echo "Done. Demo page: http://localhost:8092/components-demo"
