# registry

## Purpose

Thin wrapper over the Registry read API. Provides list, info, tree, namespace listing, count, get, search, and namespace-filtered listing actions.

## Exports

* `registry(args)` — trait entry point dispatching on action string
* `list(args)` — list traits with optional namespace filter (called by sys.list delegate)
* `info(args)` — show trait details or namespace listing (called by sys.info delegate)

---

## registry

### Purpose

Query the trait registry with various actions.

### Inputs

* `args[0]`: action string — "list", "info", "tree", "namespaces", "count", "get", "search", or "namespace" (default: "tree")
* `args[1]`: additional argument string (required for "info", "get", "search", "namespace"; optional namespace filter for "list")

### Outputs

* Varies by action (see Flow)

### State

reads:
* `crate::globals::REGISTRY` — the trait registry

writes: none

### Side Effects

none

### Dependencies

* `crate::globals::REGISTRY`
* `Registry::all`, `Registry::len`, `Registry::get`
* `TraitEntry::to_json`

### Flow

1. Get REGISTRY global; error if not initialized
2. Extract action (default "tree") and arg2 (default "")
3. Match action:
   - "list": get all traits, optionally filter by namespace (arg2), sort by path, return array of summary JSON objects
   - "info": require arg2 as trait path; try exact match → return full `to_json()`; try namespace prefix match → return array of `{path, description, language}`; otherwise return error
   - "tree": get all traits, build nested JSON object by splitting paths on dots, each leaf holds full trait JSON
   - "namespaces": collect unique first-segment of all trait paths into sorted array
   - "count": return `Registry::len()` as integer
   - "get": require arg2 as trait path, return `entry.to_json()` or error if not found
   - "search": require arg2 as query, filter traits whose path or description contains query (case-insensitive), return array of {path, description}
   - "namespace": require arg2, filter traits starting with `"{arg2}."` or equal to arg2, return array of {path, description}
   - anything else: return error

### Edge Cases

* Missing arg2 for "get"/"search"/"namespace": returns error
* Empty search results: returns empty array
* "tree" builds nested objects — traits with deep paths create nested structure

### Example

```
sys.registry "list"
=> [{"path":"sys.checksum",...}, {"path":"sys.info",...}, ...]

sys.registry "list" "sys"
=> [{"path":"sys.checksum",...}, ...]

sys.registry "info" "sys.version"
=> {full trait JSON}

sys.registry "info" "sys"
=> [{"path":"sys.checksum","description":"...","language":"rust"}, ...]

sys.registry "count"
=> 13

sys.registry "namespaces"
=> ["sys"]

sys.registry "search" "version"
=> [{"path":"sys.version","description":"Generate trait versions: YYMMDD date format"}]

sys.registry "get" "sys.version"
=> {full trait JSON}

sys.registry "namespace" "sys"
=> [{path, description}, ...]
```

---

## list

### Purpose

List all registered traits with optional namespace filtering. Public entry point called by sys.list delegate.

### Inputs

* `args[0]`: namespace filter string (optional)

### Outputs

* JSON array of trait summary objects (from `to_summary_json()`)

### State

reads:
* `crate::globals::REGISTRY`

writes: none

### Side Effects

none

### Dependencies

* `crate::globals::REGISTRY`
* `Registry::all`
* `TraitEntry::to_summary_json`

### Flow

1. Get REGISTRY; error if not initialized
2. Extract optional namespace filter from args[0] (skip if empty)
3. Get all traits from registry
4. If namespace filter: retain traits where path starts with `"{ns}."` or equals ns
5. Sort by path alphabetically
6. Map each trait to `to_summary_json()`
7. Return as JSON array

### Edge Cases

* No namespace filter: returns all traits
* Empty string filter: treated as no filter

### Example

```
registry::list(&[])
=> [{"path":"sys.checksum",...}, ...]

registry::list(&[TraitValue::String("sys".into())])
=> [{"path":"sys.checksum",...}, ...]
```

---

## info

### Purpose

Show detailed information about a specific trait, or list traits in a namespace. Public entry point called by sys.info delegate.

### Inputs

* `args[0]`: trait path or namespace string (required)

### Outputs

* JSON: full trait object for exact match
* JSON array of `{path, description, language}` for namespace match
* JSON: `{"error": "..."}` if not found

### State

reads:
* `crate::globals::REGISTRY`

writes: none

### Side Effects

none

### Dependencies

* `crate::globals::REGISTRY`
* `Registry::get`, `Registry::all`
* `TraitEntry::to_json`
* `Language::to_string`

### Flow

1. Get REGISTRY; error if not initialized
2. Extract path from args[0]; error if missing
3. Try exact trait match: `registry.get(path)` → return `to_json()`
4. Try namespace match: filter traits with prefix `"{path}."` or equal to path
5. If namespace has traits: return array of `{path, description, language}`
6. Otherwise: return `{"error": "Trait not found: {path}"}`

### Edge Cases

* Exact trait match takes priority over namespace
* Path that matches neither trait nor namespace: returns error
* No args: returns error "path argument required"

### Example

```
registry::info(&[TraitValue::String("sys.version".into())])
=> {full trait JSON}

registry::info(&[TraitValue::String("sys".into())])
=> [{"path":"sys.checksum","description":"...","language":"rust"}, ...]
```

---

## Internal Structure

Single function with match dispatch plus two public entry-point functions (`list` and `info`) that can be called directly by delegate traits. The "tree" action builds a nested JSON object by walking trait paths; all others are flat queries.

## Notes

* Search is case-insensitive (lowercases both query and target)
* Namespace filter matches both prefix "ns." and exact "ns"
* `list` and `info` are also accessible as actions via `sys.registry "list"` / `sys.registry "info"`
* sys.list and sys.info are thin delegates that call registry::list and registry::info directly
