# Kernel Direct Dependencies Analysis
## Findings: Which sys.*/www.* traits MUST stay as builtins

### Kernel Code Analysis Summary

#### Files Analyzed:
1. kernel/main/main.rs
2. kernel/serve/serve.rs
3. kernel/dispatcher/dispatcher.rs
4. kernel/config/config.rs
5. kernel/globals/globals.rs
6. kernel/reload/reload.rs
7. kernel/call/call.rs
8. sys/cli/cli.rs
9. sys/list/list.rs
10. sys/registry/registry.rs

---

## HARD KERNEL DEPENDENCIES (MUST be builtins)

### `sys.registry` - CRITICAL
- **Referenced by**: kernel/serve.rs (HTTP server)
- **Called locations**:
  - Line 179: health_check() → calls `dispatcher.call("sys.registry", ["count"])`
  - Line 188: health_check() → calls `dispatcher.call("sys.registry", ["tree"])`
  - Line 208: metrics() → calls `dispatcher.call("sys.registry", ["count"])`
  - Line 232: list_traits() → calls `dispatcher.call("sys.registry", ["tree"])`
  - Line 258: get_trait_info() → calls `dispatcher.call("sys.registry", ["info", trait_path])`
- **Type**: SYNC dispatch (not background)
- **Why**: HTTP endpoints for trait introspection hardcoded into server startup

---

## SOFT KERNEL DEPENDENCIES (Via Dispatcher - can be plugins)

### `sys.cli`
- **Referenced by**: kernel/cli entry point (sys/cli/cli.rs is the ENTRY POINT, not called by kernel)
- **Type**: Entry point, not called by kernel code
- **Can be**: Plugin (if entry point supports it)

### `sys.checksum`
- **Referenced by**: build.rs only (SHA256 helpers included at build time)
- **Type**: Utility, no runtime kernel usage
- **Can be**: Plugin safely

### All other sys.* traits (info, list, mcp, openapi, ps, snapshot, test_runner, version)
- **Referenced by**: Never directly by kernel code
- **Called via**: CLI dispatch or sys.registry mechanism
- **Type**: Utility/introspection
- **Can be**: Plugins safely

### www.* traits (admin, docs, traits/build)
- **Referenced by**: Never directly by kernel code
- **Type**: HTTP endpoints / documentation
- **Can be**: Plugins safely

---

## DISPATCH ARCHITECTURE

The kernel uses a **two-layer dispatch system**:
1. **compiled::dispatch()** → compiled-in Rust trait modules (builtins)
2. **dylib_loader::dispatch()** → plugin .dylib files (loadable extensions)

### How it works:
- Dispatcher.call() → resolves_imports() → dispatch_trait()
- dispatch_trait() → tries dylib loader first, then compiled modules (build.rs generated)
- All dispatch is through the Registry + Dispatcher, NOT direct Rust imports

**Only `sys.registry` is actually CALLED by kernel code** (from kernel/serve.rs endpoints).

---

## CONCLUSION

### MUST STAY AS BUILTINS (Hard dependencies):
- **`sys.registry`** - Only trait directly called by kernel HTTP server for trait metadata

### CAN BE PLUGINS (Soft/no dependencies):
- sys.checksum, sys.cli, sys.info, sys.list, sys.mcp, sys.openapi
- sys.ps, sys.snapshot, sys.test_runner, sys.version
- www.admin, www.docs, www.traits/build

### REASONING:
1. kernel/serve.rs hard-codes calls to sys.registry for /health, /metrics, /traits, /traits/{path} endpoints
2. All other sys/www traits are reached through CLI dispatch or trait calls, never directly by kernel code
3. The dispatcher's layered architecture (dylib → compiled) allows plugins to override builtins
4. Only sys.registry needs to be a builtin because serve.rs directly imports dispatcher and calls it synchronously
