---
sidebar_position: 8
---

# CLI Reference

The `traits` binary doubles as a CLI tool. Every `sys.*` trait is available as a subcommand.

## Usage

```bash
traits <command> [args...]
```

## Commands

### `traits serve`

Start the HTTP server.

```bash
traits serve --port 8090
```

The server listens on all interfaces (`0.0.0.0`). Set `TRAITS_PORT` environment variable as an alternative to `--port`.

### `traits list`

List all registered traits.

```bash
# List all traits
traits list

# List traits in a namespace
traits list sys
```

### `traits info`

Show detailed metadata for a trait.

```bash
traits info sys.checksum
```

Output includes signature, parameters, return type, version, provides/requires.

### `traits call`

Call any trait directly.

```bash
traits call sys.checksum hash "hello world"
traits call sys.version
traits call sys.registry tree
```

Arguments are passed positionally after the trait path.

### `traits checksum`

Compute SHA-256 checksums.

```bash
# Hash a value
traits checksum hash "hello"

# Hash input/output pair
traits checksum io '["add", 2, 3]' 5

# Hash a trait signature
traits checksum signature sys.checksum
```

### `traits test_runner`

Run trait tests from `.features.json` files.

```bash
# Run all tests
traits test_runner '*'

# Run tests for a namespace
traits test_runner 'sys.*'

# Verbose output
traits test_runner '*' true

# Skip shell command tests
traits test_runner '*' false true
```

### `traits version`

Show version information.

```bash
traits version
```

### `traits ps`

List running background traits.

```bash
traits ps
```

### `traits snapshot`

Snapshot a trait's version to today's date.

```bash
traits snapshot sys.checksum
```

## Pipe support

The CLI supports stdin piping:

```bash
echo "hello" | traits checksum hash
```

Inside the browser terminal (`kernel.cli` session), the shared shell parser also supports:

```bash
cmd < input.txt
cmd > output.txt
cmd >> output.txt
cmd1 | cmd2 | cmd3
cmd1 && cmd2
cmd1 || cmd2
```

## Semantic userland tools in `kernel.cli`

The portable terminal session includes shell-compatible builtins aimed at agent workflows:

- `echo`, `pwd`
- `cd`, `mkdir`
- `cat`, `head`, `tail`
- `grep` (`-i`, `-n`, `-v`)
- `wc` (`-l`, `-w`, `-c`)
- `sed` (`s/pattern/replacement/[gi]` with optional `-i` in-place write)
- `test` and `[ ... ]`
- `find` (supports `-name`, `-type`, `-mindepth`, `-maxdepth`)
- `curl` (mapped to `sys.call`)
- `lua` (run Lua from inline code or `.lua` files)
- `js` (run JavaScript from inline code or `.js` files in WASM terminal)
- `vi` and `ee` (preview file, or write/append via args/stdin)

For `js`, scripts that use `prompt()` should provide input via trailing args or JSON stdin:

```bash
js tic_tac_toe.js 1 5 2 9 3
js tic_tac_toe.js '{"stdin":["1","5","2","9","3"]}'
```

In the WASM terminal, `js` now supports interactive `prompt()` loops: when stdin is exhausted, the terminal asks for the next value and re-runs the script with accumulated `input.stdin` entries.

`cd` now updates per-session working directory state, and path-taking commands resolve relative paths against that `cwd`.

`mkdir` supports plain mode and `-p` parent creation in the CLI VFS.

`vi` and `ee` are lightweight practical modes: preview with line numbers by default, write with `vi <file> <text>`, append with `-a`, or pipe content (`echo "text" | vi notes.txt`).

## Portable CLI backend interfaces

The shared CLI core in `traits/kernel/cli/cli.rs` now uses three backend interfaces:

- `CliCallBackend` for dispatch and registry reads (`call`, `list_all`, `get_info`, `search`, `all_paths`, `version`)
- `CliHistoryBackend` for interactive parameter history persistence (`load_param_history`, `save_param_history`)
- `CliExamplesBackend` for interactive example suggestions (`load_examples`)

`CliSession` consumes these interfaces so native and WASM backends can share one session/runtime implementation while keeping persistence and example loading pluggable.

## Terminal background command routing

The browser terminal routes CLI session commands through the SDK background layer (`Traits.backgroundCall`) instead of implementing separate command switches in `terminal.js`.

This keeps command behavior centralized in SDK adapters (`sdk.background.worker`, `sdk.background.direct`, `sdk.background.tokio`) and avoids duplicating CLI command handling between terminal and SDK code.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Trait not found or execution error |
| 2 | Argument error |
