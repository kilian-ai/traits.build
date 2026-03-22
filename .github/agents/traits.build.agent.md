# traits.build — Agent Instructions

> Pure Rust kernel for the Traits platform. Deployed at traits.build via Fly.io.
>
> - **Repository:** https://github.com/kilian-ai/traits.build
> - **Homepage:** https://traits.build/

---

## Project Overview

**traits.build** is the Rust-only kernel of the Traits platform. No JavaScript workers, no Python workers, no GUI, no polyglot — just a clean Rust kernel that compiles every trait directly into a single binary.

---

## Directory Structure

```
Polygrait/A. traits.build/
├── build.rs              # Build system: trait discovery, codegen, dispatch table
├── Cargo.toml            # Workspace: root + plugin_api + www.build (cdylib)
├── Cargo.lock
├── Dockerfile            # Multi-stage: rust:latest → debian:trixie-slim
├── fly.toml              # Fly.io: app=polygrait-api, region=iad, port=8090
├── traits.toml           # Runtime config: port, timeout, traits_dir
├── build.sh              # Build script
├── src/
│   └── main.rs           # Binary entry point (thin — delegates to kernel.main)
├── traits/
│   ├── kernel/           # 11 kernel modules (compiled in)
│   │   ├── call/         # Inter-trait dispatch
│   │   ├── config/       # traits.toml parsing + env var overrides
│   │   ├── dispatcher/   # Path resolution, arg validation, compiled dispatch
│   │   ├── dylib_loader/ # cdylib plugin discovery and loading
│   │   ├── globals/      # OnceLock statics: REGISTRY, CONFIG, TRAITS_DIR
│   │   ├── main/         # Bootstrap, trait-exists probe, introspection
│   │   ├── plugin_api/   # C ABI export macro (workspace member)
│   │   ├── registry/     # DashMap of trait defs, interface resolution
│   │   ├── reload/       # Hot-reload registry from disk
│   │   ├── serve/        # actix-web HTTP server, CORS, SSE streaming
│   │   └── types/        # TraitValue, TraitType, type coercion
│   ├── sys/              # 11 system traits (compiled as builtins)
│   │   ├── checksum/     # SHA-256 hashing
│   │   ├── cli/          # Clap parsing, subcommand dispatch
│   │   ├── info/         # Show detailed trait metadata
│   │   ├── list/         # List all traits
│   │   ├── mcp/          # MCP stdio server (JSON-RPC 2.0)
│   │   ├── openapi/      # OpenAPI 3.0 spec generation
│   │   ├── ps/           # List running background traits
│   │   ├── registry/     # Registry read API (tree, namespaces, search)
│   │   ├── snapshot/     # Snapshot trait versions
│   │   ├── test_runner/  # Run .features.json tests
│   │   └── version/      # YYMMDD version generation
│   └── www/              # 5 web traits
│       ├── admin/        # Admin dashboard + deploy/scale/destroy actions
│       │   ├── admin.rs
│       │   ├── deploy/
│       │   ├── destroy/
│       │   └── scale/
│       └── traits/build/ # Landing page (this site)
└── scripts/
```

---

## Build System

The `build.rs` at project root does all code generation:

1. **Trait discovery** — scans `traits/` recursively for `.trait.toml` files
2. **`builtin_traits.rs`** — embeds all TOML definitions via `include_str!`
3. **`compiled_traits.rs`** — generates module declarations + `dispatch_compiled()` function
4. **`kernel_modules.rs`** — crate-level `mod` declarations for kernel subdirectories
5. **`cli_formatters.rs`** — optional CLI output formatters
6. **Version management** — YYMMDD format, auto-bumps on same-day changes
7. **Checksum validation** — SHA-256 of .rs files, bumps version if changed

Build & run:
```bash
cargo build --release
./target/release/traits serve --port 8090
# or just:
./target/release/traits   # reads TRAITS_PORT env var
```

---

## API Convention

```
# REST — POST with args array
POST http://127.0.0.1:8090/traits/{namespace}/{name}
Body: {"args": [arg1, arg2, ...]}

# CLI — every sys.* trait is a direct subcommand
traits list
traits checksum hash "hello"
traits info sys.checksum
traits test_runner '*'

# Health check (used by Fly.io)
GET /health

# MCP — stdio JSON-RPC 2.0
traits mcp
# Reads JSON-RPC from stdin, writes responses to stdout.
# All traits are exposed as tools: dot paths → underscore names
# e.g. sys.checksum → sys_checksum
```

---

## Trait Inventory (29 traits)

### Kernel (11) — Core runtime

| Trait | Description | Provides |
|-------|-------------|----------|
| `kernel.main` | Entry point, bootstrap, introspection | — |
| `kernel.dispatcher` | Path resolution, arg validation, compiled dispatch | `kernel/dispatcher` |
| `kernel.registry` | DashMap trait lookup, interface resolution | `kernel/registry` |
| `kernel.config` | traits.toml + env var overrides | `kernel/config` |
| `kernel.serve` | actix-web HTTP server (background) | `kernel/serve` |
| `kernel.dylib_loader` | cdylib plugin discovery and loading | — |
| `kernel.types` | TraitValue, TraitType, type parsing | `kernel/types` |
| `kernel.globals` | OnceLock statics (REGISTRY, CONFIG, etc.) | `kernel/globals` |
| `kernel.call` | Inter-trait dispatch by dot-notation | — |
| `kernel.plugin_api` | C ABI export macro for cdylib plugins | `kernel/plugin_api` |
| `kernel.reload` | Hot-reload registry from disk | — |

### Sys (11) — System utilities

| Trait | Description | Provides |
|-------|-------------|----------|
| `sys.cli` | Clap parsing, subcommand dispatch, pipe support | `sys/cli` |
| `sys.registry` | Read API: list, info, tree, namespaces, search | `sys/registry` |
| `sys.checksum` | SHA-256 checksums (values, I/O pairs, signatures) | `sys/checksum` |
| `sys.mcp` | MCP stdio server — JSON-RPC 2.0 over stdin/stdout | `sys/mcp` |
| `sys.openapi` | OpenAPI 3.0 spec generation from trait registry | `sys/openapi` |
| `sys.version` | YYMMDD version string generation | `sys/version` |
| `sys.snapshot` | Snapshot trait version to date format | `sys/snapshot` |
| `sys.test_runner` | Run .features.json tests (dispatch + shell) | `sys/test_runner` |
| `sys.list` | List all registered traits | `sys/list` |
| `sys.info` | Detailed trait metadata + signatures | `sys/info` |
| `sys.ps` | List running background trait processes | — |

### WWW (7) — Web interface

| Trait | Description | Provides |
|-------|-------------|----------|
| `www.traits.build` | Landing page HTML | `www/webpage` |
| `www.docs.api` | API documentation (Redoc) | `www/webpage` |
| `www.admin` | Admin dashboard (Basic Auth) | `www/webpage` |
| `www.admin.deploy` | Deploy to Fly.io | — |
| `www.admin.fast_deploy` | Fast deploy via Docker + sftp | — |
| `www.admin.scale` | Scale Fly.io machines (0=stop, 1+=start) | — |
| `www.admin.destroy` | Destroy Fly.io machines | — |

---

## Interface System

Traits declare dependencies via `[requires]` / `[bindings]` / `provides` in `.trait.toml`:

```toml
[trait]
provides = ["kernel/dispatcher"]

[requires]
registry = "kernel/registry"

[bindings]
registry = "kernel.registry"
```

Resolution chain: **per-call overrides → global bindings → caller bindings → auto-discover**

Key dependency examples:
- `kernel.serve` requires: `www/webpage`, `kernel/dispatcher`
- `kernel.main` requires: `kernel/dispatcher`, `kernel/registry`, `kernel/config`, `kernel/globals`, `kernel/dylib_loader`
- `sys.cli` requires: `kernel/config`, `kernel/call`, `kernel/serve`

---

## Type System

```rust
TraitType: int, float, string, bool, bytes, null, any, handle, list<T>, map<K,V>, T?
TraitValue: Null, Bool, Int(i64), Float(f64), String, List, Map, Bytes
```

Conversions: `to_json()`, `from_json()`, type coercion at dispatch time.

---

## Deployment (Fly.io)

- **App:** `polygrait-api`
- **Region:** `iad` (Ashburn, Virginia)
- **IPv4:** `66.241.125.245`
- **IPv6:** `2a09:8280:1::d5:c7cb:0`
- **VM:** shared CPU, 1 vCPU, 512 MB RAM
- **Port:** 8090 (internal), HTTPS forced
- **Auto-scaling:** 0–2 machines, auto-stop/auto-start
- **Health check:** `GET /health` every 30s
- **Admin auth:** HTTP Basic Auth (`ADMIN_PASSWORD` Fly secret)

### Deploy workflow:
```bash
cd "Polygrait/A. traits.build"
# Build for amd64 (Fly runs amd64, Mac is aarch64)
docker buildx build --platform linux/amd64 -t registry.fly.io/polygrait-api:deployment-vN .
# Push and deploy
fly deploy --now --local-only --image registry.fly.io/polygrait-api:deployment-vN
```

### Dockerfile:
- Stage 1: `rust:latest` — `cargo build --release`
- Stage 2: `debian:trixie-slim` — only `ca-certificates`, `curl`
- No Node, no Python, no workers
- `CMD ["traits"]` — reads `TRAITS_PORT` env var

---

## Testing

Each trait has a `.features.json` file with example-based and command-based tests:

```bash
# Run all tests
traits test_runner '*'

# Run specific namespace
traits test_runner 'sys.*'
```

Test types: `exit_code`, `contains`, `matches` (regex), `json_path`

---

## What's NOT in this branch

| Component | Status | Reason |
|-----------|--------|--------|
| JS/Python workers | Not present | Rust-only kernel |
| GUI (app.js, Blockly, etc.) | Not present | API-only server |
| Transpilers | Not present | No code generation |
| MCP server | **Present** | `sys.mcp` — native Rust, stdio transport, `traits mcp` |
| Programs (saved JSON) | Not present | No program storage |
| trait_defs/ (238 traits) | Not present | Replaced by 25 compiled traits |
| Organization/Chat/Packages | Not present | Minimal core only |
| Browser automation | Not present | No Playwright traits |
| Networking traits | Not present | No HTTP/IRC/DNS traits |

---

## Conventions

- **All code is Rust.** No JS/Python traits in this branch.
- **`source = "builtin"`** for all compiled traits. `source = "dylib"` for cdylib plugins only.
- **Trait files live in `traits/{namespace}/{name}/`** — each directory has `name.trait.toml` + `name.rs` + `name.features.json`
- **build.rs auto-discovers** everything — no manual module registration needed.
- **Interface wiring is mandatory** — declare `[requires]` and `[bindings]` for all cross-trait dependencies.
- **Version format:** `vYYMMDD` or `vYYMMDD.HHMMSS` for same-day bumps.
- **After modifying traits:** rebuild with `cargo build --release`; checksums and versions auto-update.

## Trait .trait.toml Template

```toml
[trait]
description = "Short description"
version = "v260321"
author = "system"
tags = ["namespace", "category"]
provides = ["namespace/interface"]

[signature]
params = [
  { name = "param1", type = "string", description = "desc", required = true },
]

[signature.returns]
type = "string"
description = "Return description"

[implementation]
language = "rust"
source = "builtin"
entry = "function_name"

[requires]
dep = "namespace/interface"

[bindings]
dep = "namespace.concrete_trait"
```


##AGENT RULSE:
- Always commit to git after making changes to the codebase, with a clear and concise commit message describing the changes made.
- Always run the build script (`build.sh`) after making changes to ensure that the code compiles correctly and that any generated files are updated.
- Always rebuild the binary and restart the local server after making changes to the codebase to ensure that the changes take effect and to test that everything is working correctly.