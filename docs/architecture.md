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
│   ├── kernel/           # 13 kernel traits (3-layer architecture)
│   │   ├── logic/        # [L0] Shared library: types, registry model, platform
│   │   ├── wasm/         # [L0] WASM browser kernel (wasm-pack target)
│   │   ├── call/         # [L1] Inter-trait dispatch
│   │   ├── cli/          # [L1] Portable CLI processor
│   │   ├── types/        # [L1] TraitValue, TraitType
│   │   ├── config/       # [L2] Configuration loading
│   │   ├── dispatcher/   # [L2] Path resolution + arg validation
│   │   ├── dylib_loader/ # [L2] Plugin loading (cdylib)
│   │   ├── globals/      # [L2] Shared statics (REGISTRY, CONFIG)
│   │   ├── main/         # [L2] Bootstrap + introspection
│   │   ├── plugin_api/   # [L2] C ABI for plugins
│   │   ├── registry/     # [L2] Trait storage + interface resolution
│   │   ├── reload/       # [L2] Hot-reload from disk
│   │   └── serve/        # [L2] HTTP server (actix-web)
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

## Linux-WASM Guest Artifact Notes

- Guest user binaries (such as `git.bin` in `initramfs.cpio.gz`) are sensitive to optimizer toolchain versions.
- Prefer upstream Binaryen `wasm-opt` builds (v129 or newer). Older distro-packaged versions can produce artifacts that fail to execute in the linux-wasm guest with `Exec format error`.
- For NOMMU guest load issues, treat both binary size and optimizer version as independent variables during debugging.

### Linux Terminal UX Notes

- The Linux SPA terminal persists command history in `localStorage` (`linux-wasm-history`) and restores it through JS-level ArrowUp/ArrowDown navigation.
- History restoration intentionally avoids boot-time shell command injection (no startup `printf` replay), keeping prompts clean and preventing history corruption.
- Clipboard integration is host-aware: on macOS, `Cmd+C` copies selected terminal text to the OS clipboard and `Cmd+V` pastes OS clipboard text into the guest shell input.

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
| `kernel`  | 13    | Core runtime — registry, config, dispatch, HTTP, shared libraries |
| `sys`     | 26    | System utilities — checksum, testing, versioning, CLI, MCP |
| `www`     | 20    | Web interface — SPA, admin, docs, playground, terminal |
| `llm`     | 4     | LLM providers — prompt interface, OpenAI, WebLLM |

## Kernel — 3-Layer Architecture

The kernel namespace is organized into three layers, enforced by a build-time lint in `build.rs`:

### Layer 0 — Shared Libraries

Non-callable Rust library crates (`source = "library"`) used as `[dependencies]` by other crates. They follow trait conventions (`.trait.toml` + `.features.json`) for registry visibility but use `src/` layout because they are multi-file crates.

| Trait | Description |
|-------|-------------|
| `kernel.logic` | Type system, registry model, platform abstraction (`kernel_logic` crate) |
| `kernel.wasm` | WASM browser kernel — wasm-pack target, ~26 traits (`traits_wasm` crate) |

### Layer 1 — Portable Traits (`wasm = true`)

Traits that compile for **both** native and `wasm32-unknown-unknown`. They use `kernel_logic::platform::*` or `#[cfg]` blocks for platform differences.

| Trait | Description |
|-------|-------------|
| `kernel.call` | Inter-trait dispatch by dot-notation |
| `kernel.cli` | Portable CLI processor (shared WASM/native) |
| `kernel.types` | TraitValue, TraitType, type coercion |

### Layer 2 — Native Infrastructure (`wasm = false`)

Traits that only compile for native targets. They form the runtime backbone: registry management, config loading, HTTP serving, plugin loading.

| Trait | Description |
|-------|-------------|
| `kernel.config` | traits.toml + env var overrides |
| `kernel.dispatcher` | Path resolution, arg validation, compiled dispatch |
| `kernel.dylib_loader` | Runtime cdylib plugin loading |
| `kernel.globals` | OnceLock statics (REGISTRY, CONFIG, TRAITS_DIR) |
| `kernel.main` | Bootstrap, trait-exists probe, introspection |
| `kernel.plugin_api` | C ABI export macro for plugins |
| `kernel.registry` | DashMap trait storage + interface resolution |
| `kernel.reload` | Registry hot-reload from disk |
| `kernel.serve` | HTTP server (actix-web) |

### Build-time lint

The `lint_kernel_layers()` function in `build.rs` classifies all `kernel.*` traits after discovery and emits `cargo:warning` summaries. It warns if any builtin/kernel-source trait is missing an explicit `wasm = true` or `wasm = false` declaration in its `.trait.toml`.

## Apptron IDE Filesystem Bridge

- The Apptron IDE shell at `traits/www/static/apptron-ide/index.html` boots `WanixRuntime` and listens for extension-host port handoff messages.
- The IDE must load `wanix.min.js` as an ES module import (not a classic `<script src=...>` tag) so `WanixRuntime` is initialized before bridge setup.
- The IDE filesystem bridge now sources Wanix ports from a hidden `/static/apptron/index.html` iframe runtime (same-origin), reusing the shell runtime that reliably mounts Linux files.
- The hidden Apptron iframe used for IDE bridge mode (`?ide_bridge=1`) must not create its own xterm binding for `#console/data`; otherwise the hidden shell console can steal terminal I/O from the IDE PTY.
- The Apptron shell bootstrap now prefers `wss://relay.traits.build/x/net` by default (unless `network` query param overrides it), probes WebSocket reachability, and falls back to the origin-derived `/x/net` endpoint when relay is unavailable.
- During runtime boot (non-IDE mode), Apptron injects a shell bootstrap command through the terminal data channel to create `/tmp/.traits-relay.sh`, source it, and export `WANIX_DEFAULT_RELAY`, `WANIX_NETWORK`, and `WANIX_NETWORK_SOURCE` with a `relay_status` alias for the active shell session. The bootstrap duplicates the `printf` write to tolerate occasional dropped leading bytes in early shell-channel writes.
- The workbench loader at `traits/www/static/apptron-ide/lib/vscode.js` enables the built-in `apptron-system` web extension and routes its IPC port to the page bridge.
- The web extension registers the `wanix` `FileSystemProvider`, and the default workspace is `wanix:/` to ensure Explorer starts from a guaranteed existing root.
- In the bridge provider, treat both `wanix:/` and an empty URI path as the same root directory before runtime readiness; VS Code can request root as an empty path during startup.
- In the `wanix` provider bootstrap path, `stat("/")` must be reported as a directory even before the remote FS is ready; returning a file here can leave Explorer mounted but visually empty.
- When the Wanix backend reaches `vm/1/fsys`, the provider should emit a root `FileChangeType.Changed` event for `wanix:/` so Explorer refreshes immediately.
- In `system/src/web/extension.ts`, keep terminal writes on a dedicated `openWritable("#console/data")` stream with a serialized write queue; `appendFile`-style per-keystroke writes can race during startup and trigger terminal runtime-exit errors.
