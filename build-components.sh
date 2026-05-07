#!/usr/bin/env bash
# Build every WebAssembly Component Model component declared by the trait
# tree and transpile each for browser use via jco.
#
# Source of truth: every `*.trait.toml` that contains
#
#     [component]
#     auto = true
#
# Such traits are materialised into `target/component-gen/<dotted-path>/`
# by `gen-component`, then built with `cargo component build --release` and
# the resulting `<name>.component.wasm` is copied next to the source trait
# so the host `ComponentLoader` discovers it at startup.
#
# Requirements:
#   - cargo-component  (cargo install cargo-component --locked)
#   - jco              (npm i -g @bytecodealliance/jco)
#   - rust target wasm32-wasip1 (auto-installed by cargo-component)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_BASE="$ROOT/traits/www/static/components"
mkdir -p "$OUT_BASE"

# ── Regenerate auto-component crates from [component] auto = true ──
echo "==> gen-component (scanning .trait.toml for [component] auto = true)"
(cd "$ROOT" && cargo run -q -p gen-component)

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

# ── Build every generated delegator crate ──
GEN_ROOT="$ROOT/target/component-gen"
if [ -d "$GEN_ROOT" ]; then
  for crate in "$GEN_ROOT"/*/; do
    [ -d "$crate" ] || continue
    base="$(basename "$crate")"
    # Crate dir name = dotted-path with '.' → '-' (e.g. sys-chat_protocols,
    # www-local-helper). Reconstruct the host trait dir by splitting only the
    # FIRST dash off as the namespace; the rest is the on-disk sub-path with
    # '_' preserved and remaining '-' acting as path separators.
    ns="${base%%-*}"
    rest="${base#*-}"
    HOST_DIR="$ROOT/traits/$ns/${rest//-//}"
    if [ ! -d "$HOST_DIR" ]; then
      echo "WARN: gen-component produced $crate but host dir $HOST_DIR is missing — skipping"
      continue
    fi
    pkg_name="${ns}-${rest//_/-}-component"
    wasm_name="${pkg_name//-/_}.wasm"
    out_subdir="${ns}-${rest//_/-}"
    build_one "$crate" "$out_subdir" "$wasm_name" "$HOST_DIR"
  done
fi

echo "Done. Demo page: http://localhost:8092/components-demo"
