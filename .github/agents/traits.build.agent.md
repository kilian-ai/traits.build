# traits.build — Agent Instructions

> Pure Rust kernel for the Traits platform. Deployed at traits.build via Fly.io.
>
> - **Repository:** https://github.com/kilian-ai/traits.build
> - **Homepage:** https://traits.build/

---

## Project Overview

**traits.build** is the Rust-only kernel of the Traits platform. No JavaScript workers, no Python workers, no GUI, no polyglot — just a clean Rust kernel that compiles every trait directly into a single binary.

---

## Directory Structure

```
Polygrait/A. traits.build/
├── build.rs              # Build system: trait discovery, codegen, dispatch table
├── Cargo.toml            # Workspace: root + plugin_api + www.build (cdylib)
├── Cargo.lock
├── Dockerfile            # Multi-stage: rust:latest → debian:trixie-slim
├── fly.toml              # Fly.io: app=polygrait-api, region=iad, port=8090
├── traits.toml           # Runtime config: port, timeout, traits_dir
├── build.sh              # Build script
├── src/
│   └── main.rs           # Binary entry point (thin — delegates to kernel.main)
├── traits/
│   ├── kernel/           # 11 kernel modules (compiled in)
│   │   ├── call/         # Inter-trait dispatch
│   │   ├── config/       # traits.toml parsing + env var overrides
│   │   ├── dispatcher/   # Path resolution, arg validation, compiled dispatch
│   │   ├── dylib_loader/ # cdylib plugin discovery and loading
│   │   ├── globals/      # OnceLock statics: REGISTRY, CONFIG, TRAITS_DIR
│   │   ├── main/         # Bootstrap, trait-exists probe, introspection
│   │   ├── plugin_api/   # C ABI export macro (workspace member)
│   │   ├── registry/     # DashMap of trait defs, interface resolution
│   │   ├── reload/       # Hot-reload registry from disk
│   │   ├── serve/        # actix-web HTTP server, CORS, SSE streaming
│   │   └── types/        # TraitValue, TraitType, type coercion
│   ├── sys/              # 11 system traits (compiled as builtins)
│   │   ├── checksum/     # SHA-256 hashing
│   │   ├── cli/          # Clap parsing, subcommand dispatch
│   │   ├── info/         # Show detailed trait metadata
│   │   ├── list/         # List all traits
│   │   ├── mcp/          # MCP stdio server (JSON-RPC 2.0)
│   │   ├── openapi/      # OpenAPI 3.0 spec generation
│   │   ├── ps/           # List running background traits
│   │   ├── registry/     # Registry read API (tree, namespaces, search)
│   │   ├── snapshot/     # Snapshot trait versions
│   │   ├── test_runner/  # Run .features.json tests
│   │   └── version/      # YYMMDD version generation
│   └── www/              # 5 web traits
│       ├── admin/        # Admin dashboard + deploy/scale/destroy actions
│       │   ├── admin.rs
│       │   ├── deploy/
│       │   ├── destroy/
│       │   └── scale/
│       └── traits/build/ # Landing page (this site)
└── scripts/
```

---

## Build System

The `build.rs` at project root does all code generation:

1. **Trait discovery** — scans `traits/` recursively for `.trait.toml` files
2. **`builtin_traits.rs`** — embeds all TOML definitions via `include_str!`
3. **`compiled_traits.rs`** — generates module declarations + `dispatch_compiled()` function
4. **`kernel_modules.rs`** — crate-level `mod` declarations for kernel subdirectories
5. **`cli_formatters.rs`** — optional CLI output formatters
6. **Version management** — YYMMDD format, auto-bumps on same-day changes
7. **Checksum validation** — SHA-256 of .rs files, bumps version if changed

Build & run:
```bash
cargo build --release
./target/release/traits serve --port 8090
# or just:
./target/release/traits   # reads TRAITS_PORT env var
```

### build.rs Deep Internals

#### Trait Detection (visit_traits function)

**Builtin traits** (source = "builtin" or "kernel"):
- Must have a `.rs` sibling (e.g., `traits/sys/checksum/checksum.rs`)
- Are compiled directly into the kernel binary
- Trait path derived from directory structure: `traits/sys/checksum/checksum.trait.toml` → `sys.checksum`

**Dylib traits** (source = "dylib"):
- Have a `.rs` file + compiled to cdylib (`lib<name>.dylib`)
- Loaded at runtime by dylib_loader
- Must match C ABI from plugin_api

Key logic:
1. Walks traits/ recursively
2. For each `.trait.toml`:
   - Parses `source`, `entry`, `background`, `callable` fields
   - If builtin: checks for sibling `.rs` file
   - If sibling exists: registers as TraitModule
   - Updates `.trait.toml` checksum (bumps version if .rs changed)
3. **Kernel traits**: if path starts with "kernel." AND mod_name != "main", also register as KernelModule (crate-level mod)
4. **CLI formatters**: discovers `<name>_cli.rs` companion files for CLI output formatting

#### Generated Code Examples

**builtin_traits.rs** — Array of BuiltinTraitDef structs:
```rust
pub const BUILTIN_TRAIT_DEFS: &[BuiltinTraitDef] = &[
    BuiltinTraitDef { 
        path: "sys.checksum", 
        rel_path: "traits/sys/checksum/checksum.trait.toml",
        toml: include_str!(...)  // Full TOML content
    },
    ...
];
```

**compiled_traits.rs** — Module declarations + dispatch functions:
```rust
#[path = "/absolute/path/traits/sys/checksum/checksum.rs"]
pub mod checksum;

pub fn dispatch_compiled(trait_path: &str, args: &[Value]) -> Option<Value> {
    match trait_path {
        "sys.checksum" => Some(checksum::checksum_dispatch(args)),
        ...
    }
}

// dispatch: unified - tries dylib_loader first, then compiled
pub fn dispatch(trait_path: &str, args: &[Value]) -> Option<Value> {
    if let Some(loader) = dylib_loader::LOADER.get() {
        if let Some(result) = loader.dispatch(trait_path, args) {
            return Some(result);
        }
    }
    dispatch_compiled(trait_path, args)
}

// dispatch_async: for background = true traits (async entry points)
pub async fn dispatch_async(trait_path: &str, args: &[TraitValue]) -> Option<Result<TraitValue>> { ... }
```

**kernel_modules.rs** — Crate-level mod declarations:
```rust
#[path = "/absolute/path/traits/kernel/types/types.rs"]
pub mod types;
#[path = "/absolute/path/traits/kernel/dispatcher/dispatcher.rs"]
pub mod dispatcher;
```
Allows kernel/ code to be accessed as `crate::types`, `crate::dispatcher`, etc.

#### Build Version Management

- Reads/writes `traits/sys/version/version.trait.toml`
- Format: `vYYMMDD` or `vYYMMDD.HHMMSS` if multiple builds on same day
- Syncs to Cargo.toml as `0.YYMMDD.HHMMSS`
- Re-run detection: watches entire traits/ tree for changes

---

## plugin_api Crate: C ABI Contract

**Path**: `traits/kernel/plugin_api/`
**Type**: Library (not cdylib)
**Purpose**: Provides the `export_trait!` macro for cdylib plugins

### The export_trait! Macro

```rust
plugin_api::export_trait!(build::website);
```

Generates two C ABI functions in the dylib:

#### `trait_call(json_ptr: *const u8, json_len: usize, out_len: *mut usize) -> *mut u8`

**Caller** (dylib_loader in kernel):
1. Serialize args to JSON bytes
2. Pass pointer + length to trait_call
3. Receive result pointer + length written to out_len
4. Read result bytes, deserialize
5. Call trait_free to release memory

**Implementation**:
1. Deserialize JSON bytes → Vec<Value>
2. Call the target function with &[Value]
3. Serialize result to JSON bytes
4. Allocate Vec, forget it to leak the pointer
5. Return pointer, set out_len

#### `trait_free(ptr: *mut u8, len: usize)`

Reconstructs Vec from raw parts and drops it (deallocates).

---

## dylib_loader: Runtime Trait Loading

**Path**: `traits/kernel/dylib_loader/`

### Key Structures

```rust
struct LoadedTrait {
    _lib: libloading::Library,  // Keeps the library in memory
    call: TraitCallFn,           // unsafe extern "C" fn(...)
    free: TraitFreeFn,           // unsafe extern "C" fn(...)
    path: String,                // Trait path, e.g. "www.traits.build"
    dylib_path: PathBuf,         // Where loaded from
}

pub struct DylibLoader {
    traits: Arc<RwLock<HashMap<String, LoadedTrait>>>,
    search_dirs: Vec<PathBuf>,
}
```

### Discovery: Two Modes

**Mode 1: Filename Convention** — `libsys_checksum.dylib` → `sys.checksum` (convert first underscore to dot)

**Mode 2: TOML Discovery (Preferred)** — Find `.trait.toml` with `source = "dylib"`, look for companion `lib<dir_name>.dylib`. Priority: TOML processed first; Mode 1 skipped if .trait.toml governance exists.

### Loading Process

1. Load shared library via libloading
2. Verify symbols exist: trait_call, trait_free
3. Optionally call trait_init(server_dispatch) if exported
4. Store LoadedTrait in HashMap[trait_path]

### Cross-Trait Dispatch

Dylibs can call other traits via optional trait_init:
- Kernel passes server_dispatch callback
- Dylib calls server_dispatch(json_dispatch_request) → result bytes
- Request format: `{"path": "sys.checksum", "args": [...]}`
- server_dispatch tries LOADER first, falls back to dispatch_compiled

### Global LOADER

```rust
pub static LOADER: OnceLock<Arc<DylibLoader>> = OnceLock::new();
```

Set once at startup; accessed by dispatch callbacks and kernel plugins.

---

## Dispatch Flow (Unified)

### From External Call

1. **Caller** → kernel dispatcher
2. **Dispatcher** calls dispatch(trait_path, args)
3. **dispatch()** (from compiled_traits.rs):
   - Checks LOADER.dispatch() first (dylib loader)
   - Falls back to dispatch_compiled() if not found
4. **Dylib dispatch**: loads trait, calls trait_call
5. **Compiled dispatch**: direct Rust function call

### From Dylib Cross-Trait

1. **Dylib** calls server_dispatch(request_json)
2. **server_dispatch** parses `{"path": "...", "args": [...]}`
3. Tries LOADER.dispatch() first, falls back to dispatch_compiled()

---

## Example cdylib Trait: www.traits.build

**Path**: `traits/www/traits/build/`

```toml
# Cargo.toml
[package]
name = "trait-www-traits-build"
crate-type = ["cdylib"]
[dependencies]
plugin_api = { path = "../../../kernel/plugin_api" }
serde_json = "1"
```

```rust
// lib.rs
#[path = "website.rs"]
mod build;
plugin_api::export_trait!(build::website);
```

```toml
# build.trait.toml
source = "dylib"   # Triggers dylib_loader scanning
entry = "website"   # Name of exported function
```

---

## Key Design Principles

1. **Builtin first, dylib second**: compiled Rust traits for speed, dylib for hot-reloading
2. **C ABI bridge**: trait_call/trait_free allow any language to export traits
3. **JSON serialization**: universal interface between kernel and plugins
4. **Trait path hierarchy**: namespace.name (sys.checksum, www.traits.build, kernel.serve)
5. **Async support**: background = true traits get dispatch_async entry point
6. **Version tracking**: .trait.toml versioning + checksum bumping on source change
7. **Hot-reload ready**: dylib_loader supports reload, all traits via dispatch()

### Component Summary

| Component | Location | Purpose | Role |
|-----------|----------|---------|------|
| build.rs | root | Discovers traits, gen code | Compile-time |
| plugin_api | kernel/plugin_api | C ABI export macro | Library (shared) |
| dylib_loader.rs | kernel/dylib_loader | Runtime .dylib loading | Kernel module |
| compiled_traits.rs | OUT_DIR (gen) | Dispatch to compiled traits | Dispatch router |
| builtin_traits.rs | OUT_DIR (gen) | TOML registry data | Registry seed |
| cli_formatters.rs | OUT_DIR (gen) | CLI output formatting | Optional |
| kernel_modules.rs | OUT_DIR (gen) | Crate-level kernel/ mods | Module tree |
| www.traits.build | traits/www/traits/build | Example cdylib trait | Template (cdylib) |

---

## API Convention

```
# REST — POST with args array
POST http://127.0.0.1:8090/traits/{namespace}/{name}
Body: {"args": [arg1, arg2, ...]}

# CLI — every sys.* trait is a direct subcommand
traits list
traits checksum hash "hello"
traits info sys.checksum
traits test_runner '*'

# Health check (used by Fly.io)
GET /health

# MCP — stdio JSON-RPC 2.0
traits mcp
# Reads JSON-RPC from stdin, writes responses to stdout.
# All traits are exposed as tools: dot paths → underscore names
# e.g. sys.checksum → sys_checksum
```

---

## Trait Inventory (29 traits)

### Kernel (11) — Core runtime

| Trait | Description | Provides |
|-------|-------------|----------|
| `kernel.main` | Entry point, bootstrap, introspection | — |
| `kernel.dispatcher` | Path resolution, arg validation, compiled dispatch | `kernel/dispatcher` |
| `kernel.registry` | DashMap trait lookup, interface resolution | `kernel/registry` |
| `kernel.config` | traits.toml + env var overrides | `kernel/config` |
| `kernel.serve` | actix-web HTTP server (background) | `kernel/serve` |
| `kernel.dylib_loader` | cdylib plugin discovery and loading | — |
| `kernel.types` | TraitValue, TraitType, type parsing | `kernel/types` |
| `kernel.globals` | OnceLock statics (REGISTRY, CONFIG, etc.) | `kernel/globals` |
| `kernel.call` | Inter-trait dispatch by dot-notation | — |
| `kernel.plugin_api` | C ABI export macro for cdylib plugins | `kernel/plugin_api` |
| `kernel.reload` | Hot-reload registry from disk | — |

### Sys (11) — System utilities

| Trait | Description | Provides |
|-------|-------------|----------|
| `sys.cli` | Clap parsing, subcommand dispatch, pipe support | `sys/cli` |
| `sys.registry` | Read API: list, info, tree, namespaces, search | `sys/registry` |
| `sys.checksum` | SHA-256 checksums (values, I/O pairs, signatures) | `sys/checksum` |
| `sys.mcp` | MCP stdio server — JSON-RPC 2.0 over stdin/stdout | `sys/mcp` |
| `sys.openapi` | OpenAPI 3.0 spec generation from trait registry | `sys/openapi` |
| `sys.version` | YYMMDD version string generation | `sys/version` |
| `sys.snapshot` | Snapshot trait version to date format | `sys/snapshot` |
| `sys.test_runner` | Run .features.json tests (dispatch + shell) | `sys/test_runner` |
| `sys.list` | List all registered traits | `sys/list` |
| `sys.info` | Detailed trait metadata + signatures | `sys/info` |
| `sys.ps` | List running background trait processes | — |

### WWW (7) — Web interface

| Trait | Description | Provides |
|-------|-------------|----------|
| `www.traits.build` | Landing page HTML | `www/webpage` |
| `www.docs.api` | API documentation (Redoc) | `www/webpage` |
| `www.admin` | Admin dashboard (Basic Auth) | `www/webpage` |
| `www.admin.deploy` | Deploy to Fly.io | — |
| `www.admin.fast_deploy` | Fast deploy via Docker + sftp | — |
| `www.admin.scale` | Scale Fly.io machines (0=stop, 1+=start) | — |
| `www.admin.destroy` | Destroy Fly.io machines | — |

---

## Interface System

Traits declare dependencies via `[requires]` / `[bindings]` / `provides` in `.trait.toml`:

```toml
[trait]
provides = ["kernel/dispatcher"]

[requires]
registry = "kernel/registry"

[bindings]
registry = "kernel.registry"
```

Resolution chain: **per-call overrides → global bindings → caller bindings → auto-discover**

Key dependency examples:
- `kernel.serve` requires: `www/webpage`, `kernel/dispatcher`
- `kernel.main` requires: `kernel/dispatcher`, `kernel/registry`, `kernel/config`, `kernel/globals`, `kernel/dylib_loader`
- `sys.cli` requires: `kernel/config`, `kernel/call`, `kernel/serve`

---

## Type System

```rust
TraitType: int, float, string, bool, bytes, null, any, handle, list<T>, map<K,V>, T?
TraitValue: Null, Bool, Int(i64), Float(f64), String, List, Map, Bytes
```

Conversions: `to_json()`, `from_json()`, type coercion at dispatch time.

---

## Deployment (Fly.io)

- **App:** `polygrait-api`
- **Region:** `iad` (Ashburn, Virginia)
- **IPv4:** `66.241.125.245`
- **IPv6:** `2a09:8280:1::d5:c7cb:0`
- **VM:** shared CPU, 1 vCPU, 512 MB RAM
- **Port:** 8090 (internal), HTTPS forced
- **Auto-scaling:** 0–2 machines, auto-stop/auto-start
- **Health check:** `GET /health` every 30s
- **Admin auth:** HTTP Basic Auth (`ADMIN_PASSWORD` Fly secret)

### Deploy workflow:
```bash
cd "Polygrait/A. traits.build"
# Build for amd64 (Fly runs amd64, Mac is aarch64)
docker buildx build --platform linux/amd64 -t registry.fly.io/polygrait-api:deployment-vN .
# Push and deploy
fly deploy --now --local-only --image registry.fly.io/polygrait-api:deployment-vN
```

### Dockerfile:
- Stage 1: `rust:latest` — `cargo build --release`
- Stage 2: `debian:trixie-slim` — only `ca-certificates`, `curl`
- No Node, no Python, no workers
- `CMD ["traits"]` — reads `TRAITS_PORT` env var

---

## Testing

Each trait has a `.features.json` file with example-based and command-based tests:

```bash
# Run all tests
traits test_runner '*'

# Run specific namespace
traits test_runner 'sys.*'
```

Test types: `exit_code`, `contains`, `matches` (regex), `json_path`

---

## What's NOT in this branch

| Component | Status | Reason |
|-----------|--------|--------|
| JS/Python workers | Not present | Rust-only kernel |
| GUI (app.js, Blockly, etc.) | Not present | API-only server |
| Transpilers | Not present | No code generation |
| MCP server | **Present** | `sys.mcp` — native Rust, stdio transport, `traits mcp` |
| Programs (saved JSON) | Not present | No program storage |
| trait_defs/ (238 traits) | Not present | Replaced by 25 compiled traits |
| Organization/Chat/Packages | Not present | Minimal core only |
| Browser automation | Not present | No Playwright traits |
| Networking traits | Not present | No HTTP/IRC/DNS traits |

---

## Conventions

- **All code is Rust.** No JS/Python traits in this branch.
- **`source = "builtin"`** for all compiled traits. `source = "dylib"` for cdylib plugins only.
- **Trait files live in `traits/{namespace}/{name}/`** — each directory has `name.trait.toml` + `name.rs` + `name.features.json`
- **build.rs auto-discovers** everything — no manual module registration needed.
- **Interface wiring is mandatory** — declare `[requires]` and `[bindings]` for all cross-trait dependencies.
- **Version format:** `vYYMMDD` or `vYYMMDD.HHMMSS` for same-day bumps.
- **After modifying traits:** rebuild with `cargo build --release`; checksums and versions auto-update.

## Trait .trait.toml Template

```toml
[trait]
description = "Short description"
version = "v260321"
author = "system"
tags = ["namespace", "category"]
provides = ["namespace/interface"]

[signature]
params = [
  { name = "param1", type = "string", description = "desc", required = true },
]

[signature.returns]
type = "string"
description = "Return description"

[implementation]
language = "rust"
source = "builtin"
entry = "function_name"

[requires]
dep = "namespace/interface"

[bindings]
dep = "namespace.concrete_trait"
```


## AGENT RULES:
- Always commit to git after making changes to the codebase, with a clear and concise commit message describing the changes made.
- Always include build-generated files in your commits — `build.rs` auto-bumps versions in `.trait.toml` files, `Cargo.toml`, and `Cargo.lock` on every build. Run `git add -A` (not just the files you edited) to capture all version bumps, checksum updates, and generated TOML changes.
- Always run the build script (`build.sh`) after making changes to ensure that the code compiles correctly and that any generated files are updated.
- Always rebuild the binary and restart the local server after making changes to the codebase to ensure that the changes take effect and to test that everything is working correctly.
- Always store memory files you are creating in .github/memories so we can save them for future use.
- Never forget to create a features.json file for any new trait you create, and to add example-based tests in that file to ensure the trait works as expected.