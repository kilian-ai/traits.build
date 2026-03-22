# list

## Purpose

List all registered traits with optional namespace filtering. Thin delegate to `crate::dispatcher::compiled::registry::list(args)`.

## Exports

* `list(args)` — trait entry point

---

## list

### Purpose

Return a sorted array of trait summaries, optionally filtered by namespace. Delegates entirely to `registry::list(args)`.

### Inputs

* `args[0]`: namespace filter string (optional)

### Outputs

* JSON array of trait summary objects (from `to_summary_json()`)

### State

reads:
* `crate::globals::REGISTRY` (via registry::list)

writes: none

### Side Effects

none

### Dependencies

* `crate::dispatcher::compiled::registry::list`

### Flow

1. Forward args to `crate::dispatcher::compiled::registry::list(args)`
2. Return the result

### Edge Cases

* No namespace filter: returns all traits
* Empty string filter: treated as no filter
* Namespace with no traits: returns empty array

### Example

```
sys.list
=> [{"path":"sys.checksum",...}, {"path":"sys.info",...}, ...]

sys.list "sys"
=> [{"path":"sys.checksum",...}, ...]
```

---

## Internal Structure

Single function that delegates to registry::list.

## Notes

* All filtering, sorting, and mapping logic lives in registry.rs
