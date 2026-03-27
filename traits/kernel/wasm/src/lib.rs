use wasm_bindgen::prelude::*;
use serde_json::Value;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

mod registry;
mod wasm_traits;
pub(crate) mod wasm_secrets;

// ── Helper connection state (set by JS when local helper is discovered) ──
static HELPER_CONNECTED: AtomicBool = AtomicBool::new(false);

pub(crate) fn is_helper_connected() -> bool {
    HELPER_CONNECTED.load(Ordering::Relaxed)
}

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

    // Initialize platform abstraction layer (dispatch, registry, config, secrets)
    kernel_logic::platform::init(kernel_logic::platform::Platform {
        dispatch: |path, args| wasm_traits::dispatch(path, args),
        registry_all: || {
            get_registry().all().iter().map(|t| serde_json::json!({
                "path": t.path,
                "description": t.description,
                "version": t.version,
                "tags": t.tags,
                "wasm_callable": t.wasm_callable,
            })).collect()
        },
        registry_count: || get_registry().len(),
        registry_detail: |path| {
            get_registry().get(path).map(|t| serde_json::json!({
                "path": t.path,
                "description": t.description,
                "version": t.version,
                "author": t.author,
                "tags": t.tags,
                "provides": t.provides,
                "language": t.language,
                "source": t.source_type,
                "wasm_callable": t.wasm_callable,
                "params": t.params,
                "returns": t.returns_type,
                "returns_description": t.returns_description,
            }))
        },
        config_get: |_trait_path, _key, default| default.to_string(),
        secret_get: wasm_secrets::get_secret,
    });
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

/// Store a secret for use by sys.call / sys.llm (e.g. API keys).
/// Call before invoking traits that need auth: set_secret("openai_api_key", "sk-...")
#[wasm_bindgen]
pub fn set_secret(key: &str, value: &str) {
    wasm_secrets::set_secret(key, value);
}

/// Notify the WASM kernel whether a local helper (native binary) is connected.
/// When connected, helper-preferred traits (e.g. sys.ps) delegate to the helper
/// for richer native data (OS processes, filesystem, etc.) instead of using
/// the WASM-local fallback.
#[wasm_bindgen]
pub fn set_helper_connected(connected: bool) {
    HELPER_CONNECTED.store(connected, Ordering::Relaxed);
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

/// Run trait tests matching a glob pattern. Returns JSON results.
/// Only example-based tests run in WASM; shell command tests are skipped.
#[wasm_bindgen]
pub fn run_tests(pattern: &str, verbose: bool) -> String {
    let args = vec![
        serde_json::Value::String(pattern.to_string()),
        serde_json::Value::Bool(verbose),
    ];
    let result = wasm_traits::test_runner::test_runner(&args);
    serde_json::to_string(&result).unwrap_or_default()
}

// ═══════════════════════════════════════════
// ── CLI interface (powered by kernel/cli) ──
// Stateful session: all line editing, history,
// tab completion, and interactive mode live in Rust.
// The browser terminal is a thin display layer.
// ═══════════════════════════════════════════

use std::cell::RefCell;
use wasm_traits::cli::{CliBackend, self as cli_core};

/// WASM implementation of CliBackend — thin dispatch wrapper delegating to sys.cli.wasm.
struct WasmCliBackend;

impl WasmCliBackend {
    fn dispatch_method(&self, method: &str, args: &[Value]) -> Option<Value> {
        let mut full_args = vec![Value::String(method.to_string())];
        full_args.extend_from_slice(args);
        // Resolve "wasm" via kernel.cli: bindings[wasm] → requires[wasm] → auto-discover
        let backend = get_registry()
            .resolve_keyed("kernel.cli", "wasm")
            .unwrap_or_else(|| "sys.cli.wasm".to_string());
        wasm_traits::dispatch(&backend, &full_args)
    }
}

impl CliBackend for WasmCliBackend {
    fn call(&self, path: &str, args: &[Value]) -> Result<Value, String> {
        match self.dispatch_method("call", &[serde_json::json!(path), Value::Array(args.to_vec())]) {
            Some(v) => {
                if v.get("ok").and_then(|b| b.as_bool()) == Some(true) {
                    Ok(v.get("result").cloned().unwrap_or(Value::Null))
                } else {
                    Err(v.get("error").and_then(|e| e.as_str()).unwrap_or("unknown error").to_string())
                }
            }
            None => Err("Backend dispatch failed".into()),
        }
    }

    fn list_all(&self) -> Vec<Value> {
        self.dispatch_method("list_all", &[])
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    }

    fn get_info(&self, path: &str) -> Option<Value> {
        self.dispatch_method("get_info", &[serde_json::json!(path)])
            .filter(|v| !v.is_null())
    }

    fn search(&self, query: &str) -> Vec<Value> {
        self.dispatch_method("search", &[serde_json::json!(query)])
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    }

    fn all_paths(&self) -> Vec<String> {
        self.dispatch_method("all_paths", &[])
            .and_then(|v| v.as_array().cloned())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    }

    fn version(&self) -> String {
        self.dispatch_method("version", &[])
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
    }

    fn load_examples(&self, path: &str) -> Vec<Vec<String>> {
        self.dispatch_method("load_examples", &[serde_json::json!(path)])
            .and_then(|v| v.as_array().cloned())
            .map(|arr| {
                arr.iter().filter_map(|ex| {
                    ex.as_array().map(|a| {
                        a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                    })
                }).collect()
            })
            .unwrap_or_default()
    }
}

thread_local! {
    static CLI_SESSION: RefCell<Option<cli_core::CliSession>> = RefCell::new(None);
}

fn with_session<F, R>(f: F) -> R
where
    F: FnOnce(&mut cli_core::CliSession) -> R,
{
    CLI_SESSION.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(cli_core::CliSession::new());
        }
        f(opt.as_mut().unwrap())
    })
}

/// Feed raw terminal input bytes to the CLI session.
/// Returns ANSI text to write to the terminal.
/// This is the primary interface for browser terminals — all line editing,
/// history, tab completion, and interactive mode are handled in Rust.
#[wasm_bindgen]
pub fn cli_input(data: &str) -> String {
    with_session(|session| {
        session.feed(data, &WasmCliBackend)
    })
}

/// Get the welcome banner + initial prompt.
#[wasm_bindgen]
pub fn cli_welcome() -> String {
    with_session(|session| {
        session.welcome(&WasmCliBackend)
    })
}

/// Format a REST response for display in the terminal.
/// Called by terminal.js after a REST sentinel dispatch completes.
/// Returns formatted ANSI text, or empty string to fall back to JSON.
#[wasm_bindgen]
pub fn cli_format_rest_result(trait_path: &str, args_json: &str, result_json: &str) -> String {
    let args: Vec<Value> = serde_json::from_str(args_json).unwrap_or_default();
    let result: Value = serde_json::from_str(result_json).unwrap_or(Value::Null);
    cli_core::format_rest_result(trait_path, &args, &result).unwrap_or_default()
}

/// Return the current command history as a JSON array (most recent last).
/// Used by terminal.js to persist history to localStorage.
#[wasm_bindgen]
pub fn cli_get_history() -> String {
    with_session(|session| {
        serde_json::to_string(session.get_history()).unwrap_or_else(|_| "[]".to_string())
    })
}

/// Restore command history from a JSON array (e.g. from localStorage on page load).
#[wasm_bindgen]
pub fn cli_set_history(history_json: &str) {
    if let Ok(history) = serde_json::from_str::<Vec<String>>(history_json) {
        with_session(|session| session.set_history(history));
    }
}
