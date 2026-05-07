#!/usr/bin/env bash
# examples/hello/build.sh — build every available hello-<lang> component and
# register each as a trait via gen-trait. Missing toolchains are skipped.
set -euo pipefail

cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"
GEN_TRAIT="$ROOT/tools/gen-trait/target/release/gen-trait"

# Make optional toolchains discoverable when invoked from a non-login shell.
export PATH="$HOME/go/bin:$HOME/.wasi-sdk/bin:$HOME/Library/Python/3.13/bin:$HOME/Library/Python/3.12/bin:$HOME/Library/Python/3.11/bin:$HOME/Library/Python/3.10/bin:$HOME/Library/Python/3.9/bin:$HOME/.cargo/bin:$PATH"

if [[ ! -x "$GEN_TRAIT" ]]; then
    echo "Building gen-trait..."
    (cd "$ROOT/tools/gen-trait" && cargo build --release >/dev/null)
fi

have() { command -v "$1" >/dev/null 2>&1; }

OK=()
SKIP=()
FAIL=()

note_skip() { SKIP+=("$1: $2"); }
note_ok()   { OK+=("$1"); }
note_fail() { FAIL+=("$1: $2"); }

# ── Rust ───────────────────────────────────────────────────────────────────
build_rust() {
    if ! have cargo-component; then
        note_skip rust "install with: cargo install cargo-component --locked"
        return
    fi
    pushd rust >/dev/null
    cargo component build --release >/dev/null 2>&1 || { popd >/dev/null; note_fail rust "cargo component build failed"; return; }
    cp target/wasm32-wasip1/release/hello_rust.wasm ./hello-rust.component.wasm
    popd >/dev/null
    register rust rust/hello-rust.component.wasm
}

# ── JavaScript ─────────────────────────────────────────────────────────────
build_js() {
    if ! have node; then
        note_skip javascript "node not installed"
        return
    fi
    if ! have npx; then
        note_skip javascript "npx not installed"
        return
    fi
    pushd javascript >/dev/null
    # `jco componentize` reads ./wit and bundles hello.js into a component.
    # Pin to jco@1.4.0 + componentize-js@0.10.5 which emit wasi 0.2.0
    # (compatible with our wasmtime 26 host). Newer jco emits 0.2.10 and
    # traps at runtime.
    npx --yes -p @bytecodealliance/jco@1.4.0 \
        -p @bytecodealliance/componentize-js@0.10.5 \
        jco componentize \
        hello.js \
        --wit wit \
        --world-name hello-world \
        --out hello-javascript.component.wasm \
        --disable http \
        >/dev/null 2>&1 || { popd >/dev/null; note_fail javascript "jco componentize failed"; return; }
    popd >/dev/null
    register javascript javascript/hello-javascript.component.wasm
}

# ── Python ─────────────────────────────────────────────────────────────────
build_python() {
    if ! have componentize-py; then
        note_skip python "install with: pip3 install --user componentize-py"
        return
    fi
    pushd python >/dev/null
    componentize-py -d wit -w hello-world componentize app -o hello-python.component.wasm >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail python "componentize-py failed"; return; }
    popd >/dev/null
    register python python/hello-python.component.wasm
}

# ── Go (TinyGo) ────────────────────────────────────────────────────────────
build_go() {
    if ! have tinygo; then
        note_skip go "install with: brew install tinygo"
        return
    fi
    if ! have wit-bindgen-go; then
        note_skip go "install with: go install go.bytecodealliance.org/cmd/wit-bindgen-go@latest"
        return
    fi
    pushd go >/dev/null
    rm -rf internal
    wit-bindgen-go generate --world hello-world --out internal/ ./wit >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail go "wit-bindgen-go generate failed"; return; }
    go mod tidy >/dev/null 2>&1 || true
    tinygo build -target=wasip2 --wit-package ./wit --wit-world hello-world -o hello-go.module.wasm . >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail go "tinygo build failed"; return; }
    # TinyGo with -target=wasip2 emits a component directly.
    mv hello-go.module.wasm hello-go.component.wasm
    popd >/dev/null
    register go go/hello-go.component.wasm
}

# ── C ──────────────────────────────────────────────────────────────────────
build_c() {
    if ! have wit-bindgen; then
        note_skip c "install with: cargo install wit-bindgen-cli"
        return
    fi
    local wasi_clang=""
    for p in /opt/wasi-sdk/bin/clang /usr/local/wasi-sdk/bin/clang "$HOME/.wasi-sdk/bin/clang"; do
        if [[ -x "$p" ]]; then wasi_clang="$p"; break; fi
    done
    if [[ -z "$wasi_clang" ]]; then
        note_skip c "install wasi-sdk: brew install wasi-sdk (or download from github.com/WebAssembly/wasi-sdk)"
        return
    fi
    if ! have wasm-tools; then
        note_skip c "install with: cargo install wasm-tools"
        return
    fi
    pushd c >/dev/null
    wit-bindgen c --world hello-world --out-dir . ./wit >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail c "wit-bindgen c failed"; return; }
    "$wasi_clang" -mexec-model=reactor -O2 -I. \
        hello.c hello_world.c hello_world_component_type.o \
        -o hello-c.module.wasm 2>/dev/null \
        || { popd >/dev/null; note_fail c "clang compile failed"; return; }
    wasm-tools component new hello-c.module.wasm -o hello-c.component.wasm >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail c "wasm-tools component new failed"; return; }
    popd >/dev/null
    register c c/hello-c.component.wasm
}

# ── C++ ────────────────────────────────────────────────────────────────────
build_cpp() {
    if ! have wit-bindgen; then
        note_skip cpp "install with: cargo install wit-bindgen-cli"
        return
    fi
    local wasi_clangpp=""
    for p in /opt/wasi-sdk/bin/clang++ /usr/local/wasi-sdk/bin/clang++ "$HOME/.wasi-sdk/bin/clang++"; do
        if [[ -x "$p" ]]; then wasi_clangpp="$p"; break; fi
    done
    if [[ -z "$wasi_clangpp" ]]; then
        note_skip cpp "install wasi-sdk: brew install wasi-sdk"
        return
    fi
    if ! have wasm-tools; then
        note_skip cpp "install with: cargo install wasm-tools"
        return
    fi
    pushd cpp >/dev/null
    wit-bindgen c --world hello-world --out-dir . ./wit >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail cpp "wit-bindgen c failed"; return; }
    local wasi_clang="${wasi_clangpp%++}"
    "$wasi_clang" -O2 -c -I. hello_world.c -o hello_world.cobj 2>/dev/null \
        || { popd >/dev/null; note_fail cpp "clang -c hello_world.c failed"; return; }
    "$wasi_clangpp" -mexec-model=reactor -O2 -std=c++20 -fno-exceptions -fno-rtti -I. \
        hello.cpp hello_world.cobj hello_world_component_type.o \
        -o hello-cpp.module.wasm 2>/dev/null \
        || { popd >/dev/null; note_fail cpp "clang++ compile failed"; return; }
    wasm-tools component new hello-cpp.module.wasm -o hello-cpp.component.wasm >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail cpp "wasm-tools component new failed"; return; }
    popd >/dev/null
    register cpp cpp/hello-cpp.component.wasm
}

# ── Zig ────────────────────────────────────────────────────────────────────
build_zig() {
    if ! have zig; then
        note_skip zig "install with: brew install zig"
        return
    fi
    if ! have wit-bindgen; then
        note_skip zig "install with: cargo install wit-bindgen-cli"
        return
    fi
    if ! have wasm-tools; then
        note_skip zig "install with: cargo install wasm-tools"
        return
    fi
    pushd zig >/dev/null
    wit-bindgen c --world hello-world --out-dir . ./wit >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail zig "wit-bindgen c failed"; return; }
    rm -f hello.wasm hello-zig.*.wasm
    zig build-exe hello.zig hello_world.c hello_world_component_type.o \
        -target wasm32-wasi -rdynamic -fno-entry -lc \
        --export=exports_hello_zig_hello_hello \
        -OReleaseSmall 2>/dev/null \
        || { popd >/dev/null; note_fail zig "zig build-exe failed"; return; }
    mv hello.wasm hello-zig.module.wasm
    wasm-tools component embed --world hello-world ./wit hello-zig.module.wasm -o hello-zig.embed.wasm >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail zig "wasm-tools component embed failed"; return; }
    # Zig's wasm32-wasi target produces preview1 imports; use the reactor
    # adapter to convert them into wasi:cli/p2 component imports.
    local adapter="$HOME/.wasi-sdk/wasi_snapshot_preview1.reactor.wasm"
    if [[ ! -f "$adapter" ]]; then
        popd >/dev/null
        note_fail zig "missing adapter: download wasi_snapshot_preview1.reactor.wasm into ~/.wasi-sdk/"
        return
    fi
    wasm-tools component new hello-zig.embed.wasm --adapt "wasi_snapshot_preview1=$adapter" -o hello-zig.component.wasm >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail zig "wasm-tools component new failed"; return; }
    popd >/dev/null
    register zig zig/hello-zig.component.wasm
}

# ── Scheme (TinyScheme via wasi-sdk) ───────────────────────────────────────
build_scheme() {
    if ! have wit-bindgen; then
        note_skip scheme "install with: cargo install wit-bindgen-cli"
        return
    fi
    local wasi_clang=""
    for p in /opt/wasi-sdk/bin/clang /usr/local/wasi-sdk/bin/clang "$HOME/.wasi-sdk/bin/clang"; do
        if [[ -x "$p" ]]; then wasi_clang="$p"; break; fi
    done
    if [[ -z "$wasi_clang" ]]; then
        note_skip scheme "install wasi-sdk: brew install wasi-sdk"
        return
    fi
    if ! have wasm-tools; then
        note_skip scheme "install with: cargo install wasm-tools"
        return
    fi
    if ! have xxd; then
        note_skip scheme "xxd not installed"
        return
    fi
    pushd scheme >/dev/null
    wit-bindgen c --world hello-world --out-dir . ./wit >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail scheme "wit-bindgen c failed"; return; }
    # Embed init.scm and hello.scm as NUL-terminated C arrays.
    (cat init.scm; printf '\0') | xxd -i -n init_scm > init_scm.h
    (cat hello.scm; printf '\0') | xxd -i -n hello_scm > hello_scm.h
    "$wasi_clang" -mexec-model=reactor -O2 -I. \
        -DSTANDALONE=0 -DUSE_DL=0 -DUSE_INTERFACE=1 -DUSE_MATH=0 \
        -Wno-implicit-function-declaration -Wno-incompatible-pointer-types \
        bridge.c scheme.c hello_world.c hello_world_component_type.o \
        -o hello-scheme.module.wasm 2>/tmp/scheme-build.log \
        || { popd >/dev/null; note_fail scheme "clang compile failed (see /tmp/scheme-build.log)"; return; }
    local adapter="$HOME/.wasi-sdk/wasi_snapshot_preview1.reactor.wasm"
    if [[ ! -f "$adapter" ]]; then
        popd >/dev/null
        note_fail scheme "missing adapter: download wasi_snapshot_preview1.reactor.wasm into ~/.wasi-sdk/"
        return
    fi
    wasm-tools component new hello-scheme.module.wasm --adapt "wasi_snapshot_preview1=$adapter" -o hello-scheme.component.wasm >/dev/null 2>&1 \
        || { popd >/dev/null; note_fail scheme "wasm-tools component new failed"; return; }
    popd >/dev/null
    register scheme scheme/hello-scheme.component.wasm
}

register() {
    local lang="$1"
    local wasm="$2"
    local trait_dir="$ROOT/traits/hello/$lang"
    rm -f "$trait_dir/$lang.trait.toml"
    "$GEN_TRAIT" "$wasm" --as "hello.$lang" --traits-root "$ROOT/traits" >/dev/null 2>&1 \
        || { note_fail "$lang" "gen-trait registration failed"; return; }
    note_ok "$lang"
}

echo "==> Building hello components"
build_rust
build_js
build_python
build_go
build_c
build_cpp
build_zig
build_scheme

echo ""
echo "── Results ────────────────────────────────────────"
if (( ${#OK[@]} )); then
    echo "✓ Built & registered:"
    for x in "${OK[@]}"; do echo "    hello.$x"; done
fi
if (( ${#SKIP[@]} )); then
    echo "○ Skipped (toolchain missing):"
    for x in "${SKIP[@]}"; do echo "    $x"; done
fi
if (( ${#FAIL[@]} )); then
    echo "✗ Failed:"
    for x in "${FAIL[@]}"; do echo "    $x"; done
fi

echo ""
echo "Try them:"
for x in "${OK[@]}"; do
    echo "    traits call hello.$x world"
done
