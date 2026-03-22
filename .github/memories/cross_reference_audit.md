# Complete Cross-Reference Audit: sys/*.rs and www/*.rs

## Critical Finding: Kernel/Global Dependencies

**All sys and www traits depend DIRECTLY on kernel modules via `crate::`**, making them unsuitable for dylib plugins as-is.

### Dependency Categories

### 1. GLOBALS (kernel/globals/) — ALL FILES USE
- `crate::globals::REGISTRY` — Global trait registry (OnceLock)
- `crate::globals::CONFIG` — Global config (OnceLock)

### 2. DISPATCHER (kernel/dispatcher/) — USED IN MULTIPLE TRAITS
- `crate::dispatcher::compiled::dispatch()` — Direct trait dispatch
- `crate::dispatcher::compiled::mcp::run_stdio()` — MCP server
- `crate::dispatcher::compiled::registry::*()` — Registry queries
- `crate::dispatcher::cli_formatters::format_cli()` — CLI output formatting

### 3. TYPES (kernel/types/) — USED THROUGHOUT
- `crate::types::TraitValue` — Type wrapper
- `crate::types::TraitSignature` — Signature introspection
- `crate::types::TraitType::*` — All type variants
- `crate::registry::TraitEntry` — Registry entry structure

### 4. BOOTSTRAP (kernel/main/) — USED IN CLI
- `crate::bootstrap()` — Initialize dispatcher and registry
- `crate::trait_exists()` — Check if trait is registered

---

## SYS TRAITS (traits/sys/*/*.rs)

### sys/checksum/checksum.rs
**Direct imports:** NONE (only includes helpers)
**Kernel dependencies:** ZERO
**Status:** READY FOR DYLIB (no kernel references)

### sys/cli/cli.rs
**Direct imports:**
- `use crate::config::Config`
- `use crate::types::TraitValue`

**Direct calls:**
- `crate::bootstrap(&config)?` (line 58)
- `crate::dispatcher::compiled::mcp::run_stdio()` (line 59)
- `crate::trait_exists(&config, &sys_path)` (line 68)
- `crate::trait_exists(&config, &kernel_path)` (line 70)
- `crate::dispatcher::cli_formatters::format_cli()` (line 98)
- `crate::globals::REGISTRY.get()` (lines 172, 202, 244, 299)
- `crate::bootstrap(config)?` (lines 272, 331)
- `crate::dispatcher::CallConfig::default()` (line 278)

**Note:** cli.rs is essentially the CLI entry point and **CANNOT be a dylib** — it initializes the system.

### sys/info/info.rs
**Direct calls:**
- `crate::dispatcher::compiled::registry::info(args)` (line 3)

### sys/list/list.rs
**Direct calls:**
- `crate::dispatcher::compiled::registry::list(args)` (line 3)

### sys/mcp/mcp.rs
**Direct calls:**
```rust
crate::globals::REGISTRY.get() [lines 89, 132]
crate::dispatcher::compiled::dispatch(&trait_path, &args) [line 147]
crate::types::TraitSignature [line 167, 219]
crate::types::TraitType::* [lines 197-213]
crate::registry::TraitEntry [line 493]
crate::dispatcher::compiled::dispatch(&entry.path, &json_args) [line 501]
```

### sys/openapi/openapi.rs
**Direct calls:**
```rust
crate::globals::REGISTRY.get() [line 9]
crate::types::TraitType [lines 378, 408]
crate::registry::TraitEntry [line 493]
crate::dispatcher::compiled::dispatch(&entry.path, &json_args) [line 501]
```

### sys/ps/ps.rs
**Direct calls:** NONE (file I/O only)
**Kernel dependencies:** ZERO
**Status:** READY FOR DYLIB

### sys/registry/registry.rs
**Direct calls:**
```rust
crate::globals::REGISTRY.get() [line 6]
```

### sys/snapshot/snapshot.rs
**Direct calls:**
```rust
crate::globals::REGISTRY.get() [line 17]
```

### sys/test_runner/test_runner.rs
**Direct calls:**
```rust
crate::globals::REGISTRY.get() [multiple calls]
```

### sys/version/version.rs
**Direct calls:**
```rust
crate::globals::REGISTRY.get() [one call in build_system_version()]
```

---

## WWW TRAITS (traits/www/*/*.rs)

### www/admin/admin.rs
**Direct calls:**
```rust
crate::globals::CONFIG.get() [line 4]
```

### www/admin/fly_api.rs (helper, not a trait)
**Direct calls:**
```rust
crate::globals::CONFIG.get() [line 8]
```

### www/admin/deploy/deploy.rs
Includes fly_api.rs which uses `crate::globals::CONFIG`

### www/admin/destroy/destroy.rs
Includes fly_api.rs which uses `crate::globals::CONFIG`

### www/admin/fast_deploy/fast_deploy.rs
**Direct calls:** NONE (shell script execution only)
**Kernel dependencies:** ZERO
**Status:** READY FOR DYLIB

### www/admin/scale/scale.rs
Includes fly_api.rs which uses `crate::globals::CONFIG`

### www/admin/save_config/save_config.rs
**Direct calls:** NONE (file I/O only)
**Kernel dependencies:** ZERO
**Status:** READY FOR DYLIB

### www/docs/docs.rs
**Direct calls:** NONE (markdown rendering only)
**Kernel dependencies:** ZERO
**Status:** READY FOR DYLIB

### www/traits/build/build.rs (the landing page)
**Direct calls:**
```rust
crate::globals::REGISTRY.get() [line 5]
```

---

## SUMMARY TABLE

| File | Pure? | Ready? | Issue |
|------|-------|--------|-------|
| sys/checksum/checksum.rs | Yes | YES | None |
| sys/cli/cli.rs | No | NO | Entry point, uses bootstrap/dispatcher/config/types |
| sys/info/info.rs | No | NO | Calls dispatcher::registry |
| sys/list/list.rs | No | NO | Calls dispatcher::registry |
| sys/mcp/mcp.rs | No | NO | Uses REGISTRY, dispatcher::dispatch, types |
| sys/openapi/openapi.rs | No | NO | Uses REGISTRY, dispatcher::dispatch, types |
| sys/ps/ps.rs | Yes | YES | None |
| sys/registry/registry.rs | No | NO | Uses REGISTRY |
| sys/snapshot/snapshot.rs | No | PARTIAL | Uses REGISTRY read-only |
| sys/test_runner/test_runner.rs | No | NO | Uses REGISTRY |
| sys/version/version.rs | No | PARTIAL | Uses REGISTRY (1 call only) |
| www/admin/admin.rs | No | NO | Uses CONFIG |
| www/admin/fly_api.rs | No | NO | Uses CONFIG |
| www/admin/deploy/deploy.rs | No | NO | Uses CONFIG (indirect) |
| www/admin/destroy/destroy.rs | No | NO | Uses CONFIG (indirect) |
| www/admin/fast_deploy/fast_deploy.rs | Yes | YES | None |
| www/admin/scale/scale.rs | No | NO | Uses CONFIG (indirect) |
| www/admin/save_config/save_config.rs | Yes | YES | None |
| www/docs/docs.rs | Yes | YES | None |
| www/traits/build/build.rs | No | PARTIAL | Uses REGISTRY (read-only, counts only) |
