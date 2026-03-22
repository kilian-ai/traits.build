---
sidebar_position: 3
---

# Architecture

traits.build is structured around a single principle: **everything is a trait.** The kernel bootstraps by loading trait definitions, wiring interfaces, and then dispatching to traits for all functionality — including serving HTTP.

## Directory layout

```
traits.build/
├── build.rs              # Trait discovery + code generation
├── src/main.rs           # Thin entry point
├── traits/
│   ├── kernel/           # 11 kernel traits (core runtime)
│   │   ├── call/         # Inter-trait dispatch
│   │   ├── config/       # Configuration loading
│   │   ├── dispatcher/   # Path resolution + arg validation
│   │   ├── dylib_loader/ # Plugin loading (cdylib)
│   │   ├── globals/      # Shared statics (REGISTRY, CONFIG)
│   │   ├── main/         # Bootstrap + introspection
│   │   ├── plugin_api/   # C ABI for plugins
│   │   ├── registry/     # Trait storage + interface resolution
│   │   ├── reload/       # Hot-reload from disk
│   │   ├── serve/        # HTTP server (actix-web)
│   │   └── types/        # TraitValue, TraitType
│   ├── sys/              # 11 system traits
│   │   ├── checksum/     # SHA-256 hashing
│   │   ├── cli/          # CLI dispatch
│   │   ├── info/         # Trait metadata
│   │   ├── list/         # List traits
│   │   ├── openapi/      # OpenAPI spec generation
│   │   ├── ps/           # Process listing
│   │   ├── registry/     # Registry read API
│   │   ├── snapshot/     # Version snapshots
│   │   ├── test_runner/  # Test framework
│   │   └── version/      # Version strings
│   └── www/              # 6 web traits
│       ├── admin/        # Admin dashboard + deploy/scale/destroy
│       ├── docs/api/     # API docs (Redoc)
│       └── traits/build/ # Landing page
└── traits.toml           # Runtime configuration
```

## Build system

The `build.rs` script runs at compile time and performs:

1. **Trait discovery** — recursively scans `traits/` for `.trait.toml` files
2. **Module generation** — creates `compiled_traits.rs` with dispatch tables
3. **Registry embedding** — generates `builtin_traits.rs` with `include_str!()` for all TOML
4. **Kernel modules** — generates `kernel_modules.rs` for crate-level mod declarations
5. **Version computation** — `vYYMMDD` format, with `.HHMMSS` suffix for same-day builds
6. **Checksum validation** — SHA-256 of source files, bumps version on change

No manual registration is needed. Drop a trait in `traits/` and rebuild.

## Bootstrap sequence

```
main() →
  kernel.main: load config, init registry, load builtins →
    kernel.config: parse traits.toml + env vars →
    kernel.registry: populate DashMap from embedded TOML →
    kernel.dylib_loader: scan for .dylib/.so plugins →
    kernel.serve: start actix-web, resolve page routes →
      → serve_page: resolve URL path via keyed bindings
      → call_trait: POST /traits/{path} dispatch
```

## Dispatch flow

When a trait is called (via CLI or REST):

```
Request: "sys.checksum" with args ["hash", "hello"]
    ↓
Dispatcher: lookup in registry
    ↓
Arg validation: check types, count, coerce if needed
    ↓
Compiled dispatch: match trait_path → module::entry(args)
    ↓
Return: TraitValue → JSON response
```

The dispatcher supports:
- **Compiled dispatch** — traits compiled into the binary
- **Plugin dispatch** — `.dylib` plugins loaded at runtime (checked first)
- **Background dispatch** — async traits (like `kernel.serve`)
- **Streaming dispatch** — SSE via `?stream=1`

## Namespaces

| Namespace | Count | Purpose |
|-----------|------:|---------|
| `kernel`  | 11    | Core runtime — registry, config, dispatch, HTTP |
| `sys`     | 11    | System utilities — checksum, testing, versioning |
| `www`     | 6     | Web interface — landing page, admin, API docs |
