# Dual-Kernel Architecture Exploration

## A) WASM Kernel

**Entry Point**: `traits/kernel/wasm/src/lib.rs` (~295 lines)

**Compilation**: `wasm-pack` targets `wasm32-unknown-unknown`, produces `traits_wasm.js` + `traits_wasm_bg.wasm`

**Cargo.toml**: Crate `traits-wasm`, ctype `["cdylib", "rlib"]`
- Dependencies: kernel-logic, wasm-bindgen, serde_json, web-sys, sha2, maud, pulldown-cmark

**What it compiles**:
- WASM-callable traits: 26 traits (see WASM_CALLABLE list below)
- Trait registry built at compile-time from `include!(builtin_traits.rs)`
- CLI session state machine (thin wrapper around kernel/cli CliSession)
- WASM fallback implementations for native traits
- All www.* page traits (HTML generators)

**Key exports** (#[wasm_bindgen]):
- `init()` → returns {status, traits_registered, wasm_callable, version}
- `call(path, args_json)` → sync dispatch for WASM-callable traits
- `is_callable(path)` → check if trait runs in WASM
- `is_registered(path)` → check if trait exists (any type)
- `set_secret(key, value)` → store auth secrets for sys.call
- `set_helper_connected(bool)` → notify kernel of local helper presence
- `list_traits()` → full registry as JSON
- `get_trait_info(path)` → detailed trait metadata
- `search_traits(query)` → search by path/description
- `callable_traits()` → returns WASM_CALLABLE list
- `cli_input(data)` → stateful CLI session dispatch
- `run_tests(pattern, verbose)` → example-based test runner

**WASM_CALLABLE list** (26 traits):
```
kernel.call, kernel.types, sys.call, sys.checksum, sys.cli.wasm,
sys.info, sys.list, sys.llm, sys.ps, sys.ps.wasm, sys.openapi,
sys.registry, sys.test_runner, sys.version,
llm.prompt.webllm,
www.admin, www.admin.spa, www.chat_logs, www.docs, www.docs.api,
www.llm_test, www.playground, www.static, www.traits.build, www.wasm
```

**HELPER_PREFERRED traits** (1):
- `sys.ps` — has WASM fallback but prefers native binary for rich OS data

**Module structure** (`wasm_traits/mod.rs`):
- Includes sys/* trait modules by file path (checksum, registry, version, cli, ps)
- Includes www/* page generators (build, docs, admin, chat_logs, playground, wasm)
- Includes kernel modules (types, call)
- All compiled for wasm32 target
- Entries use `#[path = "../../../../..."]` to share source files with native build

---

## B) Native Kernel

**Entry Point**: `src/main.rs` (~165 lines)

**Compilation**: `cargo build --release` produces `/target/release/traits` binary

**Root Cargo.toml**:
- Crate `traits`, members: kernel/logic, kernel/plugin_api, sys/checksum, sys/ps
- Dependencies: actix-web, tokio, kernel-logic, libloading, clap, serde_json, etc.
- Binary: `traits` from `src/main.rs`

**What it compiles**:
- Full trait system with dylib plugin loader
- HTTP REST server (actix-web)
- CLI dispatcher
- Registry + plugin_api system
- All sys/* traits (with native OS access)
- All www/* traits

**Main entry**: `#[tokio::main] fn main()` → calls `dispatcher::compiled::cli::run()`

**Key function**: `bootstrap(config)` → starts:
1. Registry (load trait definitions from traits_dir)
2. Globals init (make registry accessible to thread locals)
3. Dylib loader (load .dylib/.so/.dll trait implementations recursively)
4. Dispatcher (async trait call router)

**trait_exists()**: Probe only (no bootstrap) — checks for `{namespace}/{trait}/{trait}.trait.toml`

**main_info()**: Returns binary metadata + interface resolution status

---

## C) Shared Code Layer (kernel/logic)

**Location**: `traits/kernel/logic/src/`

**Module count**: 2 public modules only
- `types.rs` — cross-language type system
- `registry.rs` — trait TOML parsing + registry structures

### **types.rs** (~80 lines):
- `TraitType: {Int, Float, String, Bool, Bytes, Null, List, Map, Optional, Any, Handle}`
- `TraitValue: {Null, Bool, Int, Float, String, List, Map, Bytes}` — runtime cross-language values
- Methods: `to_json()`, `from_json()`
- Used by both WASM and native for serialization

### **registry.rs** (~100-150 lines):
- `BuiltinTraitDef` — compile-time embedded trait metadata
- `TraitToml` — TOML deserialization for .trait.toml files
- TraitDefToml fields: description, version, author, tags, imports, gui, frontend, stream, background, command, codegen, sources, etc.
- `HttpTraitConfig` — REST-to-HTTP routing configuration (method, url, headers, auth_secret, response_path)
- `SignatureToml`, `ImplementationToml`, `CliMapToml` — other TOML sections

**Abstraction level**: Parsing + type definitions only. No actual dispatch logic.

---

## D) JS SDK Layer (traits.js)

**Location**: `traits/www/sdk/traits.js` (~600+ lines, full implementation)

**Exports**:
- `class Traits` — main client
- `loadWasm(wasmUrl, jsUrl)` — lazy-load WASM
- `discoverHelper()` — probe localhost:8090/8091/9090
- `callHelper(path, args)` — POST to local helper /traits/{path}
- Various helper functions

**Traits class methods**:

```
constructor(opts) — set server, useWasm, useHelper, wasmUrl, jsUrl
async init() — idempotent, load WASM + discover helper in parallel
async call(path, args, opts) — **dispatch cascade** (below)
isCallable(path) — check if WASM-callable
dispatchMode(path) → 'wasm'|'helper'|'rest'|'none'
async connectHelper(url) — manual helper override
_callWasm(path, args) → sync dispatch to wasm.call()
async _callWebLLM(prompt, model) → lazy-load @mlc-ai/web-llm
async _callRest(path, args, opts) → POST /traits/{path}
async _callHelper(path, args) → POST http://localhost:{port}/traits/{path}
```

**Dispatch cascade** (in call()):
1. WASM (instant, local) if wasmCallableSet.has(path)
   - Intercepts WebLLM sentinel to route to JS engine
2. Helper (localhost) if helperReady
   - POST to http://localhost:{8090|8091|9090}/traits/{path}
3. REST (server) if server URL exists
   - POST /traits/{path}
4. Error if nothing available

**Helper detection**:
- Tries localStorage.traits.helper.url first
- Auto-discovers on ports 8090, 8091, 9090 (HELPER_PORTS)
- Stores successful URL in localStorage
- Timeout: 1500ms per probe

**WebLLM support**:
- Lazy-loads @mlc-ai/web-llm (esm.run CDN)
- Model: SmolLM2-360M-Instruct-q4f16_1-MLC (default)
- Requires WebGPU support

---

## E) SPA Inlined SDK (index.html)

**Location**: `traits/www/static/index.html` (~800+ lines)

**Duplication**: YES — near-verbatim copy of traits.js functions
- `probeHelper()` — identical logic
- `discoverHelper()` — identical logic
- `callHelper()` — identical logic
- `_ensureWebLLM()` — identical logic

**Key difference from traits.js**: Maintains separate internal state
- Local `wasm`, `wasmReady`, `wasmCallableSet`
- Local `helperUrl`, `helperReady`, `helperInfo`
- Local `_webllmLib`, `_webllmEngine`, `_webllmModel`

**SPA-specific additions**:

```javascript
class TraitsSDK {
  async call(path, args) — dispatch cascade (WASM→helper→REST)
  async connectHelper(url)
  async init(opts)
  async list()
  async info(path)
  async search(query)
}

const ROUTES = {
  '/': 'www.traits.build',
  '/docs': 'www.docs',
  '/docs/api': 'www.docs.api',
  '/chat-logs': 'www.chat_logs',
  '/admin': 'www.admin.spa',
  '/settings': 'www.admin.spa',
  '/playground': 'www.docs.api',
  '/terminal': 'www.terminal',
  '/wasm': 'www.wasm',
}

async boot() — load WASM, discover helper, parse URL route
async route(path) — call appropriate www.* trait
renderPage(html) — inject into #page-frame
```

**URL routing**: Maps /path → www.namespace.trait caller

**Boot flow**:
1. Load WASM (with spinner overlay)
2. Discover local helper (parallel)
3. Parse URL, determine route from ROUTES table
4. Call appropriate www.* trait via TraitsSDK
5. Render HTML to #page-frame
6. Show nav bar

**Window global**: `window._traitsSDK` for pages to call traits

---

## F) Terminal Layer (terminal.js)

**Location**: `traits/www/terminal/terminal.js` (~200+ lines, ES module)

**Exports**:
- `createTerminal(mountEl, opts)` — returns {term, fitAddon, wasm}

**Structure**: Thin display layer over WASM-powered CLI session

**Terminal features**:
- xterm.js integration (CDN-loaded)
- FitAddon (responsive resize)
- WebLinksAddon (clickable URLs)
- Theme: dark GitHub-like colors
- Fonts: SF Mono, Fira Code, Cascadia Code
- History: 5000 lines scrollback
- Cursor: block, blinking

**WASM reuse**:
- Checks `window.TraitsWasm` (from SPA wasm-runtime.js)
- Falls back to standalone WASM load if not in SPA
- Uses `wasm.cli_input(data)` for stateful CLI dispatch

**CLEAR_SENTINEL & REST sentinel**:
- `\x1b[CLEAR]` — clear terminal
- `\x1b[REST]...\x1b[/REST]` — delegate to REST (used for helper-preferred traits)

**Flow**:
1. Mount xterm.js to element
2. Load WASM
3. Listen to user input
4. Call `wasm.cli_input()` with each keystroke
5. Receive rendered ANSI + clear/REST signals
6. Display to terminal
7. Cascade REST blocks to helper/server

**Status element**: Shows {trait_count} traits, {wasm_count} WASM-callable

---

## Duplication Summary

### traits.js vs index.html TraitsSDK
- **Identical functions**: discoverHelper(), probeHelper(), callHelper(), _ensureWebLLM()
- **Duplicated state**: wasm, helper, webllm local variables
- **Duplicated dispatch**: both implement WASM→helper→REST cascade
- **Why**: index.html was copy-pasted for zero-dependency boot (can run before traits.js loads)
- **Impact**: 2 separate SDK instances possible in a page + terminal env

### terminal.js
- **No duplication**: reuses `window.TraitsWasm` from SPA boot
- **Thin layer**: display only, all logic in WASM CliSession
- **WASM CLI state**: managed by WASM kernel, NOT by terminal.js

---

## Architecture Summary

```
┌─────────────────────────────────────────┐
│  Browser / Electron / Node.js Client    │
├──────────┬──────────────┬───────────────┤
│ traits.  │   SPA Shell  │   Terminal    │
│ js SDK   │  (index.html │  (terminal.js)│
│          │   TraitsSDK) │               │
├──────────┴──────────────┴───────────────┤
│  Dispatch Cascade (WASM→Helper→REST)    │
├──────────────────────────────────────────┤
│  WASM Kernel (traits_wasm.js + .wasm)    │
│  - 26 pure-computation traits            │
│  - CLI session state machine             │
│  - HTML page generators                  │
└──────────────────────────────────────────┘
           ↓↓↓ (optional)
┌──────────────────────────────────────────┐
│  Local Helper (Native Binary)            │
│  - Privileged traits (sys.ps, etc.)      │
│  - HTTP server at localhost:8090+        │
│  - Shares trait definitions with binary  │
└──────────────────────────────────────────┘
           ↓↓↓ (fallback)
┌──────────────────────────────────────────┐
│  Server (traits.build HTTP API)          │
│  - All traits available                  │
│  - Dylib plugin system                   │
│  - Full trait ecosystem                  │
└──────────────────────────────────────────┘
```

**Shared Code**: kernel/logic ← types + registry parsing (NO dispatch logic)

**Code Sharing via file paths**:
- WASM wasm_traits/mod.rs includes sys/*, www/* by #[path="..."]
- Native dispatcher includes all sys/*, www/* directly
- Results in: source code shared but compiled twice (WASM target + native target)
