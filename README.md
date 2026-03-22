# Traits — Kernel

A Rust-native trait platform. Traits are typed, composable function objects defined in `.trait.toml` files with companion Rust source. All 26 traits compile directly into a single `traits` binary — zero worker processes, zero HTTP overhead for inter-trait calls. Every trait is callable via CLI, REST API, or internal dispatch.

**Live:** [traits.build](https://traits.build)  
**Repo:** [github.com/kilian-ai/traits.build](https://github.com/kilian-ai/traits.build)

## Traits (26)

### kernel.* — Core runtime (11)

| Trait | Description |
|-------|-------------|
| `kernel.call` | Call another trait by dot-notation path (dispatch mechanism) |
| `kernel.config` | Configuration loader: traits.toml parsing, env var overrides |
| `kernel.dispatcher` | Core trait execution engine: path resolution, arg validation, dispatch, handle management |
| `kernel.dylib_loader` | Dynamic shared-library loader for trait cdylib plugins |
| `kernel.globals` | Global OnceLock statics: REGISTRY, CONFIG, TRAITS_DIR, HANDLES, START_TIME |
| `kernel.main` | Binary entry point, system bootstrap, compiled module list introspection |
| `kernel.plugin_api` | C ABI export macro for cdylib trait plugins. Returns ABI contract + installed plugins |
| `kernel.registry` | Trait registry: loading, lookup, interface resolution, bindings |
| `kernel.reload` | Reload trait registry from disk |
| `kernel.serve` | Start the HTTP API server (actix-web, background trait) |
| `kernel.types` | Cross-language type system: TraitType, TraitValue, wire protocol types |

### sys.* — System utilities (9)

| Trait | Description |
|-------|-------------|
| `sys.checksum` | Deterministic SHA-256 checksums (hash values, I/O pairs, trait signatures) |
| `sys.cli` | CLI bootstrap, trait dispatch, stdin injection, arg parsing, result formatting |
| `sys.info` | Show detailed trait info (delegates to `sys.registry info`) |
| `sys.list` | List all registered traits (delegates to `sys.registry list`) |
| `sys.ps` | List running background traits with process details |
| `sys.registry` | Registry read API — list, info, tree, namespaces, count, get, search |
| `sys.snapshot` | Snapshot a trait version (YYMMDD, or YYMMDD.HHMMSS for same-day) |
| `sys.test_runner` | Run `.features.json` tests — example dispatch + shell commands |
| `sys.version` | Show trait system version, or generate YYMMDD version strings |

### www.* — Web interface (6)

| Trait | Description |
|-------|-------------|
| `www.traits.build` | Landing page for traits.build |
| `www.admin` | Admin dashboard (Basic Auth protected) |
| `www.admin.deploy` | Deploy latest version to Fly.io |
| `www.admin.fast_deploy` | Fast deploy: build amd64 binary in Docker + upload via sftp + restart |
| `www.admin.scale` | Scale Fly.io machines (0 = stop all, 1+ = start) |
| `www.admin.destroy` | Destroy all Fly.io machines for the app |

## Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  CLI (main)  │────▶│  Dispatcher  │────▶│  Compiled    │
│  call/serve  │     │  resolve +   │     │  trait fns   │
│  list/info   │     │  coerce args │     │  (Rust)      │
└──────────────┘     └──────────────┘     └──────────────┘
                            │
                     ┌──────▼──────┐
                     │  Registry   │
                     │  (DashMap)  │
                     └─────────────┘
```

- **Entry point** (`src/main.rs` = `traits/kernel/main/main.rs`) — binary entry, `bootstrap()` loads config → registry → dylibs → resolves interfaces → creates Dispatcher. Declares `[requires]` for dispatcher, registry, config, globals, dylib_loader — all resolved through the interface system
- **CLI** (`traits/sys/cli/cli.rs`) — clap parsing, trait dispatch, arg coercion, result formatting. Any unknown subcommand is tried as `sys.{name}` then `kernel.{name}`
- **Dispatcher** (`traits/kernel/dispatcher/`) — resolves trait paths, validates/coerces arguments, dispatches to compiled modules
- **Registry** (`traits/kernel/registry/`) — loads `.trait.toml` definitions from disk + compiled builtins, concurrent DashMap storage, interface resolution
- **Serve** (`traits/kernel/serve/`) — actix-web HTTP server, runs as a background trait. Routes URL paths to `www/webpage` interface providers via keyed bindings
- **Dylib loader** (`traits/kernel/dylib_loader/`) — optional dynamic library loading for external trait plugins
- **Types** (`traits/kernel/types/`) — `TraitValue`, `TraitEntry`, `TraitSignature`, wire protocol types

## CLI

Every `sys.*` trait is available as a direct subcommand — no `call sys.` prefix needed:

```bash
# Start the server (default when no subcommand given)
traits serve --port 8090

# List all traits
traits list

# Show trait info
traits info sys.checksum

# Run tests
traits test_runner '*'

# Compute a checksum
traits checksum hash hello

# Snapshot a trait version
traits snapshot sys.version
```

The `call` subcommand dispatches any trait by full path (useful for non-sys namespaces):

```bash
traits call kernel.serve 8090
```

Arguments are positional and mapped to params via the CLI's inline arg parser. Pipe input is supported for params with `pipe = true`:

```bash
echo hello | traits checksum hash
```

## REST API

All traits are callable over HTTP when the server is running:

```bash
# Call a trait
curl -X POST http://127.0.0.1:8090/traits/sys/checksum \
  -H 'Content-Type: application/json' \
  -d '{"args": ["hash", "hello"]}'

# List traits
curl http://127.0.0.1:8090/traits/sys/list

# Reload registry
curl -X POST http://127.0.0.1:8090/traits/kernel/reload

# SSE streaming
curl 'http://127.0.0.1:8090/traits/sys/list?stream=1'

# Health check
curl http://127.0.0.1:8090/health
```

## Testing

Every trait has a `.features.json` file with example tests (internal dispatch) and command tests (shell):

```bash
# Run all tests
traits test_runner '*'

# Run tests for a specific trait
traits test_runner sys.checksum

# Verbose output
traits test_runner '*' true
```

## Trait Definition Format

Each trait lives in `traits/{namespace}/<name>/` with:
- `<name>.trait.toml` — metadata, params, returns, implementation pointer, interfaces
- `<name>.rs` — Rust source (compiled into the binary via `build.rs`)
- `<name>.features.json` — test cases (optional)
- `<name>.md` — documentation (optional)

Example `call.trait.toml`:

```toml
[trait]
description = "Call another trait by dot-notation path"
version = "v260322"
author = "system"
tags = ["system", "meta", "dispatch"]
provides = ["kernel/call"]

[signature]
params = [
  { name = "trait_path", type = "string", description = "Dot-notation trait path" },
  { name = "args", type = "list<any>", description = "Arguments forwarded to the target trait", optional = true }
]

[signature.returns]
type = "any"
description = "Result from the called trait"

[implementation]
language = "rust"
source = "builtin"
entry = "call"
```

## File Layout

```
src/
  main.rs              # Binary entry point (Cargo.toml target)
  main.trait.toml      # Trait definition for kernel.main

traits/kernel/
  main/                # Bootstrap, trait_exists(), introspection
  call/                # Inter-trait dispatch by dot-notation path
  config/              # Server configuration (traits.toml + env vars)
  dispatcher/          # Path resolution, arg validation, compiled dispatch
  dylib_loader/        # Dynamic library trait plugins
  globals/             # Global state (OnceLock registry, config, traits dir)
  plugin_api/          # C ABI export macro for cdylib plugins
  registry/            # Trait registry (load, store, lookup, interface resolution)
  reload/              # Hot-reload registry from disk
  serve/               # HTTP API server (actix-web)
  types/               # Core types (TraitValue, TraitEntry, signatures)

traits/sys/
  checksum/            # SHA-256 hashing primitives
  cli/                 # CLI clap parsing, dispatch routing
  info/                # Trait introspection (delegates to registry)
  list/                # Trait listing (delegates to registry)
  ps/                  # Background trait process listing
  registry/            # Registry read API (list, info, tree, search)
  snapshot/            # YYMMDD version snapshots
  test_runner/         # Test discovery and execution
  version/             # Version generation

traits/www/
  traits/build/        # Landing page (www.traits.build)
  admin/               # Admin dashboard
    deploy/            # Fly.io deployment
    fast_deploy/       # Fast binary deploy via sftp
    scale/             # Fly.io machine scaling
    destroy/           # Fly.io machine destruction
```

## Interface System

Traits declare dependencies through interfaces — named contracts that are provided, required, and bound entirely within `.trait.toml` files. There are no separate interface definition files.

### How it works

1. **Provide** — a trait declares what interfaces it satisfies via `provides` in `[trait]`:
   ```toml
   [trait]
   provides = ["kernel/dispatcher"]
   ```

2. **Require** — a consuming trait declares `[requires]` with a logical key mapped to an interface name:
   ```toml
   [requires]
   dispatcher = "kernel/dispatcher"
   ```

3. **Bind** — the same trait provides default wiring in `[bindings]`, mapping the key to a concrete trait:
   ```toml
   [bindings]
   dispatcher = "kernel.dispatcher"
   ```

4. **Resolve at runtime** — the registry resolves interfaces through a priority chain:
   - Per-call overrides (highest priority)
   - Global bindings
   - Caller's `[bindings]`
   - Auto-discover (find providers, pick by priority)

### URL-keyed bindings

`kernel.serve` uses interface keys as URL paths, binding each route to a `www/webpage` provider:

```toml
[requires]
"/" = "www/webpage"
"/admin" = "www/webpage"
dispatcher = "kernel/dispatcher"

[bindings]
"/" = "www.traits.build"
"/admin" = "www.admin"
dispatcher = "kernel.dispatcher"
```

### Current interfaces

| Interface | Providers | Required by |
|-----------|-----------|-------------|
| `kernel/dispatcher` | `kernel.dispatcher` | `kernel.main`, `kernel.serve` |
| `kernel/registry` | `kernel.registry` | `kernel.main` |
| `kernel/config` | `kernel.config` | `kernel.main` |
| `kernel/globals` | `kernel.globals` | `kernel.main` |
| `kernel/dylib_loader` | `kernel.dylib_loader` | `kernel.main` |
| `www/webpage` | `www.traits.build`, `www.admin` | `kernel.serve` (keyed by URL path) |

## Build System

`build.rs` auto-discovers all traits and generates Rust code at compile time:

- **`builtin_traits.rs`** — embeds all `.trait.toml` definitions via `include_str!`
- **`compiled_traits.rs`** — module declarations + `dispatch_compiled()` match table
- **`kernel_modules.rs`** — crate-level `pub mod` declarations for kernel subdirectories
- **`cli_formatters.rs`** — optional CLI output formatters from `*_cli.rs` companion files

Version is auto-computed as `vYYMMDD` (or `vYYMMDD.HHMMSS` for same-day rebuilds). Source file checksums are computed and written back to each `.trait.toml`.

```bash
cargo build --release
./target/release/traits serve --port 8090
```

## Deployment

Deployed on Fly.io:

```bash
cd path/to/traits.build
docker buildx build --platform linux/amd64 -t registry.fly.io/your-fly-app:latest .
fly deploy --now --local-only --image registry.fly.io/your-fly-app:latest
```

See [docs/deploy.md](docs/deploy.md) for details, [docs/release.md](docs/release.md) for GitHub releases.
