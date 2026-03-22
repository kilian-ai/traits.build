# call.rs

## Purpose

Meta-dispatch trait that calls another trait by its dot-notation path, forwarding arguments. Acts as a runtime indirection layer allowing dynamic trait invocation.

## Exports

* `call(args: &[Value]) -> Value` — dispatches to a trait identified by dot-notation path

---

## call

### Purpose

Resolves a trait by dot-notation path (e.g. `sys.checksum`) and invokes it with forwarded arguments.

### Inputs

* args[0] (trait_path): dot-notation string identifying the target trait, e.g. `"sys.checksum"`, `"sys.list"`
* args[1] (call_args): optional JSON array of arguments forwarded to the target trait; defaults to empty array `[]`

### Outputs

* On success: the return value from the dispatched trait (any JSON value)
* On empty/missing trait_path: `{ "error": "trait_path is required" }`
* On unknown trait: `{ "error": "Trait '{path}' not found" }`

### State

reads:

* global compiled trait registry (via `crate::worker::compiled::dispatch`)

writes:

* none (side effects depend on the dispatched trait)

### Side Effects

* Executes the target trait, which may have its own side effects (I/O, state mutation, etc.)

### Dependencies

* `serde_json::Value` — JSON value type
* `crate::worker::compiled::dispatch(trait_path, &args)` — internal compiled trait dispatcher

### Flow

1. Extract `trait_path` from `args[0]` as string; default to empty string if missing or not a string
2. Extract `call_args` from `args[1]` as array; default to empty array if missing or not an array
3. If `trait_path` is empty, return error object `{ "error": "trait_path is required" }`
4. Call `compiled::dispatch(trait_path, &call_args)`
5. If dispatch returns `Some(value)`, return the value
6. If dispatch returns `None`, return error object `{ "error": "Trait '{trait_path}' not found" }`

### Edge Cases

* Missing args entirely: trait_path defaults to `""`, triggers required error
* args[0] is non-string (number, object, etc.): treated as missing, triggers required error
* args[1] is non-array (string, object, etc.): treated as missing, defaults to `[]`
* args[1] omitted: target trait called with no arguments
* Dispatched trait itself returns an error object: returned as-is (no wrapping)

### Example

```
# Call sys.checksum with one argument
traits call sys.call sys.checksum '["hello"]'
# → { "md5": "5d41402abc4b2a76b9719d911017c592", ... }

# Missing trait_path
traits call sys.call
# → { "error": "trait_path is required" }

# Unknown trait
traits call sys.call nonexistent.trait
# → { "error": "Trait 'nonexistent.trait' not found" }

# Call with no args forwarded
traits call sys.call sys.list
# → [ ... list of traits ... ]
```

---

## Internal Structure

Single function module. No internal helpers or state. Delegates entirely to the compiled dispatch layer. The function is the trait entry point registered via `call.trait.toml` with `entry = "call"`.

## Notes

* The `clone()` on `call_args` is necessary because `as_array()` returns a reference; the owned vec is needed for the dispatch call.
* The trait acts as a trampoline — it adds no logic beyond argument extraction and error wrapping around the existing dispatch mechanism.
