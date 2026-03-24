# dylib_loader

## Purpose

Dynamic shared-library loader for trait dylibs. Scans directories for `.dylib` (macOS) and `.so` (Linux) files, loads them via `libloading`, extracts C ABI symbols (`trait_call`, `trait_free`, optional `trait_init`), and dispatches calls by trait path. Supports two discovery modes: TOML-based (reads `.trait.toml` with `source = "dylib"`) and filename convention (`lib{ns}_{name}.dylib`). Supports single-trait reload and full reload.

## Exports

* `DylibLoader` — registry of loaded trait dylibs; scans dirs, loads, dispatches, reloads
* `LOADER` — `OnceLock<Arc<DylibLoader>>` global reference used by the dispatch callback
* `set_global_loader(loader)` — sets the `LOADER` global once at startup

---

## Type Aliases

### TraitCallFn

`unsafe extern "C" fn(*const u8, usize, *mut usize) -> *mut u8`

Called with (json_ptr, json_len, out_len_ptr), returns pointer to result JSON bytes.

### TraitFreeFn

`unsafe extern "C" fn(*mut u8, usize)`

Frees a buffer previously returned by `trait_call`.

### DispatchFn

`unsafe extern "C" fn(*const u8, usize, *mut usize) -> *mut u8`

Same signature as TraitCallFn. Passed to dylibs so they can call other traits.

### TraitInitFn

`unsafe extern "C" fn(DispatchFn)`

Optional init function. Receives the server dispatch callback.

---

## LoadedTrait (struct, private)

### Purpose

Holds a loaded dylib along with its resolved function pointers.

### Fields

* `_lib`: `libloading::Library` — keeps the dylib alive; symbols are only valid while this exists
* `call`: `TraitCallFn` — resolved `trait_call` symbol
* `free`: `TraitFreeFn` — resolved `trait_free` symbol
* `path`: `String` — trait path (e.g. "sys.checksum")
* `dylib_path`: `PathBuf` — filesystem path to the `.dylib`/`.so` file

### State

reads: none
writes: none (immutable after construction)

### Notes

* `Send` + `Sync` implemented via unsafe impl — safe because Library owns the symbols and is never dropped while pointers are live.

---

## DylibLoader

### Purpose

Registry of loaded trait dylibs. Provides scan, load, dispatch, reload, list, and has operations.

### Fields

* `traits`: `Arc<RwLock<HashMap<String, LoadedTrait>>>` — loaded dylibs indexed by trait path
* `search_dirs`: `Vec<PathBuf>` — directories to scan for dylibs

---

## DylibLoader::new

### Purpose

Create a new DylibLoader with the given search directories.

### Inputs

* `search_dirs`: list of directory paths to scan

### Outputs

* `DylibLoader` instance with empty traits map

### State

reads: none
writes: initializes traits map as empty

### Side Effects

none

### Dependencies

none

### Flow

1. Create empty `HashMap` wrapped in `Arc<RwLock>`
2. Store search_dirs
3. Return new DylibLoader

### Edge Cases

* Empty search_dirs is valid — load_all will return 0

### Example

```rust
let loader = DylibLoader::new(vec![PathBuf::from("./dylibs")]);
```

---

## DylibLoader::load_all

### Purpose

Scan all search directories recursively and load every `.dylib`/`.so` found.

### Inputs

none (uses self.search_dirs)

### Outputs

* `usize` — number of dylibs successfully loaded

### State

reads: self.search_dirs
writes: self.traits (inserts loaded dylibs)

### Side Effects

* Filesystem: reads directory entries
* Logging: info on success, warn on failure

### Dependencies

* `scan_dir_recursive`

### Flow

1. For each dir in search_dirs
2. Skip if dir doesn't exist
3. Call `scan_dir_recursive(dir)` and accumulate count
4. Return total count

### Edge Cases

* Non-existent directories are silently skipped

---

## DylibLoader::scan_dir_recursive

### Purpose

Recursively scan a single directory for `.trait.toml` files with `source = "dylib"` (TOML discovery, preferred) and standalone dylib files (filename convention). TOML discovery runs first so dylibs already loaded via TOML are skipped by the filename scanner.

### Inputs

* `dir`: directory path to scan

### Outputs

* `usize` — number loaded from this directory tree

### State

reads: filesystem
writes: self.traits via try_load_toml_dylib and load_dylib

### Side Effects

* Filesystem read
* Logging

### Dependencies

* `is_dylib`
* `try_load_toml_dylib`
* `load_dylib`

### Flow

1. Read directory entries; warn and return 0 on error
2. Collect into three buckets: directories, .trait.toml files, .dylib/.so files
3. Recurse into subdirectories
4. Mode 2 first: process .trait.toml files via `try_load_toml_dylib`
5. Mode 1: process standalone .dylib/.so files via `load_dylib`, skipping any already loaded by TOML discovery
6. Return count
4. Return count

### Edge Cases

* Unreadable directories log a warning and return 0
* Non-dylib files are silently skipped

---

## DylibLoader::load_dylib

### Purpose

Load a single dylib file by filename convention, resolve its C ABI symbols, and register it. Derives trait path from the filename and delegates to `load_dylib_with_path`.

### Inputs

* `dylib_path`: path to the `.dylib`/`.so` file

### Outputs

* `Result<String, String>` — trait path on success, error message on failure

### State

reads: none
writes: self.traits (via load_dylib_with_path)

### Side Effects

* dlopen: loads shared library into process

### Dependencies

* `dylib_name_to_trait_path`
* `load_dylib_with_path`

### Flow

1. Derive trait path from filename via `dylib_name_to_trait_path`
2. Delegate to `load_dylib_with_path(dylib_path, trait_path)`

### Edge Cases

* File without `lib` prefix returns error from `dylib_name_to_trait_path`

---

## DylibLoader::load_dylib_with_path

### Purpose

Core dylib loading: opens the shared library, resolves C ABI symbols, optionally calls `trait_init`, and registers the trait under the given path.

### Inputs

* `dylib_path`: path to the `.dylib`/`.so` file
* `trait_path`: the dot-notation path to register this trait under

### Outputs

* `Result<String, String>` — trait path on success, error message on failure

### State

reads: none
writes: self.traits (inserts new LoadedTrait)

### Side Effects

* dlopen: loads shared library into process
* Calls `trait_init(server_dispatch)` if the symbol exists

### Dependencies

* `libloading::Library`
* `server_dispatch`

### Flow

1. dlopen the library
2. Resolve `trait_call` symbol — error if missing
3. Resolve `trait_free` symbol — error if missing
4. Check if `trait_init` exists; if so, resolve and call with `server_dispatch`
5. Build `LoadedTrait` struct
6. Insert into `self.traits` map
7. Return trait path

### Edge Cases

* Missing required symbols return descriptive error
* Poisoned RwLock returns error

---

## DylibLoader::try_load_toml_dylib

### Purpose

Try to load a dylib based on a `.trait.toml` file that declares `source = "dylib"`. Finds the companion `.dylib`/`.so` file in the same directory and derives the trait path from the filesystem structure.

### Inputs

* `toml_path`: path to the `.trait.toml` file

### Outputs

* `Option<usize>` — `Some(1)` if loaded, `None` if not a dylib trait or loading failed

### State

reads: filesystem, self.search_dirs
writes: self.traits (via load_dylib_with_path)

### Side Effects

* Filesystem read (TOML content, companion dylib lookup)
* dlopen on success

### Dependencies

* `derive_trait_path`
* `load_dylib_with_path`

### Flow

1. Read TOML file content
2. Check if any line contains `source` + `"dylib"` — return None if not
3. Get the parent directory and directory name
4. Look for companion dylib: `lib{dir_name}.{ext}` or `libtrait.{ext}`
5. Derive trait path from filesystem structure via `derive_trait_path`
6. Call `load_dylib_with_path` with found dylib and derived path
7. Return `Some(1)` on success, `None` on failure

### Edge Cases

* Non-dylib TOML files return None immediately (fast path)
* Missing companion dylib returns None
* Cannot derive trait path returns None

### Example

For `traits/www/traits/build/build.trait.toml` with `source = "dylib"`:
1. Finds `traits/www/traits/build/libbuild.dylib`
2. Derives path `www.traits.build` from `www/traits/build` relative to search dir
3. Loads and registers as `www.traits.build`

---

## DylibLoader::derive_trait_path

### Purpose

Derive a dot-notation trait path from a directory by finding its position relative to a search directory.

### Inputs

* `dir`: directory path containing the trait dylib

### Outputs

* `Option<String>` — dot-notation path, or None if dir isn't under any search dir

### State

reads: self.search_dirs

### Side Effects

none

### Dependencies

none

### Flow

1. For each search_dir, try to strip it as prefix from dir
2. Split remaining components on `/`
3. Join with `.` to form trait path
4. Return first match

### Edge Cases

* Dir not under any search dir returns None
* Empty relative path returns None

### Example

```rust
// search_dir = "./traits", dir = "./traits/www/traits/build"
// → Some("www.traits.build")
```

---

## DylibLoader::dispatch

### Purpose

Call a loaded dylib trait by path with JSON args.

### Inputs

* `trait_path`: dot-notation path (e.g. "sys.checksum")
* `args`: slice of `serde_json::Value`

### Outputs

* `Option<Value>` — `Some(result)` if trait found, `None` if not loaded

### State

reads: self.traits

### Side Effects

* Calls into native dylib code via FFI

### Dependencies

* `serde_json::to_vec`, `serde_json::from_slice`

### Flow

1. Acquire read lock on traits map
2. Look up trait_path — return None if not found
3. Serialize args to JSON bytes
4. Call `trait_call(ptr, len, &mut out_len)` via FFI
5. If null or zero length, return `Some(Value::Null)`
6. Read result bytes from pointer
7. Deserialize JSON — fallback to `Value::Null` on parse error
8. Free the dylib-allocated buffer via `trait_free`
9. Return `Some(result)`

### Edge Cases

* Null result pointer returns `Value::Null`
* Zero out_len returns `Value::Null`
* Malformed JSON from dylib returns `Value::Null`

---

## DylibLoader::reload

### Purpose

Reload a single trait by path. Drops the old library, re-loads from same path.

### Inputs

* `trait_path`: trait to reload

### Outputs

* `Result<(), String>` — Ok on success, error if not found or load fails

### State

reads: self.traits (to get dylib_path)
writes: self.traits (remove old, insert new)

### Side Effects

* dlclose old library, dlopen new

### Dependencies

* `load_dylib`

### Flow

1. Look up existing dylib_path for this trait
2. If not found, return error
3. Remove old entry from map (drops Library, closes dylib)
4. Re-load from same path via `load_dylib`
5. Log success

### Edge Cases

* Non-loaded trait returns error

---

## DylibLoader::reload_all

### Purpose

Clear all loaded dylibs and re-scan all search directories.

### Inputs

none

### Outputs

* `usize` — number of dylibs loaded after re-scan

### State

writes: self.traits (clears then repopulates)

### Side Effects

* dlclose all, then dlopen all found

### Dependencies

* `load_all`

### Flow

1. Snapshot current paths (unused but captured)
2. Clear traits map
3. Call `load_all()` and return count

### Edge Cases

none

---

## DylibLoader::list

### Purpose

Return sorted list of all loaded dylib trait paths.

### Inputs

none

### Outputs

* `Vec<String>` — sorted trait paths

### State

reads: self.traits

### Side Effects

none

### Dependencies

none

### Flow

1. Read lock traits map
2. Collect keys, sort, return

### Edge Cases

* Empty map returns empty vec

---

## DylibLoader::has

### Purpose

Check if a trait path is loaded as a dylib.

### Inputs

* `trait_path`: path to check

### Outputs

* `bool`

### State

reads: self.traits

### Side Effects

none

### Dependencies

none

### Flow

1. Read lock traits map
2. Return `contains_key(trait_path)`

### Edge Cases

none

---

## set_global_loader

### Purpose

Set the global `LOADER` OnceLock to the given `Arc<DylibLoader>`.

### Inputs

* `loader`: `Arc<DylibLoader>`

### Outputs

none

### State

writes: LOADER global

### Side Effects

none

### Dependencies

none

### Flow

1. Call `LOADER.set(loader)`, ignore if already set

### Edge Cases

* Second call is silently ignored

---

## server_dispatch

### Purpose

C ABI dispatch callback provided to dylibs via `trait_init`. Allows dylibs to call other traits.

### Inputs

* `json_ptr`: pointer to JSON bytes `{"path": "sys.x", "args": [...]}`
* `json_len`: length of input
* `out_len`: mutable pointer to write result length

### Outputs

* `*mut u8` — pointer to result JSON bytes (caller frees)

### State

reads: LOADER global

### Side Effects

* May call into other dylibs or compiled traits

### Dependencies

* `DylibLoader::dispatch`
* `crate::dispatcher::compiled::dispatch_compiled`

### Flow

1. Set `*out_len = 0`
2. Return null if input pointer is null or length is 0
3. Parse input JSON
4. Extract `path` and `args` fields
5. Try `LOADER.dispatch(path, args)` first
6. Fall back to `dispatch_compiled(path, args)`
7. If no loader, return error JSON
8. Serialize result to bytes
9. `mem::forget` the vec, write length to `out_len`, return pointer

### Edge Cases

* Null input returns null pointer
* Invalid JSON returns null pointer
* LOADER not set returns `{"error": "dispatch not initialized"}`
* Trait not found returns `{"error": "trait not found: <path>"}`

---

## is_dylib

### Purpose

Check if a filesystem path has a shared library extension.

### Inputs

* `path`: filesystem path

### Outputs

* `bool` — true if extension is "dylib" or "so"

### State

reads: none
writes: none

### Side Effects

none

### Dependencies

none

### Flow

1. Extract extension string
2. Match against "dylib" or "so"

### Edge Cases

* No extension returns false
* ".dll" returns false (Windows not supported)

### Example

```rust
assert!(is_dylib(Path::new("libfoo.dylib")));
assert!(!is_dylib(Path::new("foo.txt")));
```

---

## dylib_name_to_trait_path

### Purpose

Convert a dylib filename to a trait path. Strips "lib" prefix, converts first underscore to dot.

### Inputs

* `path`: filesystem path to dylib

### Outputs

* `Option<String>` — trait path or None if format doesn't match

### State

reads: none
writes: none

### Side Effects

none

### Dependencies

none

### Flow

1. Get file stem
2. Strip "lib" prefix — return None if missing
3. Find first underscore position — return None if missing
4. Split into namespace (before underscore) and name (after underscore)
5. Return `"{namespace}.{name}"`

### Edge Cases

* No "lib" prefix returns None
* No underscore returns None
* Multiple underscores: only first becomes dot, rest stay as underscores

### Example

```rust
// "libsys_checksum.dylib" -> Some("sys.checksum")
// "libsys_chain_anchor.dylib" -> Some("sys.chain_anchor")
// "not_a_lib.dylib" -> None
```

---

## Internal Structure

`DylibLoader` owns a thread-safe `HashMap` of `LoadedTrait` entries. Each entry keeps a `libloading::Library` alive to ensure function pointers remain valid. The `LOADER` global enables the `server_dispatch` C ABI callback, which dylibs use for cross-trait calls. Dispatch tries dylibs first, then falls back to compiled-in traits.

## Notes

* **Two discovery modes**: TOML-based (`source = "dylib"` in `.trait.toml`) is preferred; filename convention (`lib{namespace}_{name}.dylib`) is legacy fallback
* **TOML discovery** runs first in each directory so filename convention skips already-loaded dylibs
* **Companion dylib naming**: TOML mode looks for `lib{dir_name}.{ext}` or `libtrait.{ext}` in the same directory
* **Trait path derivation**: TOML mode derives path from filesystem structure relative to search dirs (e.g., `www/traits/build` → `www.traits.build`)
* Filename convention: `lib{namespace}_{name}.{dylib|so}` — only the first underscore is the namespace separator
* `server_dispatch` uses `mem::forget` on the result Vec to transfer ownership to the caller (dylib frees it)
* `Send`/`Sync` for `LoadedTrait` relies on the invariant that the Library outlives the function pointers
* The `plugin_api` crate in `traits/kernel/plugin_api/` provides the `export_trait!` macro for authoring cdylib plugins
