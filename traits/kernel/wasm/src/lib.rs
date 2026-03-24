use wasm_bindgen::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

mod registry;

// Include generated trait dispatch table
include!(concat!(env!("OUT_DIR"), "/wasm_dispatch.rs"));

// Include generated trait definitions
include!(concat!(env!("OUT_DIR"), "/wasm_builtin_traits.rs"));

static REGISTRY: OnceLock<registry::WasmRegistry> = OnceLock::new();

fn get_registry() -> &'static registry::WasmRegistry {
    REGISTRY.get_or_init(|| {
        let mut reg = registry::WasmRegistry::new();
        reg.load_builtins(BUILTIN_TRAIT_DEFS);
        // Mark which traits are callable in WASM
        for path in list_wasm_callable() {
            reg.mark_wasm_callable(path);
        }
        reg
    })
}

/// Initialize the WASM kernel. Call once before using other functions.
#[wasm_bindgen]
pub fn init() -> Result<JsValue, JsValue> {
    // Force registry initialization
    let reg = get_registry();
    let count = reg.len();
    let callable = list_wasm_callable().len();
    Ok(serde_json::to_string(&serde_json::json!({
        "status": "ok",
        "traits_registered": count,
        "wasm_callable": callable,
        "version": env!("CARGO_PKG_VERSION"),
    })).unwrap().into())
}

/// Call a trait by dot-notation path with JSON args.
/// Returns JSON string result.
#[wasm_bindgen]
pub fn call(trait_path: &str, args_json: &str) -> Result<String, JsValue> {
    let args: Vec<Value> = match serde_json::from_str(args_json) {
        Ok(Value::Array(a)) => a,
        Ok(v) => vec![v],
        Err(e) => return Err(JsValue::from_str(&format!("Invalid JSON args: {}", e))),
    };

    match dispatch_wasm(trait_path, &args) {
        Some(result) => Ok(serde_json::to_string(&result).unwrap_or_default()),
        None => {
            // Check if trait exists but isn't WASM-callable
            let reg = get_registry();
            if reg.get(trait_path).is_some() {
                Err(JsValue::from_str(&format!(
                    "Trait '{}' exists but is not callable in WASM (requires server-side I/O)",
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
    serde_json::to_string(&list_wasm_callable()).unwrap_or_default()
}

/// Get kernel version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
