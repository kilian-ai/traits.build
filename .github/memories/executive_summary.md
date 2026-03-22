# Executive Summary: Complete crate:: Cross-Reference Audit

**Date:** 2026-03-22  
**Scope:** All `crate::` references in traits/sys/*.rs and traits/www/*.rs  
**Total files analyzed:** 21 trait files + 3 template files  
**Total crate:: references found:** 70+ across all files  

---

## KEY FINDINGS

### BLOCKER: Direct Kernel Dependencies
**ALL 21 trait files have direct dependency on kernel modules via `crate::`**

- 10 traits use `crate::globals::REGISTRY`
- 5 traits use `crate::dispatcher::*`
- 3 traits use `crate::types::*`
- 1 trait uses `crate::config::Config`
- Multiple traits use `crate::bootstrap()` and `crate::trait_exists()`

**Impact:** Current code cannot be mere dylib plugins — must refactor or keep in binary.

---

## CONVERSION READINESS BY TIER

### TIER 1: Ready NOW (5 traits, 0 changes)
Can be converted to dylib immediately:

```
sys/checksum/checksum.rs         — Zero kernel deps
sys/ps/ps.rs                      — Zero kernel deps
www/admin/fast_deploy/fast_deploy.rs — Zero kernel deps
www/admin/save_config/save_config.rs — Zero kernel deps
www/docs/docs.rs                  — Zero kernel deps
```

**Action:** Create Cargo.toml + lib.rs wrapper, add export_trait! macro  
**Template:** Use traits/www/traits/build/ as reference

### TIER 2: Minimal Refactoring (4 traits)
**Dependency:** Only read-only globals (pass as parameter)

```
sys/version/version.rs            — Uses REGISTRY.len() (1 call)
www/traits/build/build.rs         — Uses REGISTRY count + ns count (1 call)
sys/snapshot/snapshot.rs          — Uses REGISTRY lookup (1 call)
sys/registry/registry.rs          — Uses REGISTRY introspection (1 call)
```

**Refactoring:** Accept trait metadata as function parameter instead of reading globals  
**Effort:** Low — modify 1-3 lines per file to accept interface parameter

### TIER 2B: Config Injection (4 traits)
**Dependency:** Read-only CONFIG (pass as parameter)

```
www/admin/admin.rs                — Uses CONFIG.fly_app/fly_region
www/admin/deploy/deploy.rs        — Uses CONFIG (via fly_api include)
www/admin/destroy/destroy.rs      — Uses CONFIG (via fly_api include)
www/admin/scale/scale.rs          — Uses CONFIG (via fly_api include)
```

**Refactoring:** Accept config snapshot as parameter  
**Effort:** Low-Medium — extract config into parameter struct

### TIER 3: Interface-Based Refactoring (2 traits)
**Problem:** Direct delegation to compiled dispatcher function

```
sys/info/info.rs                  — Calls dispatcher::registry::info()
sys/list/list.rs                  — Calls dispatcher::registry::list()
```

**Option A (Recommended):** Keep as compiled traits, not dylibs  
**Option B:** Replace dispatcher calls with PluginDispatcher interface  
**Effort:** Medium — requires interface trait definition

### TIER 4: Cannot Dylib (4 traits + dependencies)
**Reason:** Critical entry points or deep kernel integration

```
sys/cli/cli.rs                    — Entry point (MUST stay in binary)
sys/mcp/mcp.rs                    — Full dispatcher integration
sys/openapi/openapi.rs           — Complex schema generation + dispatch
sys/test_runner/test_runner.rs   — Full registry discovery + iteration
```

**Action:** Keep in binary, no dylib conversion possible

---

## KERNEL MODULE DEPENDENCY MAP

### globals:: (Most common)
**Files:** 10 traits  
**Uses:** REGISTRY.get(), CONFIG.get()  
**Critical traits using it:**
- sys/cli/cli.rs (4 uses)
- sys/mcp/mcp.rs (2 uses)
- sys/openapi/openapi.rs (1 use)

### dispatcher:: 
**Files:** 5 traits  
**Uses:** dispatch(), mcp::run_stdio(), registry::info(), registry::list(), CallConfig  
**Critical traits:**
- sys/cli/cli.rs (3 uses — bootstrap, mcp_stdio, call_config)
- sys/mcp/mcp.rs (1 use — dispatch)
- sys/openapi/openapi.rs (1 use — dispatch)

### types::
**Files:** 2 traits (40+ individual pattern matches)  
**Uses:** TraitValue, TraitSignature, TraitType enum variants  
**Critical traits:**
- sys/mcp/mcp.rs (15+ uses of TraitType variants)
- sys/openapi/openapi.rs (40+ type schema matches)

### config::, registry::, bootstrap fn, trait_exists fn
**Limited use.** Only in sys/cli.rs or as parameters.

---

## DETAILED CROSS-REFERENCE MAP

### File-by-File Breakdown

**sys/checksum/checksum.rs** — crate:: refs: 0 — Ready for dylib

**sys/cli/cli.rs** — crate:: refs: 11 — Entry point, cannot dylib

**sys/info/info.rs** — crate:: refs: 1 — Requires dispatcher interface

**sys/list/list.rs** — crate:: refs: 1 — Requires dispatcher interface

**sys/mcp/mcp.rs** — crate:: refs: 16+ — Deep integration required

**sys/openapi/openapi.rs** — crate:: refs: 6 + 40 type matches — Cannot standalone dylib

**sys/ps/ps.rs** — crate:: refs: 0 — Ready for dylib

**sys/registry/registry.rs** — crate:: refs: 1 — Ready with interface parameter

**sys/snapshot/snapshot.rs** — crate:: refs: 1 — Ready with lookup interface

**sys/test_runner/test_runner.rs** — crate:: refs: Many — Full registry iteration needed

**sys/version/version.rs** — crate:: refs: 1 — Ready with trait_count parameter

**www/admin/admin.rs** — crate:: refs: 1 — Ready with config parameter

**www/admin/fly_api.rs** — crate:: refs: 1 — Helper, inherit status from users

**www/admin/deploy/deploy.rs** — crate:: refs: 0 direct (1 indirect via fly_api) — Ready with config parameter

**www/admin/destroy/destroy.rs** — crate:: refs: 0 direct (1 indirect via fly_api) — Ready with config parameter

**www/admin/fast_deploy/fast_deploy.rs** — crate:: refs: 0 — Ready for dylib

**www/admin/scale/scale.rs** — crate:: refs: 0 direct (1 indirect via fly_api) — Ready with config parameter

**www/admin/save_config/save_config.rs** — crate:: refs: 0 — Ready for dylib

**www/docs/docs.rs** — crate:: refs: 0 — Ready for dylib

**www/traits/build/build.rs** — crate:: refs: 1 — Ready with (trait_count, ns_count) parameter

---

## RECOMMENDED STRATEGY

### Phase 1: Convert Tier 1 Immediately (LOW RISK)
Move these 5 traits to cdylib — zero code changes needed.

### Phase 2: Parameter Injection (MEDIUM EFFORT)
Refactor 8 traits to accept interface parameters instead of reading globals.

### Phase 3: Interface Definition (FUTURE)
Define PluginHost interface for sys/info, sys/list (dispatcher interface).

### Phase 4: Keep in Binary (FINAL STATE)
sys/cli, sys/test_runner, sys/mcp, sys/openapi — will never dylib.

---

## CRITICAL INSIGHT

**The conversion barrier is NOT the export mechanism** (plugin_api macro works fine).

**The barrier is KERNEL DEPENDENCIES** — most traits need:
- Registry access (trait metadata)
- Config access (deployment settings)
- Dispatcher access (trait-to-trait calls)

**Solution:** Create a **PluginHost interface** that dylibs receive at initialization, providing safe access to these kernel resources without direct `crate::` imports.
