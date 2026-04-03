#!/bin/bash
# Build the Traits kernel
# Usage: ./build.sh [--clean]
set -euo pipefail
cd "$(dirname "$0")"

if [[ "${1:-}" == "--clean" ]]; then
    echo "Cleaning build..."
    cargo clean
fi

WASM_PKG_DIR="traits/kernel/wasm/pkg"
WASM_RUNTIME_JS="traits/www/static/wasm-runtime.js"
WASM_WORKER_JS="traits/www/static/traits-worker.js"
SDK_SRC="traits/www/sdk/traits.js"
SDK_RUNTIME="traits/www/static/sdk-runtime.js"
INDEX_HTML="traits/www/static/index.html"
INDEX_STANDALONE_HTML="traits/www/static/index.standalone.html"

# ── Pre-compute build version so WASM and native builds match ──
TODAY=$(date -u '+%y%m%d')
HHMMSS=$(date -u '+%H%M%S')
VERSION_TOML="traits/sys/version/version.trait.toml"
CURRENT_VER=$(grep '^version' "$VERSION_TOML" | head -1 | sed 's/.*"\(.*\)".*/\1/' | sed 's/^v//')
if [[ "$CURRENT_VER" == "${TODAY}"* ]]; then
    export TRAITS_BUILD_VERSION="v${TODAY}.${HHMMSS}"
else
    export TRAITS_BUILD_VERSION="v${TODAY}"
fi
# Update version.trait.toml so both build.rs files can read it
sed -i '' "s/^version = .*/version = \"${TRAITS_BUILD_VERSION}\"/" "$VERSION_TOML"
echo "Build version: $TRAITS_BUILD_VERSION"

# Build WASM first so the native binary embeds the latest WASM pkg via include_bytes!
if command -v wasm-pack >/dev/null 2>&1; then
    echo "Building WASM kernel..."
    (
        cd traits/kernel/wasm
        wasm-pack build --target web --release
    )
else
    echo "Skipping WASM build — wasm-pack not found"
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
    'cli_format_rest_result',
    'cli_get_history',
    'cli_input',
    'cli_set_history',
    'cli_welcome',
    'get_trait_info',
    'init',
    'initSync',
    'is_callable',
    'is_registered',
    'list_traits',
    'run_tests',
    'search_traits',
    'set_helper_connected',
    'register_task',
    'unregister_task',
    'set_secret',
    'version',
    'vfs_dump',
    'vfs_load',
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

    echo "Generating WASM worker runtime..."
    python3 - "$WASM_PKG_DIR/traits_wasm.js" "$WASM_PKG_DIR/traits_wasm_bg.wasm" "$WASM_WORKER_JS" <<'PY'
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
    'cli_format_rest_result',
    'cli_get_history',
    'cli_input',
    'cli_set_history',
    'cli_welcome',
    'get_trait_info',
    'init',
    'initSync',
    'is_callable',
    'is_registered',
    'list_traits',
    'pvfs_dump',
    'pvfs_load',
    'register_task',
    'run_tests',
    'search_traits',
    'set_helper_connected',
    'set_helper_url',
    'set_secret',
    'unregister_task',
    'version',
    'vfs_dump',
    'vfs_load',
]

wasm_b64 = base64.b64encode(wasm_path.read_bytes()).decode('ascii')
api = ',\n'.join(f'  {name}: {name}' for name in exports)
worker = (
    'const __traits_runtime_script_url = (typeof location !== "undefined" ? location.href : "");\n'
    + js
    + '\nconst TraitsWasm = {\n'
    + api
    + f",\n  WASM_BASE64: '{wasm_b64}'\n"
    + '};\n'
    + 'function decodeBase64Bytes(b64) {\n'
    + '  const raw = atob(b64);\n'
    + '  const out = new Uint8Array(raw.length);\n'
    + '  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);\n'
    + '  return out;\n'
    + '}\n'
    + 'let __traits_ready = false;\n'
    + 'function ensureReady() {\n'
    + '  if (__traits_ready) return;\n'
    + '  TraitsWasm.initSync({ module: decodeBase64Bytes(TraitsWasm.WASM_BASE64) });\n'
    + '  JSON.parse(TraitsWasm.init());\n'
    + '  __traits_ready = true;\n'
    + '}\n'
    + 'function sendOk(id, result) { self.postMessage({ id, ok: true, result }); }\n'
    + 'function sendErr(id, err) { self.postMessage({ id, ok: false, error: (err && err.message) ? err.message : String(err) }); }\n'
    + 'let __canvasLen = -1;\n'
    + 'function checkCanvasSync() {\n'
    + '  try {\n'
    + '    const g = JSON.parse(TraitsWasm.call("sys.canvas", \'["get"]\'));\n'
    + '    const c = g.content || "";\n'
    + '    if (c.length !== __canvasLen) { __canvasLen = c.length; self.postMessage({ _type: "canvas-sync", content: c }); }\n'
    + '  } catch(e) {}\n'
    + '}\n'
    + 'function syncPvfsToMain() {\n'
    + '  try {\n'
    + '    const json = TraitsWasm.pvfs_dump();\n'
    + '    console.log("[worker] pvfs-sync dump len=" + json.length);\n'
    + '    self.postMessage({ _type: "pvfs-sync", json });\n'
    + '  } catch(e) { console.warn("[worker] pvfs-sync error:", e); }\n'
    + '}\n'
    + 'self.onmessage = function(ev) {\n'
    + '  const msg = ev.data || {};\n'
    + '  const id = msg.id;\n'
    + '  const cmd = msg.cmd || "";\n'
    + '  const payload = msg.payload || {};\n'
    + '  try {\n'
    + '    if (cmd !== "ping") ensureReady();\n'
    + '    switch (cmd) {\n'
    + '      case "ping": sendOk(id, "pong"); break;\n'
    + '      case "init": sendOk(id, true); break;\n'
    + '      case "set_helper_connected": TraitsWasm.set_helper_connected(!!payload.connected); sendOk(id, true); break;\n'
    + '      case "set_helper_url": TraitsWasm.set_helper_url(String(payload.url || "")); sendOk(id, true); break;\n'
    + '      case "set_secret": TraitsWasm.set_secret(String(payload.key || ""), String(payload.value || "")); sendOk(id, true); break;\n'
    + '      case "pvfs_load": {\n'
    + '        console.log("[worker] pvfs_load len=" + (payload.json || "").length);\n'
    + '        TraitsWasm.pvfs_load(payload.json || "{}");\n'
    + '        sendOk(id, true);\n'
    + '        break;\n'
    + '      }\n'
    + '      case "cli_input": {\n'
    + '        const out = TraitsWasm.cli_input(payload.data || "");\n'
    + '        sendOk(id, out);\n'
    + '        checkCanvasSync();\n'
    + '        syncPvfsToMain();\n'
    + '        break;\n'
    + '      }\n'
    + '      case "cli_welcome": sendOk(id, TraitsWasm.cli_welcome()); break;\n'
    + '      case "cli_get_history": sendOk(id, TraitsWasm.cli_get_history()); break;\n'
    + '      case "cli_set_history": TraitsWasm.cli_set_history(payload.history_json || "[]"); sendOk(id, true); break;\n'
    + '      case "sync_tasks": {\n'
    + '        const incoming = Array.isArray(payload.tasks) ? payload.tasks : [];\n'
    + '        const existing = new Set((Array.isArray(payload.existing_ids) ? payload.existing_ids : []).map(String));\n'
    + '        const incomingIds = new Set();\n'
    + '        for (const task of incoming) {\n'
    + '          if (!task || task.id == null) continue;\n'
    + '          const idStr = String(task.id);\n'
    + '          incomingIds.add(idStr);\n'
    + '          TraitsWasm.register_task(idStr, String(task.name || idStr), String(task.task_type || "task"), Number(task.started_ms || Date.now()), String(task.detail || ""));\n'
    + '        }\n'
    + '        for (const idStr of existing) {\n'
    + '          if (!incomingIds.has(idStr)) TraitsWasm.unregister_task(idStr);\n'
    + '        }\n'
    + '        sendOk(id, true);\n'
    + '        break;\n'
    + '      }\n'
    + '      case "vfs_dump": sendOk(id, TraitsWasm.vfs_dump()); break;\n'
    + '      case "vfs_load": TraitsWasm.vfs_load(payload.json || "{}"); sendOk(id, true); break;\n'
    + '      case "cli_format_rest_result": sendOk(id, TraitsWasm.cli_format_rest_result(payload.path || "", payload.args_json || "[]", payload.result_json || "null")); break;\n'
    + '      case "call": {\n'
    + '        const p = payload.path || "";\n'
    + '        const raw = TraitsWasm.call(p, JSON.stringify(payload.args || []));\n'
    + '        const res = JSON.parse(raw);\n'
    + '        sendOk(id, res);\n'
    + '        if (p === "sys.canvas" || p === "sys.vfs") { checkCanvasSync(); syncPvfsToMain(); }\n'
    + '        break;\n'
    + '      }\n'
    + '      case "call_raw": sendOk(id, TraitsWasm.call(payload.path || "", payload.args_json || "[]")); break;\n'
    + '      case "callable_traits": sendOk(id, JSON.parse(TraitsWasm.callable_traits())); break;\n'
    + '      default: throw new Error("Unknown command: " + cmd);\n'
    + '    }\n'
    + '  } catch (e) {\n'
    + '    sendErr(id, e);\n'
    + '  }\n'
    + '};\n'
)

out_path.write_text(worker)
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

# ── Generate terminal-runtime.js (classic script for file:// mode) ──
TERMINAL_SRC="traits/www/terminal/terminal.js"
TERMINAL_CSS="traits/www/terminal/terminal.css"
TERMINAL_RUNTIME="traits/www/static/terminal-runtime.js"
if [[ -f "$TERMINAL_SRC" ]]; then
    echo "Generating terminal runtime..."
    {
        # Inject CSS + xterm CDN stylesheet on first call
        cat <<'CSSJS'
(function() {
  if (document.getElementById('_term-css')) return;
  var link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = 'https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.min.css';
  link.id = '_xterm-css';
  document.head.appendChild(link);
  var style = document.createElement('style');
  style.id = '_term-css';
CSSJS
        # Inline the terminal CSS content
        echo -n "  style.textContent = "
        python3 -c "
import sys, json
css = open(sys.argv[1]).read()
print(json.dumps(css) + ';')
" "$TERMINAL_CSS"
        cat <<'CSSJS2'
  document.head.appendChild(style);
})();
CSSJS2
        # Terminal JS with export stripped
        sed 's/^export async function/async function/' "$TERMINAL_SRC"
        echo 'if (typeof window !== "undefined") window.createTerminal = createTerminal;'
    } > "$TERMINAL_RUNTIME"
fi

# ── Generate sdk-runtime.js (classic script from ES module traits.js) ──
if [[ -f "$SDK_SRC" ]]; then
    echo "Generating SDK runtime..."
    {
        echo '(function() {'
        sed -E \
            -e 's/^export class /class /' \
            -e 's/^export function /function /' \
            -e '/^export default/d' \
            -e '/^export \{/d' \
            "$SDK_SRC"
        echo 'if (typeof window !== "undefined") { window.Traits = Traits; window.getTraits = getTraits; }'
        echo '})();'
    } > "$SDK_RUNTIME"
fi

if [[ -f "$INDEX_HTML" && -f "$WASM_RUNTIME_JS" && -f "$WASM_WORKER_JS" && -f "$TERMINAL_RUNTIME" && -f "$SDK_RUNTIME" ]]; then
    echo "Generating standalone HTML..."
    python3 - "$INDEX_HTML" "$WASM_RUNTIME_JS" "$WASM_WORKER_JS" "$TERMINAL_RUNTIME" "$SDK_RUNTIME" "$INDEX_STANDALONE_HTML" <<'PY'
import pathlib
import sys

index_path = pathlib.Path(sys.argv[1])
wasm_runtime_path = pathlib.Path(sys.argv[2])
worker_runtime_path = pathlib.Path(sys.argv[3])
terminal_runtime_path = pathlib.Path(sys.argv[4])
sdk_runtime_path = pathlib.Path(sys.argv[5])
out_path = pathlib.Path(sys.argv[6])

html = index_path.read_text()
wasm_runtime = wasm_runtime_path.read_text()
worker_runtime = worker_runtime_path.read_text()
terminal_runtime = terminal_runtime_path.read_text()
sdk_runtime = sdk_runtime_path.read_text()

runtime_fn = """function runtimeScriptPath() {
  return 'inline:wasm-runtime';
}"""

runtime_fn_old = """function runtimeScriptPath() {
  if (isLocal) return `./wasm-runtime.js?v=${Date.now()}`;
  return '/static/www/static/wasm-runtime.js';
}"""

term_src_old = "const termSrc = isLocal ? `./terminal-runtime.js?v=${Date.now()}` : '/static/www/static/terminal-runtime.js';"
term_src_new = "const termSrc = 'inline:terminal-runtime';"

sdk_src_old = "const sdkSrc = isLocal ? `./sdk-runtime.js?v=${Date.now()}` : '/static/www/static/sdk-runtime.js';"
sdk_src_new = "const sdkSrc = 'inline:sdk-runtime';"

if runtime_fn_old not in html:
    raise SystemExit('standalone generation failed: runtimeScriptPath() block not found')
if term_src_old not in html:
    raise SystemExit('standalone generation failed: terminal runtime path not found')
if sdk_src_old not in html:
    raise SystemExit('standalone generation failed: SDK runtime path not found')

html = html.replace(runtime_fn_old, runtime_fn)
html = html.replace(term_src_old, term_src_new)
html = html.replace(sdk_src_old, sdk_src_new)

# Standalone is always "local" (hash routing) — no server to handle pushState paths
html = html.replace(
    "const isLocal = location.protocol === 'file:';",
    "const isLocal = true; // standalone: always use hash routing"
)

def escape_script(code: str) -> str:
    return code.replace('</script>', '<\\/script>')

inline_scripts = (
    '<script data-runtime-src="inline:wasm-runtime">\n'
    + escape_script(wasm_runtime)
    + '\n</script>\n'
    + '<script type="text/plain" data-runtime-src="inline:traits-worker">\n'
    + escape_script(worker_runtime)
    + '\n</script>\n'
    + '<script data-runtime-src="inline:terminal-runtime">\n'
    + escape_script(terminal_runtime)
    + '\n</script>\n'
    + '<script data-runtime-src="inline:sdk-runtime">\n'
    + escape_script(sdk_runtime)
    + '\n</script>\n'
)

marker = '<script>\n// ═══════════════════════════════════════════════════════════════'
if marker not in html:
    raise SystemExit('standalone generation failed: boot script marker not found')

html = html.replace(marker, inline_scripts + marker, 1)
out_path.write_text(html)
PY
fi

if [[ -f "$INDEX_STANDALONE_HTML" ]]; then
    cp "$INDEX_STANDALONE_HTML" index.html
    echo "Copied $INDEX_STANDALONE_HTML → index.html"
fi

echo "Copying dylibs..."
copy_dylib "trait_www_traits_build" "traits/www/traits/build" "build"
copy_dylib "trait_sys_checksum"     "traits/sys/checksum"     "checksum"
copy_dylib "trait_sys_ps"           "traits/sys/ps"           "ps"

echo "Syncing local/ scripts from trait sources..."
cp traits/www/local/helper/helper.sh local/helper.sh
cp traits/www/local/helper/helper.sh local/traits.sh
cp traits/www/local/install/install.sh local/install.sh

echo "Traits: $("$BIN" list 2>/dev/null | grep -c '"path"') registered"

# ── Create git release tag ──
VERSION="$("$BIN" version </dev/null 2>/dev/null | grep -oE 'v[0-9]{6,}\.[0-9]+' | head -1 || true)"
if [[ -n "$VERSION" ]] && command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    if ! git tag --list "$VERSION" | grep -q .; then
        git tag "$VERSION"
        echo "Tagged: $VERSION"
        # Push tag if remote exists
        if git remote get-url origin >/dev/null 2>&1; then
            git push origin "$VERSION" 2>/dev/null && echo "Pushed tag $VERSION" || echo "  (tag push failed — push manually with: git push origin $VERSION)"
        fi
    else
        echo "Tag $VERSION already exists"
    fi
fi
