use wasm_bindgen::prelude::*;
use serde_json::Value;
use std::sync::OnceLock;

mod registry;
mod wasm_traits;

// Include generated trait definitions (for registry browsing)
include!(concat!(env!("OUT_DIR"), "/wasm_builtin_traits.rs"));

static REGISTRY: OnceLock<registry::WasmRegistry> = OnceLock::new();

pub(crate) fn get_registry() -> &'static registry::WasmRegistry {
    REGISTRY.get_or_init(|| {
        let mut reg = registry::WasmRegistry::new();
        reg.load_builtins(BUILTIN_TRAIT_DEFS);
        // Mark curated WASM-callable traits
        for path in wasm_traits::WASM_CALLABLE {
            reg.mark_wasm_callable(path);
        }
        reg
    })
}

/// Initialize the WASM kernel. Call once before using other functions.
#[wasm_bindgen]
pub fn init() -> Result<JsValue, JsValue> {
    let reg = get_registry();
    let count = reg.len();
    let callable = wasm_traits::WASM_CALLABLE.len();
    Ok(serde_json::to_string(&serde_json::json!({
        "status": "ok",
        "traits_registered": count,
        "wasm_callable": callable,
        "version": env!("CARGO_PKG_VERSION"),
    })).unwrap().into())
}

/// Check if a trait can be called locally in WASM.
#[wasm_bindgen]
pub fn is_callable(trait_path: &str) -> bool {
    wasm_traits::WASM_CALLABLE.contains(&trait_path)
}

/// Check if a trait is registered (even if not WASM-callable).
#[wasm_bindgen]
pub fn is_registered(trait_path: &str) -> bool {
    get_registry().get(trait_path).is_some()
}

/// Call a trait by dot-notation path with JSON args.
/// Returns JSON string result. Only works for WASM-callable traits.
/// For non-WASM traits, use the traits.js REST client.
#[wasm_bindgen]
pub fn call(trait_path: &str, args_json: &str) -> Result<String, JsValue> {
    let args: Vec<Value> = match serde_json::from_str(args_json) {
        Ok(Value::Array(a)) => a,
        Ok(v) => vec![v],
        Err(e) => return Err(JsValue::from_str(&format!("Invalid JSON args: {}", e))),
    };

    match wasm_traits::dispatch(trait_path, &args) {
        Some(result) => Ok(serde_json::to_string(&result).unwrap_or_default()),
        None => {
            let reg = get_registry();
            if reg.get(trait_path).is_some() {
                Err(JsValue::from_str(&format!(
                    "Trait '{}' exists but requires REST dispatch (use Traits client)",
                    trait_path
                )))
            } else {
                Err(JsValue::from_str(&format!("Trait '{}' not found", trait_path)))
            }
        }
    }
}

/// List all registered traits as JSON array.
#[wasm_bindgen]
pub fn list_traits() -> String {
    let reg = get_registry();
    let traits: Vec<Value> = reg.all()
        .iter()
        .map(|e| serde_json::json!({
            "path": e.path,
            "description": e.description,
            "version": e.version,
            "tags": e.tags,
            "wasm_callable": e.wasm_callable,
            "params": e.params,
            "returns": e.returns_type,
        }))
        .collect();
    serde_json::to_string(&traits).unwrap_or_default()
}

/// Get detailed info for a specific trait as JSON.
#[wasm_bindgen]
pub fn get_trait_info(trait_path: &str) -> Option<String> {
    let reg = get_registry();
    reg.get(trait_path).map(|e| {
        serde_json::to_string(&serde_json::json!({
            "path": e.path,
            "description": e.description,
            "version": e.version,
            "author": e.author,
            "tags": e.tags,
            "wasm_callable": e.wasm_callable,
            "params": e.params,
            "returns": e.returns_type,
            "returns_description": e.returns_description,
            "provides": e.provides,
            "language": e.language,
            "source": e.source_type,
        })).unwrap_or_default()
    })
}

/// Search traits by query string (matches path and description).
#[wasm_bindgen]
pub fn search_traits(query: &str) -> String {
    let reg = get_registry();
    let q = query.to_lowercase();
    let matches: Vec<Value> = reg.all()
        .iter()
        .filter(|e| {
            e.path.to_lowercase().contains(&q) ||
            e.description.to_lowercase().contains(&q) ||
            e.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .map(|e| serde_json::json!({
            "path": e.path,
            "description": e.description,
            "version": e.version,
            "wasm_callable": e.wasm_callable,
        }))
        .collect();
    serde_json::to_string(&matches).unwrap_or_default()
}

/// Get the list of traits that can be called directly in WASM.
#[wasm_bindgen]
pub fn callable_traits() -> String {
    serde_json::to_string(&wasm_traits::WASM_CALLABLE).unwrap_or_default()
}

/// Get kernel version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ═══════════════════════════════════════════
// ── CLI interface (powered by kernel/cli) ──
// ═══════════════════════════════════════════

use wasm_traits::cli::{CliBackend, self as cli_core};

/// WASM implementation of CliBackend — bridges kernel/cli to WASM registry+dispatch.
struct WasmCliBackend;

impl CliBackend for WasmCliBackend {
    fn call(&self, path: &str, args: &[Value]) -> Result<Value, String> {
        match wasm_traits::dispatch(path, args) {
            Some(result) => Ok(result),
            None => {
                let reg = get_registry();
                if reg.get(path).is_some() {
                    Err(format!("Trait '{}' requires REST dispatch (not available in WASM)", path))
                } else {
                    Err(format!("Trait '{}' not found", path))
                }
            }
        }
    }

    fn list_all(&self) -> Vec<Value> {
        let reg = get_registry();
        reg.all().iter().map(|e| serde_json::json!({
            "path": e.path,
            "description": e.description,
            "version": e.version,
            "tags": e.tags,
            "wasm_callable": e.wasm_callable,
        })).collect()
    }

    fn get_info(&self, path: &str) -> Option<Value> {
        let reg = get_registry();
        reg.get(path).map(|e| serde_json::json!({
            "path": e.path,
            "description": e.description,
            "version": e.version,
            "author": e.author,
            "tags": e.tags,
            "wasm_callable": e.wasm_callable,
            "params": e.params,
            "returns": e.returns_type,
            "returns_description": e.returns_description,
        }))
    }

    fn search(&self, query: &str) -> Vec<Value> {
        let reg = get_registry();
        let q = query.to_lowercase();
        reg.all().iter()
            .filter(|e| {
                e.path.to_lowercase().contains(&q) ||
                e.description.to_lowercase().contains(&q) ||
                e.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .map(|e| serde_json::json!({
                "path": e.path,
                "description": e.description,
                "wasm_callable": e.wasm_callable,
            }))
            .collect()
    }

    fn all_paths(&self) -> Vec<String> {
        let reg = get_registry();
        reg.all().iter().map(|e| e.path.clone()).collect()
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

/// Execute a CLI command line. Returns ANSI-formatted output.
/// This is the primary interface for browser terminals.
#[wasm_bindgen]
pub fn cli_exec(line: &str) -> String {
    cli_core::exec_line(line, &WasmCliBackend)
}

/// Get tab completions for a prefix. Returns JSON: {"matches": [...], "common": "..."}
#[wasm_bindgen]
pub fn cli_complete(prefix: &str) -> String {
    let reg = get_registry();
    let all_paths: Vec<String> = reg.all().iter().map(|e| e.path.clone()).collect();
    let (matches, common) = cli_core::tab_completions(prefix, &all_paths);
    serde_json::to_string(&serde_json::json!({
        "matches": matches,
        "common": common,
    })).unwrap_or_default()
}

/// Get interactive mode parameters for a trait.
/// Returns JSON array of param objects, or empty string if not found.
#[wasm_bindgen]
pub fn cli_interactive_params(path: &str) -> String {
    match cli_core::interactive_params(path, &WasmCliBackend) {
        Some(params) => serde_json::to_string(&params).unwrap_or_default(),
        None => String::new(),
    }
}
