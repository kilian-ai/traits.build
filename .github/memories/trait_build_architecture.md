# Traits.Build Complete Architecture

## Workspace Structure (Root Cargo.toml)

```toml
[workspace]
members = [
    ".",              # Main kernel binary
    "traits/kernel/plugin_api",  # C ABI export macro lib
    "traits/www/traits/build",   # Example cdylib plugin
]
```

**Key bin**: `traits` (src/main.rs) - the kernel/server

---

## Build.rs: Trait Discovery & Code Generation

### Phase 1: Trait Detection

The build script scans `traits/` recursively for `.trait.toml` files and discovers **builtin vs dylib** traits:

**Builtin traits** (source = "builtin" or "kernel"):
- Must have a `.rs` sibling (e.g., `traits/sys/checksum/checksum.rs`)
- Are compiled directly into the kernel binary
- Trait path derived from directory structure: `traits/sys/checksum/checksum.trait.toml` → `sys.checksum`

**Dylib traits** (source = "dylib"):
- Have a `.rs` file + compiled to cdylib (`lib<name>.dylib`)
- Loaded at runtime by dylib_loader
- Must match C ABI from plugin_api

**Key logic** (visit_traits function):
1. Walks traits/ recursively
2. For each `.trait.toml`:
   - Parses `source`, `entry`, `background`, `callable` fields
   - If builtin: checks for sibling `.rs` file
   - If sibling exists: registers as TraitModule
   - Updates `.trait.toml` checksum (bumps version if .rs changed)
3. **Kernel traits**: if path starts with "kernel." AND mod_name != "main", also register as KernelModule (crate-level mod)
4. **CLI formatters**: discovers `<name>.cli.rs` companion files for CLI output formatting

### Phase 2: Code Generation

Generates three files in OUT_DIR (included at build time):

#### 1. `builtin_traits.rs`
Array of BuiltinTraitDef structs:
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
Used by registry to populate trait definitions at runtime.

#### 2. `compiled_traits.rs`
Module declarations + dispatch functions:
```rust
#[path = "/absolute/path/traits/sys/checksum/checksum.rs"]
pub mod checksum;

// dispatch_compiled: traitmatch on trait_path → function call
pub fn dispatch_compiled(trait_path: &str, args: &[Value]) -> Option<Value> {
    match trait_path {
        "sys.checksum" => Some(checksum::checksum_dispatch(args)),
        ...
    }
}

// dispatch_trait_value: TraitValue interface for workers
pub fn dispatch_trait_value(trait_path: &str, args: &[TraitValue]) -> Option<TraitValue> { ... }

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

// list_compiled: returns all compiled trait paths
pub fn list_compiled() -> Vec<&'static str> { ... }
```

#### 3. `cli_formatters.rs`
Module declarations + format_cli dispatch:
```rust
#[path = "/absolute/path/traits/sys/checksum/checksum.cli.rs"]
pub mod checksum_cli;

pub fn format_cli(trait_path: &str, result: &Value) -> Option<String> {
    match trait_path {
        "sys.checksum" => Some(checksum_cli::format_cli(result)),
        ...
    }
}
```

#### 4. `kernel_modules.rs`
Crate-level mod declarations for kernel/ modules:
```rust
#[path = "/absolute/path/traits/kernel/types/types.rs"]
pub mod types;

#[path = "/absolute/path/traits/kernel/dispatcher/dispatcher.rs"]
pub mod dispatcher;
...
```
Allows kernel/ code to be accessed as `crate::types`, `crate::dispatcher`, etc.

### Build Version Management

- Reads/writes `traits/sys/version/version.trait.toml`
- Format: `vYYMMDD` or `vYYMMDD.HHMMSS` if multiple builds on same day
- Syncs to Cargo.toml as `0.YYMMDD.HHMMSS`
- Re-run detection: watches entire traits/ tree for changes

---

## plugin_api Crate: C ABI Contract

**Path**: `traits/kernel/plugin_api/`
**Type**: Library (not cdylib)
**Purpose**: Provides the export_trait! macro for cdylib plugins

### The export_trait! Macro

```rust
plugin_api::export_trait!(build::website);
```

Generates two C ABI functions in the dylib:

#### 1. `trait_call(json_ptr: *const u8, json_len: usize, out_len: *mut usize) -> *mut u8`

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

#### 2. `trait_free(ptr: *mut u8, len: usize)`

Reconstructs Vec from raw parts and drops it (deallocates).

### plugin_api.rs Entry Point

Only compiled in kernel binary (behind cfg(kernel)):
```rust
pub fn plugin_api(args: &[Value]) -> Value {
    // Query dylib_loader for installed plugins
    serde_json::json!({
        "abi": {
            "version": 1,
            "entry": "trait_call(...)",
            "free": "trait_free(...)",
            "convention": "C",
            "format": "JSON bytes in, JSON bytes out"
        },
        "installed_plugins": loader.list(),
        "plugin_count": ...
    })
}
```

---

## dylib_loader.rs: Runtime Trait Loading

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

#### Mode 1: Filename Convention
- Pattern: `libsys_checksum.dylib` → trait path `sys.checksum`
- Convert first underscore to dot: `sys_checksum` → `sys.checksum`
- Function: dylib_name_to_trait_path()

#### Mode 2: TOML Discovery (Preferred)
- Find `.trait.toml` with `source = "dylib"`
- Look for companion: `lib<dir_name>.dylib` or `libtrait.dylib`
- Derive trait path from dir structure: `traits/www/traits/build/` → `www.traits.build`
- Function: try_load_toml_dylib()

**Priority**: TOML is processed first; Mode 1 skipped if .trait.toml governance exists.

### Loading Process

1. Load shared library via libloading
2. Verify symbols exist: trait_call, trait_free
3. Optionally call trait_init(server_dispatch) if exported
4. Store LoadedTrait in HashMap[trait_path]

### Dispatch

```rust
pub fn dispatch(&self, trait_path: &str, args: &[Value]) -> Option<Value>
```

1. Serialize args to JSON bytes
2. Call trait_call(ptr, len, &mut out_len)
3. Deserialize result from returned bytes
4. Free via trait_free(ptr, out_len)

### Cross-Trait Dispatch

Dylibs can call other traits via optional trait_init:
- Kernel passes server_dispatch callback
- Dylib calls server_dispatch(json_dispatch_request) → result bytes
- Request format: `{"path": "sys.checksum", "args": [...]}`
- server_dispatch tries LOADER first, falls back to dispatch_compiled

### Global LOADER

```rust
pub static LOADER: OnceLock<Arc<DylibLoader>> = OnceLock::new();
pub fn set_global_loader(loader: Arc<DylibLoader>) { ... }
```

Set once at startup; accessed by dispatch callbacks and kernel plugins.

---

## Example: trait-www-traits-build cdylib

**Path**: `traits/www/traits/build/`

### Cargo.toml

```toml
[package]
name = "trait-www-traits-build"
crate-type = ["cdylib"]

[dependencies]
plugin_api = { path = "../../../kernel/plugin_api" }
serde_json = "1"
```

### lib.rs

```rust
#[path = "website.rs"]
mod build;

plugin_api::export_trait!(build::website);
```

Uses the macro to export C ABI entry points.

### website.rs

```rust
pub fn website(_args: &[Value]) -> Value {
    // Query kernel globals
    let (trait_count, ns_count) = match crate::globals::REGISTRY.get() {
        Some(reg) => {
            let all = reg.all();
            let namespaces: HashSet<&str> = all.iter()
                .filter_map(|e| e.path.split('.').next())
                .collect();
            (all.len(), namespaces.len())
        }
        None => (0, 0),
    };
    
    // Return HTML
    Value::String(html)
}
```

#### build.trait.toml

```toml
source = "dylib"  # This triggers dylib_loader scanning
entry = "website" # Name of exported function
```

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
2. **server_dispatch** (in dylib_loader):
   - Parses {"path": "...", "args": [...]}
   - Tries LOADER.dispatch() first
   - Falls back to dispatch_compiled() 
   - Returns result JSON

---

## Key Design Principles

1. **Builtin first, dylib second**: compiled Rust traits for speed, dylib for hot-reloading
2. **C ABI bridge**: trait_call/trait_free allow any language to export traits
3. **JSON serialization**: universal interface between kernel and plugins
4. **Trait path hierarchy**: namespace.name (sys.checksum, www.traits.build, kernel.serve)
5. **Async support**: background = true traits get dispatch_async entry point
6. **Version tracking**: .trait.toml versioning + checksum bumping on source change
7. **Hot-reload ready**: dylib_loader supports reload, all traits via dispatch()

---

## Summary Table

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
