# plugin_api

## Purpose

Helper crate for authoring cdylib trait plugins. Provides the `export_trait!` macro that generates C ABI exports (`trait_call`, `trait_free`) wrapping a Rust function with signature `fn(&[serde_json::Value]) -> serde_json::Value`.

## Exports

* `export_trait!` — macro that generates the C ABI entry points for a trait function

---

## export_trait! (macro)

### Purpose

Generate `trait_call` and `trait_free` C ABI functions wrapping a trait handler function. This is the only thing a cdylib plugin needs to export.

### Inputs

* A path to a function with signature `fn(&[serde_json::Value]) -> serde_json::Value`

### Generated Symbols

* `trait_call(json_ptr: *const u8, json_len: usize, out_len: *mut usize) -> *mut u8` — receives JSON-serialized args, calls the handler, returns JSON-serialized result
* `trait_free(ptr: *mut u8, len: usize)` — frees a buffer previously returned by `trait_call`

### Flow (trait_call)

1. If input is null or empty, call handler with empty slice
2. Deserialize input bytes as `Vec<Value>`
3. On deserialization error, return `{"error": "invalid JSON args"}`
4. Call the handler function with args slice
5. Serialize result to JSON bytes
6. Transfer ownership via `mem::forget`, write length to `out_len`, return pointer

### Flow (trait_free)

1. If pointer is non-null and length > 0, reconstruct Vec and drop it

### Usage

```rust
// lib.rs of a cdylib plugin
#[path = "my_impl.rs"]
mod my_impl;

plugin_api::export_trait!(my_impl::handler);
```

### Notes

* The kernel's `dylib_loader` is the caller — it passes JSON args and frees the result buffer
* Both sides (kernel + plugin) share the same allocator since they're in the same process
* Memory safety relies on the caller freeing each `trait_call` result exactly once via `trait_free`
* Crate lives at `traits/kernel/plugin_api/` alongside other kernel modules
