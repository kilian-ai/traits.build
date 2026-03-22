# dispatcher.rs

## Purpose

Core dispatcher for the Traits kernel. Merges the former Router (resolution/validation) and WorkerManager (execution) into a single module. Resolves trait paths, validates arguments, coerces types, handles interface/binding resolution, manages the handle protocol, and executes traits directly through compiled modules. Sits between the API/CLI layer and the compiled trait implementations.

## Exports

* `CallConfig` — configuration struct carrying interface overrides, trait overrides, and per-override param defaults
* `RouterError` — error enum for all dispatch failures (not found, arg count, type mismatch, worker error, timeout, handle error)
* `Dispatcher` — the core dispatcher struct
* `compiled` — auto-generated module containing all compiled trait dispatch functions (from build.rs)

---

## compiled (module)

### Purpose

Auto-generated module included from build.rs output. Contains dispatch functions for all compiled Rust traits discovered in the traits/ directory at build time.

### Key Functions

* `dispatch(path, args)` — dispatch a trait call by path with JSON Value args, returns Option<Value>
* `dispatch_trait_value(path, args)` — dispatch with TraitValue args, returns Option<TraitValue>
* `dispatch_compiled(path, args)` — dispatch for dylib cross-trait calls
* `parse_args::parse_args(args)` — CLI argument parser trait
* `serve::start_server(config, port)` — HTTP server startup

### State

reads:
* globals (REGISTRY, CONFIG, HANDLES, TRAITS_DIR) — accessed by individual trait implementations

writes:
* globals may be mutated by individual traits

---

## CallConfig

### Purpose

Carries per-call configuration: which interfaces/traits to redirect, and parameter defaults for those redirects.

### Fields

* interface_overrides: map of interface path (contains `/`) to implementation trait path, e.g. `{ "llm/prompt": "net.openai" }`
* trait_overrides: map of trait path (contains `.`) to redirect target, e.g. `{ "net.copilot_chat": "net.openai" }`
* load_params: map of override key to param name/value defaults, e.g. `{ "llm/prompt": { "model": String("gpt-5") } }`

### new

Creates a CallConfig from explicit interface and trait override maps. load_params starts empty.

### with_base

Merges another CallConfig as a base layer. Self (per-call overrides) takes priority over base (persistent trait-level config). Used to layer a trait's `[load]` TOML section under per-call overrides.

---

## RouterError

### Variants

* NotFound(path) — trait path not in registry
* ArgCount { expected, got } — wrong number of arguments vs signature
* TypeMismatch { name, expected, got } — argument doesn't match parameter type
* WorkerError(message) — execution error from dispatch layer
* Timeout(seconds) — trait call exceeded timeout
* HandleError(message) — handle protocol error (missing handle, cross-language, etc.)

---

## Dispatcher

### Purpose

Resolves trait paths through multiple layers (overrides, bindings, interfaces, imports), validates and coerces arguments, enforces handle constraints, and executes traits directly via compiled modules. Eliminates the former WorkerManager intermediary.

### Fields

* registry: Registry — trait definition lookup
* timeout: u64 — max seconds per trait call

---

## Dispatcher::new

### Purpose

Construct a new Dispatcher.

### Inputs

* registry: Registry
* timeout: u64

### Outputs

* Dispatcher instance

### State

reads: none
writes: none

### Side Effects

none

### Dependencies

none

### Flow

1. Store both fields

### Edge Cases

none

### Example

```
Dispatcher::new(registry, 30)
```

---

## Dispatcher::extract_load_config (private, static)

### Purpose

Extracts inline `{ load: { ... } }` configuration from the last argument of a trait call. Supports simple form (`"key": "target"`) and object form (`"key": { "impl": "target", "model": "gpt-5" }`).

### Inputs

* args: Vec<TraitValue> — the call arguments
* config: &CallConfig — existing config to merge into

### Outputs

* (Vec<TraitValue>, CallConfig) — cleaned args (load object removed) and merged config

### State

reads: none
writes: none

### Side Effects

none

### Dependencies

none

### Flow

1. Check if last arg is a Map containing a "load" key with a Map value
2. If not, return args and config unchanged
3. For each key/value in the load map:
   a. If value is String: add to interface_overrides (key contains `/`) or trait_overrides (key contains `.`)
   b. If value is Map with "impl" key: extract target, add to overrides, collect remaining keys as load_params
4. Remove the load object from args:
   a. If the last arg Map has only the "load" key, pop the entire arg
   b. Otherwise, just remove the "load" key from the Map
5. Return cleaned args and merged config

### Edge Cases

* Last arg is not a Map — returns unchanged
* Last arg Map has no "load" key — returns unchanged
* Load value is neither String nor Map — ignored
* Object form without "impl" key — ignored
* Last arg Map has other keys besides "load" — only "load" is removed, other keys preserved

### Example

```
// args = [String("hello"), Map({ "load": Map({ "llm/prompt": String("net.openai") }) })]
// → cleaned args = [String("hello")], config gains interface_overrides["llm/prompt"] = "net.openai"
```

---

## Dispatcher::apply_load_params (private)

### Purpose

Fills in or overrides positional arguments based on named parameter defaults from load_params config. Matches param names against the target trait's signature to find positional indices.

### Inputs

* source_key: &str — the override key to look up in config.load_params
* target_path: &str — the trait being called (to get its signature)
* args: Vec<TraitValue> — current arguments
* config: &CallConfig — config containing load_params

### Outputs

* Vec<TraitValue> — args with defaults applied

### State

reads:
* self.registry — to look up target trait's signature

writes: none

### Side Effects

none

### Dependencies

* self.registry.get(target_path) — target trait entry

### Flow

1. Look up load_params for source_key; if empty or missing, return args unchanged
2. Look up target trait's entry; if not found, return args unchanged
3. For each (param_name, param_val) in load_params:
   a. Find the positional index in the target's signature params matching param_name
   b. If index < args.len(), override that position
   c. If index >= args.len(), pad with Null up to that position, then push the value
4. Return modified args

### Edge Cases

* source_key not in load_params — no-op
* target_path not in registry — no-op
* param_name not in target signature — ignored
* Gap between current args length and target index — filled with Null

---

## Dispatcher::call

### Purpose

Main entry point for calling a trait. Handles the full pipeline: load config extraction, handle protocol routing, persistent load merging, trait-level overrides, interface/binding resolution, import resolution, argument validation, type coercion, and direct execution.

### Inputs

* path: &str — dot-notation trait path
* args: Vec<TraitValue> — call arguments
* config: &CallConfig — per-call configuration

### Outputs

* Ok(TraitValue) — result from the trait
* Err(RouterError) — any dispatch/validation/execution error

### State

reads:
* self.registry — trait definitions, bindings, interfaces
* self.timeout — call timeout

writes: none (but dispatched trait may mutate global state)

### Side Effects

* Executes the target trait (may have arbitrary side effects)
* Resolves and executes imports (dependency traits)
* May recurse via Box::pin for overrides, bindings, and imports

### Dependencies

* Self::extract_load_config — inline config extraction
* self.registry.get, get_binding, is_interface, resolve_with_bindings — trait resolution
* self.apply_load_params — param default injection
* self.resolve_imports — dependency resolution
* self.call_handle_method — handle protocol dispatch
* self.execute_trait — direct execution
* tokio::time::timeout — timeout enforcement

### Flow

1. Extract inline `{ load: {} }` from last arg
2. If path is a reserved handle method (`__release__`, `__inspect__`, etc.), delegate to call_handle_method
3. Merge persistent `[load]` from the trait's TOML definition (per-call overrides take priority)
4. Check trait_overrides for a redirect; if found, apply load_params and recurse into the redirect
5. Check bindings/interfaces; if path resolves to a different implementation, recurse into it
6. Resolve imports (transitive dependencies) with cycle detection
7. Look up trait in registry → NotFound error if missing
8. Validate argument count: args.len() must be between required_count and max_count
9. Check cross-language handle constraints: handles must match the target trait's language prefix
10. Coerce argument types to match signature:
    * non-string primitives → String (if param expects string)
    * String → List (JSON parse or comma-split)
    * String → Map (JSON parse)
    * String → Bool ("true"/"false"/"1"/"0"/"yes"/"no")
    * String → Int/Float (parse)
    * String → Handle ("hdl:..." prefix)
    * Int/Float/Bool/List/Map → String (stringify/JSON)
11. Validate argument types against signature (Null allowed for optional params)
12. Build WorkerRequest with UUID id
13. Execute trait directly via execute_trait with timeout
14. Return result or error from WorkerResponse

### Edge Cases

* Reserved handle methods bypass all trait resolution
* Trait has persistent `[load]` in TOML — merged as base under per-call config
* Override chains: trait A overrides to B, B may override to C (recursive Box::pin)
* Binding and interface resolution only if registry reports path has bindings or is an interface
* Import cycles — handled by visited set in resolve_imports
* Type coercion: List/Map → String via serde_json, String → List tries JSON first then comma-split
* Handle argument in wrong language → HandleError with instructions to use __export__

### Example

```
// Simple call
dispatcher.call("sys.checksum", vec![TraitValue::String("hash".into()), TraitValue::String("hello".into())], &config).await

// Call with inline load override
dispatcher.call("net.chat", vec![
    TraitValue::String("hello".into()),
    TraitValue::Map(HashMap::from([
        ("load".into(), TraitValue::Map(HashMap::from([
            ("llm/prompt".into(), TraitValue::String("net.openai".into()))
        ])))
    ]))
], &config).await
```

---

## Dispatcher::call_stream

### Purpose

Start a streaming trait call. Validates args and dispatches; chunks flow through the mpsc sender channel.

### Inputs

* path: &str — trait path
* args: Vec<TraitValue> — call arguments
* stream_tx: mpsc::Sender<TraitValue> — channel for streaming chunks
* config: &CallConfig — per-call config

### Outputs

* Ok(()) — stream started
* Err(RouterError) — routing/validation error

### State

reads:
* self.registry

writes: none

### Side Effects

* Sends chunks to stream_tx as they arrive
* Drops sender when stream ends (closes receiver)

### Dependencies

* Self::extract_load_config
* self.resolve_imports
* self.execute_rust — direct execution

### Flow

1. Extract inline load config
2. Merge persistent `[load]` from trait TOML
3. Check trait_overrides; if redirect, recurse via Box::pin
4. Resolve imports
5. Look up trait → NotFound if missing
6. Validate argument count
7. Build streaming WorkerRequest (stream: true)
8. For Rust traits: execute directly, send single result as one chunk
9. For non-Rust traits: return WorkerError (this build only supports Rust)

### Edge Cases

* No type coercion (unlike call) — streaming path skips coercion step
* No handle constraint checks in streaming path
* For Rust traits, falls back to single-shot (sends one chunk then drops tx)

---

## Dispatcher::call_handle_method (private)

### Purpose

Routes handle protocol methods to the correct execution path based on the handle's language prefix.

### Inputs

* method: &str — one of `__release__`, `__inspect__`, `__export__`, `__handles__`, `__log__`, `__stop__`
* args: Vec<TraitValue> — first arg is usually the handle (except `__handles__`)

### Outputs

* Ok(TraitValue) — result from execution
* Err(RouterError) — handle error

### State

reads: none directly (delegates to dispatch_to_language)

writes: none directly

### Side Effects

* `__release__` removes a handle from global state
* `__inspect__` reads handle metadata
* `__export__` serializes handle state
* `__handles__` lists all handles for a language

### Dependencies

* handle_prefix_to_language — map prefix string to Language enum
* self.dispatch_to_language — route to language-specific execution

### Flow

1. If method is `__handles__`:
   a. First arg must be a language string ("python", "rust", etc.)
   b. Parse to Language enum
   c. Dispatch to that language's handler with empty args
2. For all other methods:
   a. First arg must be a handle value
   b. Extract handle's language prefix (e.g. "py", "rs")
   c. Map prefix to Language enum
   d. Dispatch to that language's handler

### Edge Cases

* `__handles__` requires a language string, not a handle
* Missing first arg → HandleError
* Non-handle first arg (for release/inspect/export) → HandleError
* Unknown language prefix → HandleError

---

## Dispatcher::dispatch_to_language (private)

### Purpose

Dispatches a WorkerRequest to a language-specific execution path. For Rust, executes handle methods directly. Other languages return an error in this build.

### Inputs

* language: &Language — target language
* request: WorkerRequest

### Outputs

* Ok(TraitValue) — result
* Err(RouterError) — timeout, worker error, or unsupported language

### State

reads: self.timeout
writes: none

### Side Effects

* Executes the request

### Dependencies

* self.execute_rust_handle_method
* tokio::time::timeout

### Flow

1. If language is Rust:
   a. Wrap execute_rust_handle_method in timeout
   b. Return result or error
2. Otherwise: return WorkerError (unsupported language)

---

## Dispatcher::resolve_imports (private, async recursive)

### Purpose

Recursively resolves and executes import dependencies for a trait before calling it. Uses a visited set for cycle/diamond detection.

### Inputs

* path: &str — trait to resolve imports for
* visited: &mut HashSet<String> — already-processed paths

### Outputs

* Ok(()) — all imports resolved
* Err(RouterError) — import not found or failed

### State

reads:
* self.registry — trait entries and their imports lists

writes:
* visited set (passed by reference)

### Side Effects

* Executes each import trait with empty args (side effects depend on imports)

### Dependencies

* self.registry.get
* self.execute_trait
* tokio::time::timeout

### Flow

1. If path already in visited, return Ok (cycle guard)
2. Add path to visited
3. Look up trait entry; if not found, return Ok (will fail later in call)
4. If no imports, return Ok
5. For each import_path:
   a. Recursively resolve that import's own imports (Box::pin)
   b. Look up import entry → error if not found
   c. Execute with empty args and timeout
   d. Check for error in response
   e. Log success

### Edge Cases

* Cyclic imports — visited set prevents infinite recursion
* Diamond dependencies — visited set prevents duplicate execution
* Import not in registry → WorkerError with descriptive message
* Import execution fails → WorkerError propagated

---

## Dispatcher::registry

### Purpose

Accessor for the dispatcher's registry reference.

### Inputs

none

### Outputs

* &Registry

---

## Dispatcher::execute_trait (private)

### Purpose

Routes a trait call to the appropriate language execution path. Currently only supports Rust.

### Inputs

* trait_entry: &TraitEntry — the resolved trait definition
* request: WorkerRequest — the call request

### Outputs

* Ok(WorkerResponse) — execution result
* Err — unsupported language

### State

reads: trait_entry.language
writes: none

### Side Effects

* Executes the trait

### Dependencies

* self.execute_rust

### Flow

1. If language is Rust, delegate to execute_rust
2. Otherwise, return error (only Rust supported in this build)

---

## Dispatcher::execute_rust (private)

### Purpose

Executes a Rust trait in-process through the compiled dispatch module.

### Inputs

* trait_entry: &TraitEntry — trait definition
* request: WorkerRequest — call request with args

### Outputs

* Ok(WorkerResponse) — result from compiled module
* Err — only for sys.serve async failure

### State

reads:
* compiled::dispatch_trait_value — compiled trait lookup

writes: none (but traits may mutate globals)

### Side Effects

* Executes the trait implementation (arbitrary side effects)
* sys.serve starts an HTTP server

### Dependencies

* self.execute_serve — special-case for sys.serve
* compiled::dispatch_trait_value — compiled module dispatch

### Flow

1. If path is "sys.serve", delegate to execute_serve (needs async)
2. Call compiled::dispatch_trait_value(path, args)
3. If Some(result), return success WorkerResponse
4. If None, return error "No Rust implementation for {path}"

### Edge Cases

* sys.serve needs special async handling — cannot use sync dispatch
* Trait path not in compiled modules → returns error (not panic)

---

## Dispatcher::execute_serve (private)

### Purpose

Starts the HTTP server for sys.serve trait dispatch. Special-cased because it requires async runtime.

### Inputs

* request: &WorkerRequest — contains port argument

### Outputs

* Ok(WorkerResponse) — server started successfully
* Err — server error

### State

reads:
* globals::CONFIG — server configuration
* request.args[0] — port override (Int)

writes: none

### Side Effects

* Starts an HTTP server on the specified port
* Blocks until server shuts down

### Dependencies

* crate::globals::CONFIG
* compiled::serve::start_server

### Flow

1. Extract port from first arg (Int), or fall back to config default
2. Get config from globals (error if not available)
3. Call compiled::serve::start_server(config, port).await
4. Return success Map({ "ok": true }) or error

---

## Dispatcher::execute_rust_handle_method (private)

### Purpose

Handles reserved protocol methods for Rust handles (__release__, __inspect__, __handles__) using the global HANDLES state.

### Inputs

* request: &WorkerRequest — method name and args

### Outputs

* Ok(WorkerResponse) — handle operation result

### State

reads:
* globals::HANDLES — global handle storage

writes:
* HANDLES — __release__ removes entries

### Side Effects

* __release__: removes handle from global state
* __inspect__: reads handle metadata (no mutation)
* __handles__: lists all handles (no mutation)

### Dependencies

* crate::globals::HANDLES
* crate::globals::now_epoch

### Flow

For __release__:
1. Extract handle_id from first arg
2. Lock HANDLES, remove by id
3. Return Bool(true/false)

For __inspect__:
1. Extract handle_id from first arg
2. Lock HANDLES, look up entry
3. Return Map with id, type, summary, created, age_seconds
4. Or error "Invalid handle" if not found

For __handles__:
1. Lock HANDLES
2. Return List of Maps with id, type, summary, created for each handle

For unknown methods:
1. Return error "Unknown handle method: {method}"

### Edge Cases

* HANDLES not initialized → error "Handles not initialized"
* Missing handle arg → error with method name
* Handle not found (for inspect) → error "Invalid handle: {id}"

---

## Dispatcher::shutdown

### Purpose

Graceful shutdown. No-op for the Rust-only build.

---

## handle_prefix_to_language (private, free function)

### Purpose

Maps a 2-4 char handle language prefix to a Language enum variant.

### Inputs

* prefix: &str — one of "py", "js", "ts", "java", "perl", "rs", "cl"

### Outputs

* Some(Language) if known prefix
* None if unknown

---

## language_to_handle_prefix (private, free function)

### Purpose

Maps a Language enum to its handle prefix string.

### Inputs

* lang: &Language

### Outputs

* &'static str — "py", "js", "ts", "java", "perl", "rs", "cl"

---

## HANDLE_METHODS (constant)

Reserved handle protocol method names: `__release__`, `__inspect__`, `__export__`, `__handles__`, `__log__`, `__stop__`.

---

## Internal Structure

The Dispatcher merges what were previously two separate concerns:

**Former Router layer** (resolution + validation):
```
call(path, args, config)
  → extract_load_config (inline { load: {} })
  → handle protocol check (HANDLE_METHODS → call_handle_method)
  → merge persistent [load] from trait TOML
  → trait_overrides redirect (recursive call)
  → binding/interface resolution (recursive call)
  → resolve_imports (recursive, cycle-safe)
  → registry lookup + arg count validation
  → cross-language handle check
  → type coercion (string↔primitives, JSON parse)
  → type validation
```

**Former WorkerManager layer** (execution):
```
  → execute_trait (language routing)
    → execute_rust (compiled dispatch)
      → compiled::dispatch_trait_value (in-process call)
    → execute_serve (special-case: HTTP server)
  → execute_rust_handle_method (handle protocol on globals)
```

CallConfig flows through the entire pipeline, layered: per-call overrides > inline load > persistent TOML load.

The compiled module (`pub mod compiled`) is included from build.rs output and provides the dispatch gateway to all compiled Rust traits.

## Notes

* Type coercion is intentionally one-directional per type (e.g. CLI passes numbers as strings, dispatcher coerces to match signature). This is the main system boundary where external input is normalized.
* The `call_stream` path is lighter — no coercion or handle validation. This may be intentional (streaming is for trusted internal use) or a gap (`?`).
* resolve_imports calls each import with empty args — imports are expected to be setup/initialization traits that work without arguments.
* The `Box::pin` usage in recursive async calls (overrides, imports) is required because Rust async functions can't be directly recursive.
* The former WorkerManager had 3 no-op methods (set_registry, set_config, shutdown) and 1 dead field (traits_dir) — these were eliminated in the merge.
* This build only supports Rust traits. Non-Rust languages return an explicit error rather than silently failing.
