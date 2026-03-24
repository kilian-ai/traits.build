/// Dynamic shared-library loader for trait dylibs.
///
/// Each trait is compiled as a cdylib exporting:
///   trait_call(json_ptr, json_len, out_len) -> *mut u8
///   trait_free(ptr, len)
///   trait_init(dispatch_fn)               (optional)
///
/// The loader scans a directory for .dylib/.so files, loads them,
/// and dispatches calls by trait path. Supports hot-reload.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use serde_json::Value;
use tracing::{info, warn};

/// C ABI function signatures (must match plugin_api crate)
type TraitCallFn = unsafe extern "C" fn(*const u8, usize, *mut usize) -> *mut u8;
type TraitFreeFn = unsafe extern "C" fn(*mut u8, usize);
type DispatchFn = unsafe extern "C" fn(*const u8, usize, *mut usize) -> *mut u8;
type TraitInitFn = unsafe extern "C" fn(DispatchFn);

/// A loaded trait dylib with its symbols
struct LoadedTrait {
    /// Keep the library alive — symbols are only valid while this exists
    _lib: libloading::Library,
    call: TraitCallFn,
    free: TraitFreeFn,
    #[allow(dead_code)]
    path: String,
    #[allow(dead_code)]
    dylib_path: PathBuf,
}

/// Registry of loaded trait dylibs
pub struct DylibLoader {
    traits: Arc<RwLock<HashMap<String, LoadedTrait>>>,
    search_dirs: Vec<PathBuf>,
}

// SAFETY: LoadedTrait holds a Library + function pointers from it.
// The pointers remain valid as long as the Library is alive.
// We guarantee this by never dropping the Library while pointers exist.
unsafe impl Send for LoadedTrait {}
unsafe impl Sync for LoadedTrait {}

impl DylibLoader {
    pub fn new(search_dirs: Vec<PathBuf>) -> Self {
        Self {
            traits: Arc::new(RwLock::new(HashMap::new())),
            search_dirs,
        }
    }

    /// Scan search directories (recursively) and load all trait dylibs.
    /// Supports two discovery modes:
    ///   1. Filename convention: libsys_<name>.dylib → sys.<name>
    ///   2. TOML discovery: .trait.toml with source = "dylib" + companion .dylib
    pub fn load_all(&self) -> usize {
        let mut loaded = 0;
        for dir in &self.search_dirs {
            if !dir.exists() {
                continue;
            }
            loaded += self.scan_dir_recursive(dir);
        }
        loaded
    }

    /// Recursively scan a directory for .trait.toml files with source = "dylib"
    /// (TOML discovery, preferred) and standalone dylib files (filename convention).
    fn scan_dir_recursive(&self, dir: &Path) -> usize {
        let mut loaded = 0;
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                warn!("Cannot read dylib dir {}: {}", dir.display(), e);
                return 0;
            }
        };

        // Collect entries so we can process TOML files before dylib files
        let mut dirs = Vec::new();
        let mut tomls = Vec::new();
        let mut dylibs = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.to_string_lossy().ends_with(".trait.toml") {
                tomls.push(path);
            } else if is_dylib(&path) {
                dylibs.push(path);
            }
        }

        // Recurse into subdirectories
        for d in dirs {
            loaded += self.scan_dir_recursive(&d);
        }

        // Mode 2 first: .trait.toml with source = "dylib"
        for toml_path in tomls {
            if let Some(count) = self.try_load_toml_dylib(&toml_path) {
                loaded += count;
            }
        }

        // Mode 1: standalone .dylib/.so files (skip if already loaded via TOML)
        for dylib_path in dylibs {
            // Skip if this dylib was already loaded by TOML discovery
            {
                let traits = self.traits.read().unwrap();
                if traits.values().any(|t| t.dylib_path == dylib_path) {
                    continue;
                }
            }
            // Skip if a .trait.toml exists in the same directory — the TOML
            // governs how this trait is loaded; if it wanted dylib, Mode 2
            // would have handled it already.
            if let Some(parent) = dylib_path.parent() {
                let has_toml = std::fs::read_dir(parent)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .any(|e| e.path().to_string_lossy().ends_with(".trait.toml"));
                if has_toml {
                    continue;
                }
            }
            match self.load_dylib(&dylib_path) {
                Ok(trait_path) => {
                    info!("Loaded dylib trait: {} from {}", trait_path, dylib_path.display());
                    loaded += 1;
                }
                Err(e) => {
                    warn!("Failed to load {}: {}", dylib_path.display(), e);
                }
            }
        }

        loaded
    }

    /// Load a single dylib and register it by trait path (filename convention).
    fn load_dylib(&self, dylib_path: &Path) -> Result<String, String> {
        let trait_path = dylib_name_to_trait_path(dylib_path)
            .ok_or_else(|| format!("Cannot derive trait path from {}", dylib_path.display()))?;
        self.load_dylib_with_path(dylib_path, &trait_path)
    }

    /// Try to load a dylib from a .trait.toml that declares source = "dylib".
    /// Looks for a companion .dylib file in the same directory.
    /// Derives trait path from the filesystem structure relative to the traits root.
    fn try_load_toml_dylib(&self, toml_path: &Path) -> Option<usize> {
        let content = std::fs::read_to_string(toml_path).ok()?;

        // Quick check: must have source = "dylib"
        let is_dylib_source = content.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("source") && trimmed.contains("\"dylib\"")
        });
        if !is_dylib_source {
            return None;
        }

        let dir = toml_path.parent()?;
        let dir_name = dir.file_name()?.to_str()?;

        // Look for companion dylib: lib<crate_name>.dylib or lib<dir_name>.dylib
        // Also check for the Cargo cdylib output copied into this directory
        let dylib_ext = if cfg!(target_os = "macos") { "dylib" } else { "so" };
        let candidates = [
            dir.join(format!("lib{}.{}", dir_name, dylib_ext)),
            dir.join(format!("libtrait.{}", dylib_ext)),
        ];

        let dylib_path = candidates.iter().find(|p| p.exists())?;

        // Derive trait path from directory structure
        // Walk up from toml dir through search_dirs to find the relative path
        let trait_path = self.derive_trait_path(dir)?;

        match self.load_dylib_with_path(dylib_path, &trait_path) {
            Ok(tp) => {
                info!("Loaded dylib trait (TOML): {} from {}", tp, dylib_path.display());
                Some(1)
            }
            Err(e) => {
                warn!("Failed to load TOML dylib {}: {}", toml_path.display(), e);
                None
            }
        }
    }

    /// Derive a trait path from a directory by finding its position relative to a search dir.
    /// e.g., search_dir/www/traits/build → www.traits.build
    fn derive_trait_path(&self, dir: &Path) -> Option<String> {
        for search_dir in &self.search_dirs {
            if let Ok(rel) = dir.strip_prefix(search_dir) {
                let parts: Vec<&str> = rel.components()
                    .filter_map(|c| c.as_os_str().to_str())
                    .collect();
                if !parts.is_empty() {
                    return Some(parts.join("."));
                }
            }
        }
        None
    }

    /// Load a dylib file and register it under the given trait path.
    fn load_dylib_with_path(&self, dylib_path: &Path, trait_path: &str) -> Result<String, String> {

        // SAFETY: We're loading a shared library. The library must export
        // the expected C ABI symbols. We verify symbols exist before storing.
        let lib = unsafe {
            libloading::Library::new(dylib_path)
                .map_err(|e| format!("dlopen failed: {}", e))?
        };

        let call: TraitCallFn = unsafe {
            *lib.get::<TraitCallFn>(b"trait_call")
                .map_err(|e| format!("missing trait_call symbol: {}", e))?
        };

        let free: TraitFreeFn = unsafe {
            *lib.get::<TraitFreeFn>(b"trait_free")
                .map_err(|e| format!("missing trait_free symbol: {}", e))?
        };

        // trait_init is optional — only needed for cross-trait dispatch
        let has_init = unsafe { lib.get::<TraitInitFn>(b"trait_init").is_ok() };
        if has_init {
            let init: TraitInitFn = unsafe {
                *lib.get::<TraitInitFn>(b"trait_init").unwrap()
            };
            // Provide the dispatch callback
            unsafe { init(server_dispatch) };
        }

        let loaded = LoadedTrait {
            _lib: lib,
            call,
            free,
            path: trait_path.to_string(),
            dylib_path: dylib_path.to_path_buf(),
        };

        let mut traits = self.traits.write().map_err(|e| e.to_string())?;
        traits.insert(trait_path.to_string(), loaded);

        Ok(trait_path.to_string())
    }

    /// Call a trait by path with JSON args. Returns Some(result) if found.
    pub fn dispatch(&self, trait_path: &str, args: &[Value]) -> Option<Value> {
        let traits = self.traits.read().ok()?;
        let loaded = traits.get(trait_path)?;

        let input = serde_json::to_vec(args).unwrap_or_default();
        let mut out_len: usize = 0;

        let result_ptr = unsafe {
            (loaded.call)(input.as_ptr(), input.len(), &mut out_len)
        };

        if result_ptr.is_null() || out_len == 0 {
            return Some(Value::Null);
        }

        let result_bytes = unsafe { std::slice::from_raw_parts(result_ptr, out_len) };
        let result: Value = serde_json::from_slice(result_bytes).unwrap_or(Value::Null);

        // Free the buffer allocated by the dylib
        unsafe { (loaded.free)(result_ptr, out_len) };

        Some(result)
    }

    /// Reload a specific trait by path. Unloads old, loads new.
    #[allow(dead_code)]
    pub fn reload(&self, trait_path: &str) -> Result<(), String> {
        let dylib_path = {
            let traits = self.traits.read().map_err(|e| e.to_string())?;
            traits.get(trait_path).map(|t| t.dylib_path.clone())
        };

        match dylib_path {
            Some(path) => {
                // Unload first (drop the old library)
                {
                    let mut traits = self.traits.write().map_err(|e| e.to_string())?;
                    traits.remove(trait_path);
                }
                // Re-load
                self.load_dylib(&path)?;
                info!("Reloaded dylib trait: {}", trait_path);
                Ok(())
            }
            None => Err(format!("Trait {} not loaded as dylib", trait_path)),
        }
    }

    /// Reload all traits (e.g. after rebuilding dylibs)
    #[allow(dead_code)]
    pub fn reload_all(&self) -> usize {
        let _old_paths: Vec<(String, PathBuf)> = {
            let traits = self.traits.read().unwrap();
            traits.iter().map(|(k, v)| (k.clone(), v.dylib_path.clone())).collect()
        };

        // Clear all
        {
            let mut traits = self.traits.write().unwrap();
            traits.clear();
        }

        // Re-scan directories
        self.load_all()
    }

    /// List all loaded dylib trait paths
    pub fn list(&self) -> Vec<String> {
        let traits = self.traits.read().unwrap();
        let mut paths: Vec<String> = traits.keys().cloned().collect();
        paths.sort();
        paths
    }

    /// Check if a trait path is loaded as a dylib
    #[allow(dead_code)]
    pub fn has(&self, trait_path: &str) -> bool {
        let traits = self.traits.read().unwrap();
        traits.contains_key(trait_path)
    }
}

// ── Dispatch callback provided to dylibs for cross-trait calls ──

// Global reference to the DylibLoader for the dispatch callback
pub static LOADER: std::sync::OnceLock<Arc<DylibLoader>> = std::sync::OnceLock::new();

/// Set the global loader reference (called once at startup)
pub fn set_global_loader(loader: Arc<DylibLoader>) {
    let _ = LOADER.set(loader);
}

/// The dispatch function provided to dylibs via trait_init.
/// Dylibs call this to invoke other traits.
///
/// Input JSON: {"path": "sys.checksum", "args": [...]}
/// Output: JSON result bytes
///
/// SAFETY: This is called from dylib code via C ABI. The input pointer
/// must be valid for `len` bytes. We allocate the result buffer here and
/// the dylib frees it (both share the same allocator in the same process).
unsafe extern "C" fn server_dispatch(
    json_ptr: *const u8,
    json_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    *out_len = 0;

    if json_ptr.is_null() || json_len == 0 {
        return std::ptr::null_mut();
    }

    let bytes = std::slice::from_raw_parts(json_ptr, json_len);
    let request: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return std::ptr::null_mut(),
    };

    let path = request.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let args: Vec<Value> = request.get("args")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Dispatch: try dylib loader first, then fall back to compiled-in traits
    let result = if let Some(loader) = LOADER.get() {
        if let Some(r) = loader.dispatch(path, &args) {
            r
        } else {
            // Fall back to compiled-in dispatch
            crate::dispatcher::compiled::dispatch_compiled(path, &args)
                .unwrap_or_else(|| serde_json::json!({"error": format!("trait not found: {}", path)}))
        }
    } else {
        serde_json::json!({"error": "dispatch not initialized"})
    };

    let result_bytes = serde_json::to_vec(&result).unwrap_or_default();
    let len = result_bytes.len();
    let ptr = result_bytes.as_ptr() as *mut u8;
    std::mem::forget(result_bytes);
    *out_len = len;
    ptr
}

// ── Trait dispatch entry point ──

/// kernel.dylib_loader introspection: returns loaded dylib info.
pub fn dylib_loader(args: &[serde_json::Value]) -> serde_json::Value {
    let _ = args;
    match LOADER.get() {
        Some(loader) => {
            let list = loader.list();
            serde_json::json!({
                "loaded_count": list.len(),
                "loaded_traits": list,
                "search_dirs": loader.search_dirs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            })
        }
        None => serde_json::json!({
            "loaded_count": 0,
            "loaded_traits": [],
            "status": "not initialized"
        }),
    }
}

// ── Helpers ──

/// Check if a path is a shared library
fn is_dylib(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "dylib" | "so" => true,
        _ => false,
    }
}

/// Convert a dylib filename to a trait path.
/// e.g., "libsys_checksum.dylib" -> "sys.checksum"
///       "libsys_chain_anchor.dylib" -> "sys.chain_anchor"
fn dylib_name_to_trait_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    // Strip the "lib" prefix
    let name = stem.strip_prefix("lib")?;
    // Convert first underscore to dot (namespace separator): sys_checksum -> sys.checksum
    let dot_pos = name.find('_')?;
    let namespace = &name[..dot_pos];
    let trait_name = &name[dot_pos + 1..];
    Some(format!("{}.{}", namespace, trait_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dylib_name_to_trait_path() {
        assert_eq!(
            dylib_name_to_trait_path(Path::new("libsys_checksum.dylib")),
            Some("sys.checksum".to_string())
        );
        assert_eq!(
            dylib_name_to_trait_path(Path::new("libsys_chain_anchor.dylib")),
            Some("sys.chain_anchor".to_string())
        );
        assert_eq!(
            dylib_name_to_trait_path(Path::new("libsys_registry_chain.so")),
            Some("sys.registry_chain".to_string())
        );
        assert_eq!(
            dylib_name_to_trait_path(Path::new("not_a_lib.dylib")),
            None
        );
    }
}
