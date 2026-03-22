/// Generate C ABI exports (`trait_call`, `trait_free`) for a trait function.
///
/// The target function must have signature:
///   `fn(&[serde_json::Value]) -> serde_json::Value`
///
/// # Usage
/// ```ignore
/// mod my_trait;
/// plugin_api::export_trait!(my_trait::handler);
/// ```
///
/// This generates:
/// - `trait_call(json_ptr, json_len, out_len) -> *mut u8`
/// - `trait_free(ptr, len)`
///
/// The caller (kernel's dylib_loader) passes JSON-serialized args as bytes,
/// receives JSON-serialized result as bytes, and frees the buffer afterward.
#[macro_export]
macro_rules! export_trait {
    ($func:path) => {
        /// C ABI entry point: receives JSON args, calls trait function, returns JSON result.
        ///
        /// # Safety
        /// - `json_ptr` must point to `json_len` valid bytes of JSON
        /// - `out_len` must be a valid pointer to write the result length
        /// - Caller must free the returned buffer via `trait_free`
        #[no_mangle]
        pub unsafe extern "C" fn trait_call(
            json_ptr: *const u8,
            json_len: usize,
            out_len: *mut usize,
        ) -> *mut u8 {
            *out_len = 0;

            if json_ptr.is_null() || json_len == 0 {
                // Empty args → call with empty slice
                let result = $func(&[]);
                let result_bytes = serde_json::to_vec(&result).unwrap_or_default();
                let len = result_bytes.len();
                let ptr = result_bytes.as_ptr() as *mut u8;
                std::mem::forget(result_bytes);
                *out_len = len;
                return ptr;
            }

            let bytes = std::slice::from_raw_parts(json_ptr, json_len);
            let args: Vec<serde_json::Value> = match serde_json::from_slice(bytes) {
                Ok(v) => v,
                Err(_) => {
                    let err = serde_json::json!({"error": "invalid JSON args"});
                    let err_bytes = serde_json::to_vec(&err).unwrap_or_default();
                    let len = err_bytes.len();
                    let ptr = err_bytes.as_ptr() as *mut u8;
                    std::mem::forget(err_bytes);
                    *out_len = len;
                    return ptr;
                }
            };

            let result = $func(&args);
            let result_bytes = serde_json::to_vec(&result).unwrap_or_default();
            let len = result_bytes.len();
            let ptr = result_bytes.as_ptr() as *mut u8;
            std::mem::forget(result_bytes);
            *out_len = len;
            ptr
        }

        /// C ABI: free a buffer previously returned by `trait_call`.
        ///
        /// # Safety
        /// - `ptr` must have been returned by `trait_call`
        /// - `len` must match the `out_len` value set by `trait_call`
        /// - Must be called exactly once per `trait_call` return
        #[no_mangle]
        pub unsafe extern "C" fn trait_free(ptr: *mut u8, len: usize) {
            if !ptr.is_null() && len > 0 {
                drop(Vec::from_raw_parts(ptr, len, len));
            }
        }
    };
}

// ── Trait dispatch entry point (only in the main binary) ──

/// kernel.plugin_api introspection: returns plugin ABI contract and installed plugins.
#[cfg(kernel)]
pub fn plugin_api(args: &[serde_json::Value]) -> serde_json::Value {
    let _ = args;

    // Query the global dylib loader for installed plugins
    let plugins = match crate::dylib_loader::LOADER.get() {
        Some(loader) => {
            let list = loader.list();
            serde_json::json!(list)
        }
        None => serde_json::json!([]),
    };

    serde_json::json!({
        "abi": {
            "version": 1,
            "entry": "trait_call(json_ptr: *const u8, json_len: usize, out_len: *mut usize) -> *mut u8",
            "free": "trait_free(ptr: *mut u8, len: usize)",
            "convention": "C",
            "format": "JSON bytes in, JSON bytes out"
        },
        "installed_plugins": plugins,
        "plugin_count": plugins.as_array().map(|a| a.len()).unwrap_or(0)
    })
}
