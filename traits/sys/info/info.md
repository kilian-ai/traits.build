# info

## Purpose

Show detailed information about a specific trait, or list traits in a namespace if the path matches a namespace prefix. Thin delegate to `crate::dispatcher::compiled::registry::info(args)`.

## Exports

* `info(args)` — trait entry point

---

## info

### Purpose

Return full trait details for a specific trait path, or a list of traits if the path is a namespace. Delegates entirely to `registry::info(args)`.

### Inputs

* `args[0]`: trait path or namespace string (required)

### Outputs

* JSON: full trait object (from `to_json()`) for exact match
* JSON array of `{path, description, language}` for namespace match
* JSON: `{"error": "..."}` if not found

### State

reads:
* `crate::globals::REGISTRY` (via registry::info)

writes: none

### Side Effects

none

### Dependencies

* `crate::dispatcher::compiled::registry::info`

### Flow

1. Forward args to `crate::dispatcher::compiled::registry::info(args)`
2. Return the result

### Edge Cases

* Exact trait match takes priority over namespace
* Path that matches neither trait nor namespace: returns error
* No args: returns error "path argument required"

### Example

```
sys.info "sys.version"
=> {full trait JSON with path, description, language, signature, ...}

sys.info "sys"
=> [{"path":"sys.checksum","description":"...","language":"rust"}, ...]

sys.info "nonexistent"
=> {"error": "Trait not found: nonexistent"}
```

---

## Internal Structure

Single function that delegates to registry::info.

## Notes

* All lookup logic (exact match, namespace prefix, error) lives in registry.rs
