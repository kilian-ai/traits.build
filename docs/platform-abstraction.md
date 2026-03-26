---
sidebar_position: 11
---

# Platform Abstraction Layer

The platform abstraction layer provides a unified API for platform-specific capabilities so trait source files can be **platform-agnostic** — no `#[cfg(target_arch)]` blocks needed.

**Path:** `traits/kernel/logic/src/platform/` (two files: `mod.rs` + `time.rs`)

## Why It Exists

The same `.rs` trait files are compiled for two targets:

- **Native** — `x86_64-apple-darwin` (or `linux-gnu`), the release binary
- **WASM** — `wasm32-unknown-unknown`, the browser kernel

Before the platform layer, each trait that needed dispatch, registry access, or time had to duplicate logic inside `#[cfg(target_arch = "wasm32")]` blocks. For example, `kernel.call` needed `crate::dispatcher::compiled::dispatch()` on native and `wasm_traits::dispatch()` on WASM. This scattered platform awareness across 12+ files with 131+ cfg blocks.

The `.trait.toml` dispatch system was considered but rejected — it serializes everything through JSON, which adds overhead for primitive operations like "get registry count" or "read a config value."

## Architecture

Two abstraction mechanisms, chosen based on the nature of the capability:

| Mechanism | Module | When Used | Overhead |
|-----------|--------|-----------|----------|
| **Compile-time** | `platform::time` | Pure functions with no runtime state dependency | Zero (cfg-gated at compile) |
| **Runtime** | `Platform` struct | Functions that access initialized runtime state (registry, config, secrets) | One function pointer indirection |

### Compile-Time: `platform::time`

The `time.rs` module uses cfg-gated implementations internally:

- **WASM**: `js_sys::Date::new_0()` for UTC components
- **Native**: `std::time::SystemTime` + Hinnant's algorithm for date conversion

Callers see a single API:

```rust
use kernel_logic::platform::time;

let (year, month, day, hour, min, sec) = time::now_utc();
```

The cfg blocks are hidden inside the platform module — trait source files stay clean.

### Runtime: `Platform` Struct

A struct of 6 function pointers stored in a `static OnceLock<Platform>`:

```rust
pub struct Platform {
    pub dispatch: fn(&str, &[Value]) -> Option<Value>,     // trait dispatch
    pub registry_all: fn() -> Vec<Value>,                   // all traits as JSON
    pub registry_count: fn() -> usize,                      // trait count
    pub registry_detail: fn(&str) -> Option<Value>,         // single trait detail
    pub config_get: fn(&str, &str, &str) -> String,         // per-trait config
    pub secret_get: fn(&str) -> Option<String>,             // secret retrieval
}
```

Initialized once at startup. Each target provides its own adapter implementations.

## Platform Services

| Service | Convenience Function | Returns |
|---------|---------------------|---------|
| Dispatch | `platform::dispatch(path, args)` | `Option<Value>` |
| Registry list | `platform::registry_all()` | `Vec<Value>` — JSON summary objects |
| Registry count | `platform::registry_count()` | `usize` |
| Registry detail | `platform::registry_detail(path)` | `Option<Value>` — full trait JSON |
| Config | `platform::config_get(trait_path, key, default)` | `String` |
| Secrets | `platform::secret_get(key)` | `Option<String>` |
| Time | `platform::time::now_utc()` | `(u32, u32, u32, u32, u32, u32)` |

All convenience functions are in `kernel_logic::platform::*`.

## Initialization

### Native (`kernel/main/main.rs` → `bootstrap()`)

Called after `globals::init()` populates the registry, before dylib loading:

```rust
kernel_logic::platform::init(Platform {
    dispatch:        |path, args| crate::dispatcher::compiled::dispatch(path, args),
    registry_all:    || { /* REGISTRY.get().all() → sorted → to_summary_json() */ },
    registry_count:  || REGISTRY.get().map(|r| r.len()).unwrap_or(0),
    registry_detail: |path| REGISTRY.get()?.get(path).map(|t| t.to_json()),
    config_get:      |t, k, d| crate::config::trait_config_or(t, k, d),
    secret_get:      |key| SecretContext::resolve(&[key]).get(key).map(|v| v.to_string()),
});
```

### WASM (`kernel/wasm/src/lib.rs` → `init()`)

Called after `get_registry()` builds the WASM registry:

```rust
kernel_logic::platform::init(Platform {
    dispatch:        |path, args| wasm_traits::dispatch(path, args),
    registry_all:    || { /* get_registry().all() → serde_json::to_value() */ },
    registry_count:  || get_registry().len(),
    registry_detail: |path| { /* get_registry().get(path) → JSON with all fields */ },
    config_get:      |_t, _k, d| d.to_string(),  // no config in browser
    secret_get:      |key| wasm_secrets::get_secret(key),
});
```

**Key difference**: `config_get` on WASM always returns the default value — there's no `traits.toml` config system in the browser.

**Initialization order matters**: Platform must be initialized AFTER the registry is populated. On native, `globals::init()` must complete first. On WASM, `get_registry()` must return first.

## Migrated Traits

These 7 traits now use `kernel_logic::platform::*` instead of `#[cfg]` blocks:

### kernel.call

Inter-trait dispatch by dot-path. Previously had cfg blocks to choose between `compiled::dispatch()` (native) and `wasm_traits::dispatch()` (WASM).

```rust
// Before: 2 cfg blocks
// After:
kernel_logic::platform::dispatch(trait_path, &call_args)
```

### sys.info / sys.list

Both delegate to `sys.registry` via platform dispatch:

```rust
kernel_logic::platform::dispatch("sys.registry", &[json!("info"), json!(path)])
kernel_logic::platform::dispatch("sys.registry", &[json!("list"), json!(namespace)])
```

### sys.registry

Three helper functions delegate directly to platform accessors:

```rust
fn get_all_entries() -> Vec<Value>       { platform::registry_all() }
fn get_entry_detail(p: &str) -> Option<Value> { platform::registry_detail(p) }
fn registry_count() -> usize             { platform::registry_count() }
```

### sys.llm

The `dispatch_sys_call()` helper for HTTP calls:

```rust
kernel_logic::platform::dispatch("sys.call", &[json!(args)])
```

### sys.version

Uses both compile-time and runtime abstractions:

```rust
let (y, mo, d, h, m, s) = kernel_logic::platform::time::now_utc();
let count = kernel_logic::platform::registry_count();
```

**Note**: 2 cfg blocks remain for `env!("TRAITS_BUILD_VERSION")` vs `env!("CARGO_PKG_VERSION")` — these are compile-time constants that inherently differ between build targets and cannot be abstracted.

### www.admin

Config access for Fly.io app name:

```rust
kernel_logic::platform::config_get("www.admin", "fly_app", "polygrait-api")
```

## Boundaries — What's NOT Migrated

### Kernel Infrastructure (4 files)

`kernel/config`, `kernel/dispatcher`, `kernel/registry`, `kernel/types` — these ARE the platform internals. They implement the adapters that fill the Platform struct. Abstracting them would be circular.

### Dylib Traits

`sys.checksum`, `sys.ps`, `www.traits.build` — dylib traits are loaded as separate shared libraries at runtime. Each dylib gets its own copy of the `kernel-logic` crate's static address space. The `OnceLock<Platform>` in the dylib's copy is never initialized by the kernel's `platform::init()` call.

**Rule**: Only builtin traits (compiled as modules into the main binary via `#[path = "..."] pub mod`) share the binary's `OnceLock`. Dylib traits must use `#[cfg]` blocks or the C ABI `server_dispatch` callback for cross-trait calls.

### HTTP Client (`sys/call.rs`)

Uses `reqwest` on native and would need browser `fetch()` on WASM. The implementations are too fundamentally different for fn pointer abstraction — different async runtimes, different error types, different connection handling.

### Heavily Native Traits

- `sys/test_runner.rs` — ~34 cfg blocks, filesystem I/O + process spawning
- `sys/chat_learnings.rs` — ~16 cfg blocks, reads JSON from filesystem
- `sys/chat_protocols/vscode.rs` — ~8 cfg blocks, filesystem-heavy
- `sys/chat_workspaces.rs` — ~5 cfg blocks, scans filesystem directories

These traits have no meaningful WASM implementation. Their cfg blocks exist to provide compile-time stubs or no-ops on WASM.

### Inherently Target-Specific

- `llm/prompt/webllm.rs` — WASM-only (WebGPU), 2 cfg blocks. Cannot run on native.
- `sys/openapi.rs` — 8 cfg blocks, registry access patterns too specialized for generic platform API.

## Developer Guide

### When to Use `platform::*`

Use platform abstractions when your trait:

1. Compiles for **both** native and WASM (i.e., has `wasm = true` in `.trait.toml`)
2. Needs one of the 6 platform services or time
3. Is a **builtin** trait (not a dylib)

### When NOT to Use It

Use `#[cfg]` blocks directly when your trait:

- Is **native-only** (no `wasm = true`) — cfg blocks are simpler
- Is **WASM-only** (e.g., `llm.prompt.webllm`) — no need for abstraction
- Is a **dylib** — platform `OnceLock` won't be initialized in dylib address space
- Needs capabilities not in the Platform struct (filesystem, networking, process spawning)

### Adding a New Platform Service

1. **Add a field** to the `Platform` struct in `traits/kernel/logic/src/platform/mod.rs`
2. **Add a convenience accessor** function below the existing ones
3. **Wire the native adapter** in `traits/kernel/main/main.rs` → `bootstrap()`
4. **Wire the WASM adapter** in `traits/kernel/wasm/src/lib.rs` → `init()`
5. **Migrate traits** that previously used `#[cfg]` for this capability

Example — adding a `log` service:

```rust
// 1. In platform/mod.rs — add to Platform struct:
pub log: fn(&str, &str),  // (level, message)

// 2. Convenience accessor:
pub fn log(level: &str, msg: &str) {
    (platform().log)(level, msg)
}

// 3. Native adapter (main.rs):
log: |level, msg| eprintln!("[{level}] {msg}"),

// 4. WASM adapter (wasm/lib.rs):
log: |level, msg| web_sys::console::log_1(&format!("[{level}] {msg}").into()),
```

### Behavioral Differences Between Targets

| Service | Native | WASM | Impact |
|---------|--------|------|--------|
| `config_get` | Reads `traits.toml` | Returns default always | Traits using config must have sensible defaults |
| `secret_get` | AES-256-GCM encrypted store | Browser `localStorage` | Different security models |
| `registry_count` | All ~58 builtin + dylib traits | ~26 WASM-compiled traits | Count differs significantly |
| `dispatch` | Tries dylib first, then compiled | WASM dispatch table only | Some traits unavailable in browser |

### Common Patterns

**Delegate to another trait via dispatch:**
```rust
use kernel_logic::platform;

pub fn my_trait(args: &[Value]) -> Value {
    match platform::dispatch("some.other.trait", args) {
        Some(result) => result,
        None => json!({"error": "trait not found"}),
    }
}
```

**Read config with fallback:**
```rust
let port = platform::config_get("my.trait", "port", "8080");
```

**Access registry metadata:**
```rust
let count = platform::registry_count();
let all = platform::registry_all();  // Vec<Value> of summary JSON
if let Some(detail) = platform::registry_detail("sys.checksum") {
    // full trait definition as JSON
}
```

## File Reference

| File | Purpose |
|------|---------|
| `traits/kernel/logic/src/platform/mod.rs` | Platform struct, OnceLock, init(), 6 convenience accessors |
| `traits/kernel/logic/src/platform/time.rs` | Compile-time cfg-gated `now_utc()` |
| `traits/kernel/logic/src/lib.rs` | `pub mod platform;` declaration |
| `traits/kernel/logic/Cargo.toml` | `js-sys` conditional dep for wasm32 |
| `traits/kernel/main/main.rs` | Native adapter wiring in `bootstrap()` |
| `traits/kernel/wasm/src/lib.rs` | WASM adapter wiring in `init()` |

## Remaining cfg Block Inventory

~95 cfg blocks across 14 files remain outside the platform module. These are intentionally NOT migrated:

| Category | Files | Reason |
|----------|-------|--------|
| Kernel infrastructure | config, dispatcher, registry, types | They ARE the platform internals |
| Dylib traits | checksum, ps, www.traits.build | Separate address space |
| HTTP client | sys/call | Too platform-specific |
| Filesystem-heavy | test_runner, chat_*, chat_protocols | Native-only |
| Target-specific | webllm, openapi | Inherently single-target |
| Build-time constants | sys/version (2 blocks) | `env!()` macros |
