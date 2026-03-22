# Complete crate:: Grep Results & Dependency Map

## GREP SEARCH RESULTS (from initial scan)

### All sys/* files combined (showing selected matches):
```
traits/sys/mcp/mcp.rs:89
    let registry = match crate::globals::REGISTRY.get() {

traits/sys/mcp/mcp.rs:132
    let registry = match crate::globals::REGISTRY.get() {

traits/sys/mcp/mcp.rs:147
    match crate::dispatcher::compiled::dispatch(&trait_path, &args) {

traits/sys/mcp/mcp.rs:167
fn build_input_schema(sig: &crate::types::TraitSignature) -> Value {

traits/sys/mcp/mcp.rs:195
fn trait_type_to_json_schema(tt: &crate::types::TraitType) -> Value {

traits/sys/mcp/mcp.rs:197-213
        crate::types::TraitType::Int => json!({"type": "integer"}),
        crate::types::TraitType::Float => json!({"type": "number"}),
        crate::types::TraitType::String => json!({"type": "string"}),
        crate::types::TraitType::Bool => json!({"type": "boolean"}),
        crate::types::TraitType::Bytes => json!({"type": "string"}),
        crate::types::TraitType::List(inner) => json!({...}),
        crate::types::TraitType::Map(_k, v) => json!({...}),
        crate::types::TraitType::Optional(inner) => trait_type_to_json_schema(inner),
        crate::types::TraitType::Any => json!({"type": "string"}),
        crate::types::TraitType::Handle => json!({"type": "string"}),
        crate::types::TraitType::Null => json!({"type": "string"}),

traits/sys/openapi/openapi.rs:9
    let reg = match crate::globals::REGISTRY.get() {

traits/sys/openapi/openapi.rs:377
fn trait_type_to_schema(t: &crate::types::TraitType) -> Value {

traits/sys/openapi/openapi.rs:378
    use crate::types::TraitType;

traits/sys/openapi/openapi.rs:407
fn example_value(t: &crate::types::TraitType) -> Value {

traits/sys/openapi/openapi.rs:408
    use crate::types::TraitType;

traits/sys/openapi/openapi.rs:493
fn generate_live_examples(all: &[crate::registry::TraitEntry]) -> std::collections::HashMap<&str, LiveExample> {

traits/sys/openapi/openapi.rs:501
            if let Some(result) = crate::dispatcher::compiled::dispatch(&entry.path, &json_args) {

traits/sys/cli/cli.rs:1-2
use crate::config::Config;
use crate::types::TraitValue;

traits/sys/cli/cli.rs:58
                let _dispatcher = crate::bootstrap(&config)?;

traits/sys/cli/cli.rs:59
                crate::dispatcher::compiled::mcp::run_stdio();

traits/sys/cli/cli.rs:68
                let trait_path = if crate::trait_exists(&config, &sys_path) {

traits/sys/cli/cli.rs:70
                } else if crate::trait_exists(&config, &kernel_path) {

traits/sys/cli/cli.rs:98
        if let Some(formatted) = crate::dispatcher::cli_formatters::format_cli(trait_path, &json_val) {

traits/sys/cli/cli.rs:172
    if let Some(reg) = crate::globals::REGISTRY.get() {

traits/sys/cli/cli.rs:202
    if let Some(reg) = crate::globals::REGISTRY.get() {

traits/sys/cli/cli.rs:244
    if let Some(reg) = crate::globals::REGISTRY.get() {

traits/sys/cli/cli.rs:272
    let dispatcher = crate::bootstrap(config)?;

traits/sys/cli/cli.rs:278
    match dispatcher.call(trait_path, trait_args, &crate::dispatcher::CallConfig::default()).await {

traits/sys/cli/cli.rs:299
    let reg = match crate::globals::REGISTRY.get() {

traits/sys/cli/cli.rs:331
    let dispatcher = crate::bootstrap(config)?;

traits/www/traits/build/build.rs:5
    let (trait_count, ns_count) = match crate::globals::REGISTRY.get() {

traits/www/admin/fly_api.rs:8
    crate::globals::CONFIG.get()

traits/www/admin/admin.rs:4
    let (fly_app, fly_region) = match crate::globals::CONFIG.get() {
```

---

## KERNEL MODULE DEPENDENCY TREE

```
kernel/ (traits/kernel/)
├── globals/
│   ├── REGISTRY (OnceLock)
│   └── CONFIG (OnceLock)
│
├── dispatcher/
│   ├── compiled/
│   │   ├── dispatch() — main entry point
│   │   ├── mcp::run_stdio() — MCP server
│   │   └── registry::info(), ::list()
│   └── cli_formatters::format_cli()
│
├── types/
│   ├── TraitValue
│   ├── TraitSignature
│   └── TraitType (enum)
│
├── registry/
│   └── TraitEntry
│
├── config/
│   └── Config struct
│
├── main/
│   ├── bootstrap() fn
│   └── trait_exists() fn
│
├── plugin_api/
│   └── export_trait! macro, plugin_api() trait
│
└── dylib_loader/ (inside plugin_api)
    └── LOADER (OnceLock)
```

---

## WHICH TRAITS USE WHICH KERNEL MODULES

### globals::REGISTRY users (10 traits):
```
sys/cli/cli.rs ......................... 4 uses
sys/mcp/mcp.rs ......................... 2 uses
sys/openapi/openapi.rs ................ 1 use
sys/registry/registry.rs .............. 1 use
sys/snapshot/snapshot.rs .............. 1 use
sys/test_runner/test_runner.rs ........ many uses
sys/version/version.rs ................ 1 use
www/admin/admin.rs .................... 0 (uses CONFIG instead)
www/traits/build/build.rs ............. 1 use
```

### globals::CONFIG users (5 traits):
```
www/admin/admin.rs .................... 1 use
www/admin/fly_api.rs (helper) ......... 1 use
www/admin/deploy/deploy.rs (includes fly_api)
www/admin/destroy/destroy.rs (includes fly_api)
www/admin/scale/scale.rs (includes fly_api)
```

### dispatcher:: users (5 traits):
```
sys/cli/cli.rs ........................ 3 uses (bootstrap, mcp, call_config)
sys/info/info.rs ...................... 1 use (registry::info)
sys/list/list.rs ...................... 1 use (registry::list)
sys/mcp/mcp.rs ........................ 1 use (dispatch)
sys/openapi/openapi.rs ............... 1 use (dispatch for examples)
```

### types:: users (2 traits):
```
sys/cli/cli.rs ........................ 1 use (TraitValue)
sys/mcp/mcp.rs ........................ 1 import + 17+ uses (TraitType, TraitSignature)
sys/openapi/openapi.rs ............... 1 use (TraitType) + 40+ pattern matches
```

### registry:: users (2 traits):
```
sys/mcp/mcp.rs ........................ 1 use (TraitEntry parameter)
sys/openapi/openapi.rs ............... 1 use (TraitEntry parameter)
```

### config:: users (1 trait):
```
sys/cli/cli.rs ........................ 1 import (Config struct)
```

### bootstrap/trait_exists users (1 trait):
```
sys/cli/cli.rs ........................ 4 uses (bootstrap x2, trait_exists x2)
```

---

## DEPENDENCY CARDINALITY MATRIX

```
Trait File                          globals  dispatcher  types  registry  config  bootstrap  ready?
─────────────────────────────────── ──────── ──────────── ────── ──────── ────── ────────── ───────
sys/checksum/checksum.rs                0        0          0       0        0         0        ✅
sys/cli/cli.rs                          4        3          1       0        1         2        ❌
sys/info/info.rs                        0        1          0       0        0         0        ❌
sys/list/list.rs                        0        1          0       0        0         0        ❌
sys/mcp/mcp.rs                          2        1         17       1        0         0        ❌
sys/openapi/openapi.rs                  1        1         40       1        0         0        ❌
sys/ps/ps.rs                            0        0          0       0        0         0        ✅
sys/registry/registry.rs                1        0          0       0        0         0        ⚠️
sys/snapshot/snapshot.rs                1        0          0       0        0         0        ⚠️
sys/test_runner/test_runner.rs          ?        0          0       0        0         0        ⚠️
sys/version/version.rs                  1        0          0       0        0         0        ⚠️
www/admin/admin.rs                      0        0          0       0        1         0        ⚠️
www/admin/deploy/deploy.rs              0        0          0       0        1(indirect) 0      ⚠️
www/admin/destroy/destroy.rs            0        0          0       0        1(indirect) 0      ⚠️
www/admin/fast_deploy/fast_deploy.rs    0        0          0       0        0         0        ✅
www/admin/save_config/save_config.rs    0        0          0       0        0         0        ✅
www/admin/scale/scale.rs                0        0          0       0        1(indirect) 0      ⚠️
www/docs/docs.rs                        0        0          0       0        0         0        ✅
www/traits/build/build.rs               1        0          0       0        0         0        ⚠️
```

---

## CONVERSION PRIORITY RANKING

### Tier 1: IMMEDIATE (No changes needed)
```
1. sys/checksum/checksum.rs
2. sys/ps/ps.rs
3. www/admin/fast_deploy/fast_deploy.rs
4. www/admin/save_config/save_config.rs
5. www/docs/docs.rs
```

### Tier 2: PARAMETER INJECTION (Minimal refactoring)
```
6. sys/version/version.rs — inject trait_count
7. www/traits/build/build.rs — inject (trait_count, ns_count)
8. sys/snapshot/snapshot.rs — inject trait lookup fn
9. sys/registry/registry.rs — inject registry data
10. www/admin/admin.rs — inject (fly_app, fly_region)
11. www/admin/deploy/deploy.rs — inject fly_app via parameter
12. www/admin/destroy/destroy.rs — inject fly_app via parameter
13. www/admin/scale/scale.rs — inject fly_app via parameter
```

### Tier 3: INTERFACE-BASED (Moderate refactoring)
```
14. sys/info/info.rs — replace dispatcher call with interface
15. sys/list/list.rs — replace dispatcher call with interface
```

### Tier 4: KEEP IN BINARY (No conversion)
```
16. sys/cli/cli.rs — Entry point
17. sys/mcp/mcp.rs — Deep integration with dispatcher
18. sys/openapi/openapi.rs — Complex dispatch + schema generation
19. sys/test_runner/test_runner.rs — Full registry discovery
```
