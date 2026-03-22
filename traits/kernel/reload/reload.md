# reload

## Purpose

Reload the trait registry from disk. Re-scans the traits directory and updates the in-memory registry.

## Exports

* `reload(args)` — trait entry point, triggers registry reload

---

## reload

### Purpose

Reload all trait definitions from the configured traits directory.

### Inputs

* `_args`: ignored

### Outputs

* JSON: `{"ok": true, "count": N}` on success (N = number of traits loaded)
* JSON: `{"error": "..."}` on failure

### State

reads:
* `crate::globals::REGISTRY` — the registry instance to reload
* `crate::globals::TRAITS_DIR` — the directory path to scan

writes:
* `crate::globals::REGISTRY` — internal state updated via `load_from_dir`

### Side Effects

* Filesystem: reads trait definition files
* Updates in-memory registry state

### Dependencies

* `crate::globals::REGISTRY`
* `crate::globals::TRAITS_DIR`
* `Registry::load_from_dir`

### Flow

1. Get REGISTRY global; error if not initialized
2. Get TRAITS_DIR global; error if not configured
3. Call `registry.load_from_dir(traits_dir)`
4. On success: return `{"ok": true, "count": N}`
5. On error: return `{"error": "Reload failed: {message}"}`

### Edge Cases

* REGISTRY not initialized: returns error
* TRAITS_DIR not set: returns error
* Invalid trait files: load_from_dir may return error or skip them

### Example

```
sys.reload
=> {"ok": true, "count": 13}
```

---

## Internal Structure

Thin wrapper around `Registry::load_from_dir`. No local state.

## Notes

none
