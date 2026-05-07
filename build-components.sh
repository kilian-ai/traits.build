#!/usr/bin/env bash
# Build all WebAssembly Component Model components and transpile them
# for browser use via jco.
#
# Requirements:
#   - cargo-component  (cargo install cargo-component --locked)
#   - jco              (npm i -g @bytecodealliance/jco)
#   - rust target wasm32-wasip1 (auto-installed by cargo-component)
#
# Two source kinds are built:
#   1. Hand-written crates listed in COMPONENTS below (real logic / compute).
#   2. Auto-generated delegator crates produced by `gen-component` from any
#      `*.trait.toml` declaring `[component] auto = true`. These live in
#      target/component-gen/<dotted-path>/ and are scanned dynamically.
#
# Output goes to: traits/www/static/components/<name>/
# Each output dir contains the transpiled ESM that loads via importmap with
# `@bytecodealliance/preview2-shim` (resolved from a CDN by the demo page).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_BASE="$ROOT/traits/www/static/components"

# (component_crate_dir, output_subdir, wasm_file_name, host_trait_dir)
COMPONENTS=(
  "traits/sys/checksum-component        sys-checksum     sys_checksum_component.wasm           traits/sys/checksum"
  "traits/sys/echo-component            sys-echo         sys_echo_component.wasm               traits/sys/echo"
  "traits/sys/list-component            sys-list         sys_list_component.wasm               traits/sys/list"
  "traits/kernel/call-component         kernel-call      kernel_call_component.wasm            traits/kernel/call"
  "traits/www/local/helper-component    www-local-helper www_local_helper_component.wasm       traits/www/local/helper"
  "traits/www/local/install-component   www-local-install www_local_install_component.wasm     traits/www/local/install"
)

mkdir -p "$OUT_BASE"

# ── 0. Regenerate auto-component crates from [component] auto = true ──
echo "==> gen-component (scanning .trait.toml for [component] auto = true)"
(cd "$ROOT" && cargo run -q -p gen-component)

# ── helper: build one crate, transpile, copy host .component.wasm ──
build_one() {
  local CRATE_DIR="$1"
  local OUT_NAME="$2"
  local WASM_NAME="$3"
  local HOST_DIR="$4"

  echo "==> Building $CRATE_DIR"
  (cd "$CRATE_DIR" && cargo component build --release)

  local WASM_PATH="$CRATE_DIR/target/wasm32-wasip1/release/$WASM_NAME"
  local OUT_DIR="$OUT_BASE/$OUT_NAME"

  echo "==> Transpiling to $OUT_DIR"
  rm -rf "$OUT_DIR"
  jco transpile "$WASM_PATH" -o "$OUT_DIR" --no-typescript

  local HOST_NAME
  HOST_NAME="$(basename "$HOST_DIR").component.wasm"
  cp "$WASM_PATH" "$HOST_DIR/$HOST_NAME"
  echo "==> Host component:    $HOST_DIR/$HOST_NAME"
}

# ── 1. Hand-written components ──
for entry in "${COMPONENTS[@]}"; do
  read -r CRATE_DIR OUT_NAME WASM_NAME HOST_DIR <<<"$entry"
  build_one "$ROOT/$CRATE_DIR" "$OUT_NAME" "$WASM_NAME" "$ROOT/$HOST_DIR"
done

# ── 2. Auto-generated delegator components ──
GEN_ROOT="$ROOT/target/component-gen"
if [ -d "$GEN_ROOT" ]; then
  for crate in "$GEN_ROOT"/*/; do
    [ -d "$crate" ] || continue
    # Crate dir name is dotted-path with '.' replaced by '-' (e.g. sys-chat_protocols).
    # The host trait dir is reconstructed from the same path: split on '-' from
    # the LEFT (only first segment is the namespace), keep the rest joined as
    # the trait sub-path with '_' preserved.
    base="$(basename "$crate")"
    # Convert sys-chat_protocols → sys/chat_protocols (split on first dash only).
    ns="${base%%-*}"
    rest="${base#*-}"
    HOST_DIR="$ROOT/traits/$ns/${rest//-//}"
    if [ ! -d "$HOST_DIR" ]; then
      echo "WARN: gen-component produced $crate but host dir $HOST_DIR is missing — skipping"
      continue
    fi
    # Crate package name = "<ns>-<rest-with-dashes>-component"; the generated
    # wasm is named with underscores by cargo (package name ".replace('-', '_')").
    pkg_name="${ns}-${rest//_/-}-component"
    wasm_name="${pkg_name//-/_}.wasm"
    out_subdir="${ns}-${rest//_/-}"
    build_one "$crate" "$out_subdir" "$wasm_name" "$HOST_DIR"
  done
fi

echo "Done. Demo page: http://localhost:8092/components-demo"
