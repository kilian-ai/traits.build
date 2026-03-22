# Detailed crate:: Cross-Reference Map

## KERNEL DEPENDENCIES BY MODULE

### 1. crate::globals:: (kernel/globals/)
**Used in 10+ files — CRITICAL**

Location | Type | Usage
---------|------|------
sys/cli/cli.rs | L172, 202, 244, 299 | `REGISTRY.get()` — trait registry
sys/mcp/mcp.rs | L89, 132 | `REGISTRY.get()` — tool enumeration
sys/openapi/openapi.rs | L9 | `REGISTRY.get()` — API generation
sys/registry/registry.rs | L6 | `REGISTRY.get()` — introspection
sys/snapshot/snapshot.rs | L17 | `REGISTRY.get()` — trait lookup
sys/test_runner/test_runner.rs | Many | `REGISTRY.get()` — discover traits
sys/version/version.rs | Line? | `REGISTRY.get()` — trait count
www/admin/admin.rs | L4 | `CONFIG.get()` — fly app/region
www/admin/fly_api.rs | L8 | `CONFIG.get()` — fly app name
www/traits/build/build.rs | L5 | `REGISTRY.get()` — trait/ns count

### 2. crate::dispatcher:: (kernel/dispatcher/)
**Used in 5+ files — BLOCKS DYLIB CONVERSION**

Location | Type | Usage
---------|------|------
sys/cli/cli.rs | L58 | `bootstrap()` — initialize system
sys/cli/cli.rs | L59 | `dispatcher::compiled::mcp::run_stdio()` — MCP entry
sys/cli/cli.rs | L98 | `dispatcher::cli_formatters::format_cli()` — output formatting
sys/cli/cli.rs | L278 | `dispatcher::CallConfig::default()` — config creation
sys/mcp/mcp.rs | L147 | `dispatcher::compiled::dispatch()` — trait execution
sys/openapi/openapi.rs | L501 | `dispatcher::compiled::dispatch()` — live examples
sys/info/info.rs | L3 | `dispatcher::compiled::registry::info()` — delegation
sys/list/list.rs | L3 | `dispatcher::compiled::registry::list()` — delegation

### 3. crate::types:: (kernel/types/)
**Used in 3 files — TYPE SYSTEM DEPENDENCY**

Location | Type | Usage
---------|------|------
sys/cli/cli.rs | L2 | `use crate::types::TraitValue`
sys/mcp/mcp.rs | L167, 195, 197-213, 219 | `TraitSignature`, `TraitType::*` variants
sys/openapi/openapi.rs | L377, 378, 408 | `TraitType` — schema generation

### 4. crate::config:: (kernel/config/)
**Used in 1 file — CONFIG DEPENDENCY**

Location | Type | Usage
---------|------|------
sys/cli/cli.rs | L1 | `use crate::config::Config` — entire config

### 5. crate::registry:: (kernel/registry/)
**Used in 2 files — REGISTRY ENTRY DEPENDENCY**

Location | Type | Usage
---------|------|------
sys/mcp/mcp.rs | L493 | `TraitEntry` parameter type
sys/openapi/openapi.rs | L493 | `TraitEntry` parameter type

### 6. crate:: functions (kernel/main/)
**Used in cli.rs only**

Location | Type | Usage
---------|------|------
sys/cli/cli.rs | L58, 331 | `crate::bootstrap(config)` — dispatcher init
sys/cli/cli.rs | L68, 70 | `crate::trait_exists(config, path)` — validation

---

## ZERO-DEPENDENCY TRAITS (READY FOR DYLIB NOW)

### sys/checksum/checksum.rs
- Pure utility (canonicalization, hashing)
- No `crate::` references
- Can become dylib immediately

### sys/ps/ps.rs
- File I/O only (reads .pid files, sysctl)
- No `crate::` references
- Can become dylib immediately

### www/admin/fast_deploy/fast_deploy.rs
- Shell script executor only
- No `crate::` references
- Can become dylib immediately

### www/admin/save_config/save_config.rs
- File I/O only (modifies traits.toml)
- No `crate::` references
- Can become dylib immediately

### www/docs/docs.rs
- Markdown rendering only (pulldown_cmark)
- No `crate::` references (only include_str! for markdown)
- Can become dylib immediately

---

## MUST-STAY-IN-BINARY TRAITS

### sys/cli/cli.rs
**IMPOSSIBLE TO DYLIB** — Entry point
- Receives ALL CLI commands
- Initializes dispatcher via `bootstrap()`
- Calls `mcp::run_stdio()` directly
- Uses Config struct directly
- Must be in binary for startup

---

## INTERFACE DESIGN NEEDED

To enable dylib conversion, create a **PluginHost** interface:

```rust
pub trait PluginHost {
    // Registry access
    fn get_trait(&self, path: &str) -> Option<TraitEntryRef>;
    fn list_traits(&self) -> Vec<TraitEntryRef>;
    fn trait_count(&self) -> usize;
    
    // Config access
    fn get_config(&self) -> Option<ConfigRef>;
    
    // Dispatcher (for info/list/mcp/openapi)
    fn dispatch(&self, path: &str, args: &[Value]) -> Option<Value>;
}
```
