# Line-by-Line crate:: Reference Table

## FILE: traits/sys/cli/cli.rs
**Total crate:: refs: 11**
**Kernel modules used: config, types, bootstrap, dispatcher, globals**

```
Line  Code                                          Module          Impact
────  ────────────────────────────────────────────  ──────────────  ──────────────
1     use crate::config::Config                     config          REQUIRED
2     use crate::types::TraitValue                  types           REQUIRED
58    let _dispatcher = crate::bootstrap(&config)   main            CRITICAL
59    crate::dispatcher::compiled::mcp::run_stdio() dispatcher      CRITICAL
68    if crate::trait_exists(&config, &sys_path)   main            CRITICAL
70    } else if crate::trait_exists(&config, &...) main            CRITICAL
98    if let Some(formatted) = crate::dispatcher...  dispatcher      CRITICAL
172   if let Some(reg) = crate::globals::REGISTRY... globals         CRITICAL
202   if let Some(reg) = crate::globals::REGISTRY... globals         CRITICAL
244   if let Some(reg) = crate::globals::REGISTRY... globals         CRITICAL
272   let dispatcher = crate::bootstrap(config)?     main            CRITICAL
278   match dispatcher.call(..., &crate::dispatcher...) dispatcher    CRITICAL
299   let reg = match crate::globals::REGISTRY.get() globals         CRITICAL
331   let dispatcher = crate::bootstrap(config)?     main            CRITICAL
```

**Verdict:** NOT A DYLIB — Entry point initialization

---

## FILE: traits/sys/checksum/checksum.rs
**Total crate:: refs: 0**
**Kernel modules used: NONE**

**Verdict:** READY FOR DYLIB NOW

---

## FILE: traits/sys/info/info.rs
**Total crate:: refs: 1**
**Kernel modules used: dispatcher**

```
Line  Code                                          Module      Impact
────  ────────────────────────────────────────────  ──────────  ────────
3     crate::dispatcher::compiled::registry::info   dispatcher  DIRECT CALL
```

**Note:** Direct delegation to compiled dispatcher function
**Verdict:** BLOCKED — Needs dispatcher interface

---

## FILE: traits/sys/list/list.rs
**Total crate:: refs: 1**
**Kernel modules used: dispatcher**

```
Line  Code                                          Module      Impact
────  ────────────────────────────────────────────  ──────────  ────────
3     crate::dispatcher::compiled::registry::list   dispatcher  DIRECT CALL
```

**Note:** Direct delegation to compiled dispatcher function
**Verdict:** BLOCKED — Needs dispatcher interface

---

## FILE: traits/sys/mcp/mcp.rs
**Total crate:: refs: 16**
**Kernel modules used: globals, dispatcher, types, registry**

```
Line  Code                                          Module      Impact
────  ────────────────────────────────────────────  ──────────  ────────
89    let registry = match crate::globals::REGISTRY globals     GET REGISTRY
132   let registry = match crate::globals::REGISTRY globals     GET REGISTRY
147   match crate::dispatcher::compiled::dispatch   dispatcher  DISPATCH TRAIT
167   fn build_input_schema(sig: &crate::types::... types       SIGNATURE PARAM
195   fn trait_type_to_json_schema(tt: &crate:...  types       TYPE PARAM
197   crate::types::TraitType::Int => json!(...)     types       TYPE MATCH
198   crate::types::TraitType::Float => json!(...)   types       TYPE MATCH
199   crate::types::TraitType::String => json!(...)  types       TYPE MATCH
200   crate::types::TraitType::Bool => json!(...)    types       TYPE MATCH
201   crate::types::TraitType::Bytes => json!(...)   types       TYPE MATCH
202   crate::types::TraitType::List(inner) => json.. types       TYPE MATCH
206   crate::types::TraitType::Map(_k, v) =>...     types       TYPE MATCH
210   crate::types::TraitType::Optional(inner)...  types       TYPE MATCH
211   crate::types::TraitType::Any => json!(...)    types       TYPE MATCH
212   crate::types::TraitType::Handle => json!(...)  types       TYPE MATCH
213   crate::types::TraitType::Null => json!(...)    types       TYPE MATCH
219   sig: &crate::types::TraitSignature            types       SIGNATURE PARAM
493   fn generate_live_examples(all: &[crate::r...  registry    TRAIT ENTRY PARAM
501   if let Some(result) = crate::dispatcher::...  dispatcher  DISPATCH TRAIT
```

**Verdict:** BLOCKED — Uses globals, dispatcher, types deeply

---

## FILE: traits/sys/openapi/openapi.rs
**Total crate:: refs: 5 + 20+ pattern matches**
**Kernel modules used: globals, types, registry, dispatcher**

```
Line  Code                                          Module      Impact
────  ────────────────────────────────────────────  ──────────  ────────
9     let reg = match crate::globals::REGISTRY     globals     GET REGISTRY
377   fn trait_type_to_schema(t: &crate::types::...types       TYPE PARAM
378   use crate::types::TraitType                    types       TYPE IMPORT
407   fn example_value(t: &crate::types::TraitType) types       TYPE PARAM
408   use crate::types::TraitType                    types       TYPE IMPORT
493   fn generate_live_examples(all: &[crate::r...  registry    ENTRY PARAM
501   if let Some(result) = crate::dispatcher::...  dispatcher  DISPATCH
```

**Plus ~40 type match patterns:** `crate::types::TraitType::Int`, `::Float`, etc.

**Verdict:** BLOCKED — Uses globals, types, registry, dispatcher

---

## FILE: traits/sys/ps/ps.rs
**Total crate:: refs: 0**
**Kernel modules used: NONE**

**Verdict:** READY FOR DYLIB NOW

---

## FILE: traits/sys/registry/registry.rs
**Total crate:: refs: 1**
**Kernel modules used: globals**

```
Line  Code                                          Module      Impact
────  ────────────────────────────────────────────  ──────────  ────────
6     let reg = match crate::globals::REGISTRY     globals     GET REGISTRY
```

**Full function:** Handles registry introspection (list, info, tree, namespaces, count, get, search)

**Verdict:** PARTIAL — Read-only registry access, could pass as interface

---

## FILE: traits/sys/snapshot/snapshot.rs
**Total crate:: refs: 1**
**Kernel modules used: globals**

```
Line  Code                                          Module      Impact
────  ────────────────────────────────────────────  ──────────  ────────
17    let registry = match crate::globals::REGISTRY globals     GET REGISTRY/LOOKUP
```

**Note:** Only used to look up trait by path to find its toml_path

**Verdict:** PARTIAL — Needs trait lookup interface

---

## FILE: traits/sys/test_runner/test_runner.rs
**Total crate:: refs: Multiple (exact count in grep)**
**Kernel modules used: globals**

**Note:** Discovers all traits matching pattern from registry

**Verdict:** PARTIAL — Could pass `Vec<TraitEntry>` as parameter

---

## FILE: traits/sys/version/version.rs
**Total crate:: refs: 1**
**Kernel modules used: globals**

```rust
fn build_system_version() -> Value {
    let trait_count = crate::globals::REGISTRY
        .get()
        .map(|r| r.len())
        .unwrap_or(0);
```

**Verdict:** MINIMAL — Could pass `trait_count` as parameter

---

## FILE: traits/www/admin/admin.rs
**Total crate:: refs: 1**
**Kernel modules used: globals**

```
Line  Code                                          Module      Impact
────  ────────────────────────────────────────────  ──────────  ────────
4     let (fly_app, fly_region) = match crate::... globals     GET CONFIG
```

**Verdict:** PARTIAL — Could pass `(fly_app, fly_region)` as parameters

---

## FILE: traits/www/admin/fly_api.rs (helper module)
**Total crate:: refs: 1**
**Kernel modules used: globals**

```
Line  Code                                          Module      Impact
────  ────────────────────────────────────────────  ──────────  ────────
8     crate::globals::CONFIG.get()                  globals     GET CONFIG
```

**Verdict:** PARTIAL — Could pass fly_app as parameter

---

## FILE: traits/www/admin/deploy/deploy.rs
**Total crate:: refs: 0 (direct)**
**Kernel modules used: globals (via fly_api include)**

**Verdict:** PARTIAL — Inherits CONFIG dependency from fly_api

---

## FILE: traits/www/admin/destroy/destroy.rs
**Total crate:: refs: 0 (direct)**
**Kernel modules used: globals (via fly_api include)**

**Verdict:** PARTIAL — Inherits CONFIG dependency from fly_api

---

## FILE: traits/www/admin/fast_deploy/fast_deploy.rs
**Total crate:: refs: 0**
**Kernel modules used: NONE**

**Verdict:** READY FOR DYLIB NOW

---

## FILE: traits/www/admin/scale/scale.rs
**Total crate:: refs: 0 (direct)**
**Kernel modules used: globals (via fly_api include)**

**Verdict:** PARTIAL — Inherits CONFIG dependency from fly_api

---

## FILE: traits/www/admin/save_config/save_config.rs
**Total crate:: refs: 0**
**Kernel modules used: NONE**

**Verdict:** READY FOR DYLIB NOW

---

## FILE: traits/www/docs/docs.rs
**Total crate:: refs: 0**
**Kernel modules used: NONE**

**Verdict:** READY FOR DYLIB NOW

---

## FILE: traits/www/traits/build/build.rs (landing page)
**Total crate:: refs: 1**
**Kernel modules used: globals**

```
Line  Code                                          Module      Impact
────  ────────────────────────────────────────────  ──────────  ────────
5     let (trait_count, ns_count) = match crate::... globals     GET REGISTRY
```

**Verdict:** MINIMAL — Could pass `(trait_count, ns_count)` as parameters

---

## SUMMARY BY CATEGORY

### ZERO DEPENDENCIES (5 traits, ready for dylib now)
1. sys/checksum/checksum.rs
2. sys/ps/ps.rs
3. www/admin/fast_deploy/fast_deploy.rs
4. www/admin/save_config/save_config.rs
5. www/docs/docs.rs

### MINIMAL DEPENDENCIES (4 traits, can be converted with parameters)
1. sys/version/version.rs — needs `trait_count: usize`
2. sys/snapshot/snapshot.rs — needs trait lookup interface
3. www/admin/admin.rs — needs `config: (fly_app, fly_region)`
4. www/traits/build/build.rs — needs `(trait_count, ns_count)`

### HEAVY DEPENDENCIES (14+ traits, cannot convert)
1. sys/cli/cli.rs — Entry point
2. sys/info/info.rs — dispatcher delegation
3. sys/list/list.rs — dispatcher delegation
4. sys/mcp/mcp.rs — full dispatcher integration
5. sys/openapi/openapi.rs — full dispatcher integration
6. sys/registry/registry.rs — full registry access
7. sys/test_runner/test_runner.rs — full registry discovery
8. www/admin/deploy/deploy.rs — CONFIG + dispatcher
9. www/admin/destroy/destroy.rs — CONFIG
10. www/admin/scale/scale.rs — CONFIG
11. (others in fly_api chain)
