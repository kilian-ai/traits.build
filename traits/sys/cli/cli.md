# cli.rs

## Purpose

Dispatch trait calls from the CLI. Handles argument parsing, stdin pipe injection, usage printing on errors, and two entry points for invoking traits (positional args vs raw string args). Bootstrap and trait_exists logic lives in `main.rs` (crate root).

## Exports

* `run` — Async CLI entry point: parse clap, load config, route subcommands to trait calls
* `dispatch_trait` — Call a trait by path with `&[&str]` args (used by main.rs subcommands)
* `call_trait` — Call a trait by path with `&[String]` args (used by main.rs `call` command)

---

## parse_cli_value

### Purpose

Convert a single raw string argument into a typed TraitValue based on the parameter's declared type.

### Inputs

* raw: raw string from the command line
* param_type: the declared type of the parameter from the trait signature

### Outputs

* TraitValue — typed value (String, Int, Float, Bool, Json, etc.)

### State

reads: none

writes: none

### Side Effects

* none

### Dependencies

* serde_json::Value
* TraitValue

### Flow

1. Match param_type:
   - Int/Integer: parse as i64, fall back to string
   - Float/Number: parse as f64, fall back to string
   - Bool/Boolean: parse "true"/"false"/"1"/"0", fall back to string
   - Json/Object/Array/Any: attempt serde_json parse, fall back to wrapping as JSON string
   - String or other: return as TraitValue::String
2. Return the typed TraitValue

### Edge Cases

* Unparseable int/float/bool: silently falls back to String
* JSON param with invalid JSON: wrapped as JSON string value

### Example

```rust
parse_cli_value("42", &ParamType::Int) // → TraitValue::Int(42)
parse_cli_value("hello", &ParamType::String) // → TraitValue::String("hello")
parse_cli_value("{\"a\":1}", &ParamType::Json) // → TraitValue::Json({"a":1})
```

---

## parse_cli_args

### Purpose

Convert raw CLI string arguments into typed TraitValue vec using inline positional and flag-style parsing with registry signature lookup.

### Inputs

* trait_path: dot-notation trait path, e.g. "sys.checksum"
* raw_args: slice of raw string arguments from the command line

### Outputs

* Vec<TraitValue> — parsed and typed arguments ready for router.call()

### State

reads:
* crate::globals::REGISTRY — to look up trait signature for param names and types

writes:
* none

### Side Effects

* none

### Dependencies

* crate::globals::REGISTRY
* Registry::get
* parse_cli_value
* serde_json::Value
* TraitValue::from_json

### Flow

1. Look up trait entry in REGISTRY by trait_path
2. Get the trait's parameter signature (names, types, required/optional)
3. Scan raw_args for flag-style arguments (--name value or --name=value)
4. Assign flag args to their matching parameter by name
5. Assign remaining positional args in order to unfilled parameters
6. For each assigned arg, call parse_cli_value with the param's declared type
7. Return Vec<TraitValue> in parameter-definition order

### Edge Cases

* If trait not found in registry, falls back to returning all args as strings
* Flag args (--name) that don't match any parameter name: treated as positional
* Mixed positional and flag args: flags resolved first, positional fills remaining slots
* Empty raw_args: returns empty vec

### Example

```rust
let args = parse_cli_args("sys.checksum", &["hash".into(), "test".into()]);
// → [TraitValue::String("hash"), TraitValue::String("test")]

let args = parse_cli_args("sys.checksum", &["--data".into(), "hello".into(), "--action".into(), "hash".into()]);
// → [TraitValue::String("hash"), TraitValue::String("hello")]
```

---

## print_trait_usage

### Purpose

Print human-readable usage info to stderr for a trait, including parameter names, types, required/optional status, pipe hint, and descriptions. Called on argument mismatch errors.

### Inputs

* trait_path: dot-notation path of the trait to describe

### Outputs

* none (prints to stderr)

### State

reads:
* crate::globals::REGISTRY — to look up trait entry

writes:
* none

### Side Effects

* Writes to stderr via eprintln!

### Dependencies

* crate::globals::REGISTRY
* Registry::get

### Flow

1. Get global REGISTRY
2. Look up trait by path
3. Print "Usage: traits call {path} {params}" where optional params wrapped in []
4. Print trait description if non-empty
5. Print each parameter: name, type (Debug format), required/optional, "(accepts stdin)" if pipe=true, description

### Edge Cases

* If REGISTRY not initialized or trait not found, prints nothing
* Empty params list — skips "Parameters:" section
* Empty description — skips description line

### Example

Output for `sys.checksum`:
```
Usage: traits call sys.checksum <action> <data>
  Compute deterministic SHA-256 checksums: hash values, I/O pairs, or trait signatures

Parameters:
  action       String, required — Action: hash | io | signature | update
  data         Any, required (accepts stdin) — Data to checksum: ...
```

---

## read_stdin_pipe

### Purpose

Read all of stdin if it's piped (non-TTY). Returns trimmed content or None.

### Inputs

* none (reads from process stdin)

### Outputs

* Some(String) — trimmed stdin content if piped and non-empty
* None — if stdin is a terminal or content is empty

### State

reads:
* process stdin file descriptor

writes:
* none

### Side Effects

* Consumes stdin entirely (can only be called once per process)

### Dependencies

* std::io::IsTerminal
* std::io::Read (read_to_string)

### Flow

1. Check if stdin is a terminal — if yes, return None
2. Read entire stdin into buffer
3. Trim trailing \n and \r
4. If empty after trim, return None
5. Return Some(trimmed)

### Edge Cases

* Binary data on stdin — read_to_string may fail (returns None via .ok()?)
* Empty pipe (e.g. `echo -n "" |`) — returns None
* Multiple trailing newlines — all stripped

### Example

```
echo "hello" | traits call ...
# read_stdin_pipe() → Some("hello")
```

---

## maybe_inject_stdin

### Purpose

If stdin is piped and a positional argument is missing for the pipe-designated parameter, read stdin and inject it into the args vec at the correct position.

### Inputs

* trait_path: trait to look up parameter definitions for
* args: mutable vec of string arguments (modified in place)

### Outputs

* none (mutates args in place)

### State

reads:
* crate::globals::REGISTRY — to find pipe param index

writes:
* args — may append stdin content

### Side Effects

* Consumes stdin via read_stdin_pipe()

### Dependencies

* crate::globals::REGISTRY
* Registry::get
* read_stdin_pipe

### Flow

1. Get global REGISTRY, look up trait entry
2. Find pipe param index: first param with pipe=true, or fall back to index 0 if params exist
3. If no params, do nothing
4. If args already cover that index, do nothing (stdin not needed)
5. If args.len() <= pipe_idx, attempt read_stdin_pipe()
6. If stdin has content, pad args to pipe_idx with empty strings (gap case), then push stdin content

### Edge Cases

* No params → no injection
* Param already provided on command line → stdin ignored
* stdin is a terminal → no injection
* Explicit pipe=true on non-first param (e.g. index 1) takes priority over implicit first-param fallback
* Gap between args.len() and pipe_idx padded with empty strings (rare)

### Example

```
# sys.checksum has pipe=true on param index 1 (data)
# args = ["hash"], pipe_idx = 1, args.len() == 1 <= 1
echo test | traits call sys.checksum hash
# → args becomes ["hash", "test"]
```

---

## dispatch_trait

### Purpose

Bootstrap runtime and dispatch a trait call with `&[&str]` args. Used by main.rs for built-in subcommands (serve, list, info).

### Inputs

* config: &Config
* trait_path: dot-notation trait path
* args: slice of &str arguments

### Outputs

* Ok(()) — prints JSON result to stdout
* Err — prints usage on arg mismatch, returns error

### State

reads:
* config (via crate::bootstrap)
* globals (maybe_inject_stdin, parse_cli_args)

writes:
* globals (via crate::bootstrap)

### Side Effects

* Bootstraps entire runtime via crate::bootstrap() (filesystem reads, dylib loading)
* May consume stdin
* Prints JSON result to stdout
* Prints usage to stderr on mismatch errors
* Shuts down workers on exit

### Dependencies

* crate::bootstrap
* maybe_inject_stdin
* parse_cli_args
* Dispatcher::call
* print_trait_usage

### Flow

1. Call crate::bootstrap(config) to get dispatcher
2. Convert &str args to Vec<String>
3. Call maybe_inject_stdin to potentially fill from stdin
4. Call parse_cli_args to get typed args
5. Call router.call(trait_path, trait_args, default config)
6. On success: pretty-print JSON result to stdout
7. On error: if message contains "Argument count mismatch" or "expected", print usage; shutdown dispatcher; return error
8. Shutdown dispatcher

### Edge Cases

* Bootstrap failure → returns error immediately (no dispatcher to shut down)
* Router error without "Argument count mismatch" — no usage printed, just error

### Example

```rust
dispatch_trait(&config, "sys.version", &[]).await?;
// stdout: { "version": "260319", ... }
```

---

## call_trait

### Purpose

Bootstrap runtime and dispatch a trait call with `&[String]` args. Used by main.rs for the `call` subcommand.

### Inputs

* config: &Config
* path: dot-notation trait path
* args: slice of String arguments

### Outputs

* Ok(()) — prints JSON result to stdout
* Err — prints usage on arg mismatch, returns error

### State

reads:
* config (via crate::bootstrap)
* globals (maybe_inject_stdin, parse_cli_args)

writes:
* globals (via crate::bootstrap)

### Side Effects

* Bootstraps entire runtime via crate::bootstrap()
* May consume stdin
* Prints JSON to stdout
* Prints usage to stderr on mismatch
* Shuts down dispatcher on exit

### Dependencies

* crate::bootstrap
* maybe_inject_stdin
* parse_cli_args
* Dispatcher::call
* print_trait_usage

### Flow

1. Call crate::bootstrap(config)
2. Clone args to mutable vec
3. Call maybe_inject_stdin to potentially fill from stdin
4. Call parse_cli_args to get typed args
5. Call router.call(path, trait_args, default config)
6. On success: pretty-print JSON to stdout
7. On error: print usage if arg mismatch; shutdown dispatcher; return error
8. Shutdown dispatcher

### Edge Cases

* Identical to dispatch_trait except input type is &[String] vs &[&str]

### Example

```rust
call_trait(&config, "sys.checksum", &["hash".into(), "test".into()]).await?;
// stdout: { "ok": true, "checksum": "4d967a..." }
```

---

## Internal Structure

```
cli.rs
├── Cli / Commands          (private) clap derive structs
├── run                     (public)  async entry: parse clap, route subcommands
├── parse_cli_value         (private) convert single raw arg to typed TraitValue
├── parse_cli_args          (private) inline positional + flag parsing with registry signature lookup
├── print_trait_usage       (private) stderr usage printer
├── print_result            (private) stdout JSON/CLI formatter
├── read_stdin_pipe         (private) stdin reader
├── maybe_inject_stdin      (private) arg injection from stdin
├── collapse_shell_globs    (private) repack shell-expanded globs
├── dispatch_trait          (public)  &str args entry point
└── call_trait              (public)  String args entry point
```

`dispatch_trait` and `call_trait` are nearly identical — they differ only in input type. Both follow the same pipeline: crate::bootstrap → inject stdin → parse args → dispatcher.call → print result or error.

`bootstrap()` lives in main.rs (crate root), shared by both CLI paths and the HTTP server.

## Notes

* `read_stdin_pipe` can only be called once per process (stdin is consumed). Both `dispatch_trait` and `call_trait` call it indirectly via `maybe_inject_stdin`, but only one of them runs per CLI invocation.
* The pipe param resolution has two tiers: explicit `pipe=true` in TOML takes priority; implicit fallback uses param index 0.
* `parse_cli_args` does inline positional and flag-style argument parsing using the trait's registry signature for param names and types. Type coercion is handled by `parse_cli_value`. The former `sys.parse_args` trait was absorbed into this module.
