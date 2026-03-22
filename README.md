# Traits — Kernel

A Rust-native trait platform. Traits are typed, composable function objects defined in `.trait.toml` files with companion Rust source. All `sys.*` and `kernel.*` traits compile directly into the `traits` binary — zero worker processes, zero HTTP overhead for inter-trait calls. Every kernel trait is callable and returns introspection data.

## Traits

### kernel.*

| Trait | Description |
|-------|-------------|
| `kernel.call` | Call another trait by dot-notation path (dispatch mechanism) |
| `kernel.config` | Configuration loader: traits.toml parsing, env var overrides |
| `kernel.dispatcher` | Core trait execution engine: path resolution, arg validation, dispatch |
| `kernel.dylib_loader` | Dynamic shared-library loader for trait cdylib plugins |
| `kernel.globals` | Global OnceLock statics: REGISTRY, CONFIG, TRAITS_DIR, HANDLES, START_TIME |
| `kernel.main` | Binary entry point, system bootstrap, compiled module list introspection |
| `kernel.plugin_api` | C ABI export macro for cdylib trait plugins. Returns ABI contract + installed plugins |
| `kernel.registry` | Trait registry: loading, lookup, interface resolution, bindings |
| `kernel.reload` | Reload trait registry from disk |
| `kernel.serve` | Start the HTTP API server |
| `kernel.types` | Cross-language type system: TraitType, TraitValue, wire protocol types |

### sys.*

| Trait | Description |
|-------|-------------|
| `sys.checksum` | Deterministic SHA-256 checksums (hash, I/O pairs, signatures) |
| `sys.cli` | CLI bootstrap, trait dispatch, stdin injection, arg parsing, result formatting |
| `sys.info` | Show detailed trait info (delegates to `sys.registry info`) |
| `sys.list` | List all registered traits (delegates to `sys.registry list`) |
| `sys.ps` | List running background traits with process details |
| `sys.registry` | Registry read API — list, info, tree, namespaces, count, get, search |
| `sys.snapshot` | Snapshot a trait version (YYMMDD, or YYMMDD.HHMMSS for same-day) |
| `sys.test_runner` | Run `.features.json` tests — example dispatch + shell commands |
| `sys.version` | Generate trait versions in YYMMDD date format |

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

- **Entry point** (`traits/kernel/main/main.rs`) — binary entry, `bootstrap()` (registry + dylibs + interface resolution + globals → Dispatcher), `trait_exists()` probe, `main_info()` introspection. `src/` is a symlink to `traits/kernel/main/`. Declares `[requires] dispatcher = "kernel/dispatcher"` resolved through the interface system
- **CLI** (`traits/sys/cli/cli.rs`) — clap parsing, trait dispatch, arg coercion, result formatting
- **Dispatcher** (`traits/kernel/dispatcher/`) — resolves trait paths, validates/coerces arguments, dispatches to compiled modules
- **Registry** (`traits/kernel/registry/`) — loads `.trait.toml` definitions from disk + compiled builtins, concurrent DashMap storage
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
traits call sys.checksum hash hello
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
curl -X POST http://127.0.0.1:8090/traits/sys/reload
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

Each trait lives in `traits/sys/<name>/` with:
- `<name>.trait.toml` — metadata, params, returns, implementation pointer
- `<name>.rs` — Rust source (compiled into the binary via `build.rs`)
- `<name>.features.json` — test cases
- `<name>.md` — documentation

Example `call.trait.toml`:

```toml
[trait]
description = "Call another trait by dot-notation path"
version = "v260322"
author = "system"
tags = ["system", "meta", "dispatch"]

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
traits/sys/
  call/            # Inter-trait dispatch
  checksum/        # SHA-256 hashing primitives
  info/            # Trait introspection (delegates to registry)
  list/            # Trait listing (delegates to registry)
  registry/        # Registry read API (list, info, tree, search, etc.)
  reload/          # Hot-reload registry from disk
  serve/           # HTTP API server
  snapshot/        # YYMMDD version snapshots
  test_runner/     # Test discovery and execution
  version/         # Version generation

src/ → traits/kernel/main/  (symlink)

traits/kernel/
  main/            # Binary entry point, bootstrap(), trait_exists()
  cli/             # CLI clap parsing, dispatch routing
  config/          # Server configuration (traits.toml)
  dispatcher/      # Trait resolution, arg validation, compiled dispatch
  dylib_loader/    # Dynamic library trait plugins
  globals/         # Global state (OnceLock registry, config, traits dir)
  plugin_api/      # C ABI export macro for cdylib plugins
  registry/        # Trait registry (load, store, lookup)
  serve/           # HTTP API server (actix-web)
  types/           # Core types (TraitValue, TraitEntry, signatures)

traits/kernel/
  dispatcher.interface.toml  # Interface contract for the dispatcher

traits/www/
  webpage.interface.toml     # Interface contract for web pages
```

## Interface System

Traits declare dependencies through interfaces — typed contracts that can be satisfied by different implementations.

### How it works

1. **Define an interface** — `{namespace}/{name}.interface.toml` declares the contract:
   ```toml
   [interface]
   description = "Core trait execution engine"
   [signature.returns]
   type = "object"
   ```

2. **Provide it** — a trait declares `provides = ["namespace/interface"]` in `[trait]`:
   ```toml
   [trait]
   provides = ["kernel/dispatcher"]
   ```

3. **Require it** — a consuming trait declares `[requires]` with a keyed slot and `[bindings]` for the default wiring:
   ```toml
   [requires]
   dispatcher = "kernel/dispatcher"

   [bindings]
   dispatcher = "kernel.dispatcher"
   ```

4. **Resolve at runtime** — the registry resolves interfaces through a 4-level chain:
   - Per-call overrides (highest priority)
   - Global bindings
   - Caller's `[bindings]`
   - Auto-discover (find providers, pick by priority)

### Current interfaces

| Interface | Provider | Required by |
|-----------|----------|-------------|
| `kernel/dispatcher` | `kernel.dispatcher` | `kernel.main` |
| `www/webpage` | `www.traits.build` | `kernel.serve` (keyed by URL path) |
