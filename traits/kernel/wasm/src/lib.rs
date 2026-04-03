use wasm_bindgen::prelude::*;
use serde_json::Value;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

mod registry;
mod wasm_traits;
pub(crate) mod wasm_secrets;

// ── Helper connection state (set by JS when local helper is discovered) ──
static HELPER_CONNECTED: AtomicBool = AtomicBool::new(false);
static HELPER_URL: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

pub(crate) fn is_helper_connected() -> bool {
    HELPER_CONNECTED.load(Ordering::Relaxed)
}

/// Get the helper URL — first from in-memory (set via RPC for workers),
/// then from localStorage (main thread fallback).
fn get_helper_url() -> Option<String> {
    if let Ok(guard) = HELPER_URL.read() {
        if let Some(ref url) = *guard {
            return Some(url.clone());
        }
    }
    ls_get("traits.helper.url")
}

// ── Task registry — delegates to kernel_logic::platform shared registry ──

/// Register a background task/service visible in `sys.ps`.
///
/// Called from JS SDK when spawning workers, starting services (terminal,
/// webllm), or executing background trait calls.
///
/// - `id`:        unique identifier (e.g. "worker-0", "terminal", "task-42")
/// - `name`:      display name (e.g. "Web Worker #0", "Terminal", "sys.checksum")
/// - `task_type`: "service" | "worker" | "task"
/// - `started_ms`: `Date.now()` timestamp from JS
/// - `detail`:    optional extra info (adapter name, trait path, etc.)
#[wasm_bindgen]
pub fn register_task(id: &str, name: &str, task_type: &str, started_ms: f64, detail: &str) {
    kernel_logic::platform::register_task(id, name, task_type, started_ms, detail);
}

/// Remove a task from the registry (task completed or service stopped).
#[wasm_bindgen]
pub fn unregister_task(id: &str) {
    kernel_logic::platform::unregister_task(id);
}

/// Update the status of a registered task.
#[wasm_bindgen]
pub fn update_task_status(id: &str, status: &str) {
    kernel_logic::platform::update_task_status(id, status);
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

// ── Persistent VFS (global, separate from CLI session VFS) ──────────────────
//
// The CLI session VFS is scoped to with_session() and causes double-borrow
// panics when traits try to access it during dispatch. This global VFS is
// accessible from any trait via platform::vfs_read/vfs_write, auto-persists
// to localStorage, and is seeded with the same builtins as the session VFS.

use kernel_logic::vfs::Vfs;

thread_local! {
    static PERSISTENT_VFS: RefCell<Option<kernel_logic::vfs::LayeredVfs>> = const { RefCell::new(None) };
}

/// Ensure the persistent VFS is initialized (lazy — first access creates it).
fn ensure_pvfs() {
    PERSISTENT_VFS.with(|cell| {
        if cell.borrow().is_none() {
            let mut vfs = kernel_logic::vfs::LayeredVfs::new();
            // Seed builtins (same as CliSession VFS)
            for (_path, rel_path, toml) in BUILTIN_TRAIT_DEFS {
                vfs.seed(rel_path, *toml);
            }
            for (_path, rel_path, feat) in BUILTIN_FEATURES {
                vfs.seed(rel_path, *feat);
            }
            for (rel_path, content) in BUILTIN_DOCS {
                vfs.seed(rel_path, *content);
            }
            // Restore user layer from localStorage
            if let Some(json) = ls_get("traits.pvfs") {
                vfs.load(&json);
            }
            *cell.borrow_mut() = Some(vfs);
        }
    });
}

fn pvfs_read(path: &str) -> Option<String> {
    ensure_pvfs();
    PERSISTENT_VFS.with(|cell| {
        cell.borrow().as_ref().and_then(|vfs| vfs.read(path))
    })
}

fn pvfs_write(path: &str, content: &str) {
    ensure_pvfs();
    PERSISTENT_VFS.with(|cell| {
        if let Some(vfs) = cell.borrow_mut().as_mut() {
            vfs.write(path, content);
            // Auto-persist user layer to localStorage
            ls_set("traits.pvfs", &vfs.dump());
        }
    });
}

fn pvfs_list() -> Vec<String> {
    ensure_pvfs();
    PERSISTENT_VFS.with(|cell| {
        cell.borrow().as_ref().map(|vfs| vfs.list()).unwrap_or_default()
    })
}

fn pvfs_delete(path: &str) -> bool {
    ensure_pvfs();
    PERSISTENT_VFS.with(|cell| {
        if let Some(vfs) = cell.borrow_mut().as_mut() {
            let deleted = vfs.delete(path);
            if deleted {
                ls_set("traits.pvfs", &vfs.dump());
            }
            deleted
        } else {
            false
        }
    })
}

/// Read from localStorage.
fn ls_get(key: &str) -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item(key).ok()?
}

/// Write to localStorage.
fn ls_set(key: &str, value: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.set_item(key, value);
    }
}

/// Attempt to dispatch a non-WASM trait through the connected helper via HTTP.
///
/// When a trait isn't compiled into the WASM kernel (e.g. skills.spotify.pause),
/// this falls back to calling the native helper binary over REST. The helper URL
/// is stored in localStorage by the JS SDK during helper discovery.
///
/// Flow: WASM dispatch → None → helper_dispatch → sys.call (XHR) → helper REST → result
fn helper_dispatch(path: &str, args: &[Value]) -> Option<Value> {
    if !is_helper_connected() {
        return None;
    }

    // Get helper URL from memory (worker RPC) or localStorage (main thread)
    let helper_url = get_helper_url()?;

    // Build REST endpoint: POST {helper_url}/traits/{namespace}/{name}
    let rest_path = path.replace('.', "/");
    let url = format!("{}/traits/{}", helper_url.trim_end_matches('/'), rest_path);

    // Call via sys.call (WASM-callable, uses synchronous XHR)
    let call_args = vec![
        serde_json::json!(url),
        serde_json::json!({"args": args}),
        serde_json::json!(""),      // no auth secret
        serde_json::json!("POST"),  // method
    ];

    let result = wasm_traits::dispatch("sys.call", &call_args)?;

    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if ok {
        // REST endpoint returns {"result": ..., "error": null}
        // sys.call wraps it as {"ok": true, "body": {"result": ..., "error": null}}
        result.pointer("/body/result").cloned()
            .or_else(|| result.get("body").cloned())
    } else {
        // HTTP error — return as-is so caller sees the failure
        Some(result)
    }
}

/// Initialize the WASM kernel. Call once before using other functions.
#[wasm_bindgen]
pub fn init() -> Result<JsValue, JsValue> {
    let reg = get_registry();
    let count = reg.len();
    let callable = wasm_traits::WASM_CALLABLE.len();

    // Initialize platform abstraction layer (dispatch, registry, config, secrets)
    kernel_logic::platform::init(kernel_logic::platform::Platform {
        dispatch: |path, args| {
            // 1. Try WASM-local dispatch first (instant, in-browser)
            if let Some(result) = wasm_traits::dispatch(path, args) {
                return Some(result);
            }
            // 2. Fall back to helper REST dispatch for non-WASM traits
            helper_dispatch(path, args)
        },
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
                "signature": {
                    "params": t.params,
                    "returns": t.returns_type,
                    "returns_description": t.returns_description,
                },
            }))
        },
        config_get: |_trait_path, _key, default| default.to_string(),
        secret_get: wasm_secrets::get_secret,
        make_vfs: make_wasm_vfs,
        background_tasks: wasm_background_tasks,
        vfs_read: pvfs_read,
        vfs_write: pvfs_write,
        vfs_list: pvfs_list,
        vfs_delete: pvfs_delete,
    });

    Ok(serde_json::to_string(&serde_json::json!({
        "status": "ok",
        "traits_registered": count,
        "wasm_callable": callable,
        "version": env!("CARGO_PKG_VERSION"),
    })).unwrap().into())
}

/// Build a `LayeredVfs` seeded from the embedded WASM binary assets.
///
/// Every `.trait.toml` and `.features.json` that was bundled via `include_str!`
/// in `wasm_builtin_traits.rs` is mounted as a read-only builtin file.
/// Called via `Platform::make_vfs` each time a `CliSession` is created.
///
/// Terminal usage after `init()` + `vfs_load()`:
///   ls                                            → directory tree
///   ls traits/sys/                                → files in sys namespace
///   cat traits/sys/checksum/checksum.trait.toml
///   cat traits/sys/checksum/checksum.features.json
fn make_wasm_vfs() -> Box<dyn kernel_logic::vfs::Vfs> {
    let mut vfs = kernel_logic::vfs::LayeredVfs::new();
    for (_path, rel_path, toml) in BUILTIN_TRAIT_DEFS {
        vfs.seed(rel_path, *toml);
    }
    for (_path, rel_path, feat) in BUILTIN_FEATURES {
        vfs.seed(rel_path, *feat);
    }
    for (rel_path, content) in BUILTIN_DOCS {
        vfs.seed(rel_path, *content);
    }
    Box::new(vfs)
}

// ────────────────── WASM process status ──────────────────

/// Return WASM kernel runtime state for `sys.ps`.
///
/// Reports browser background tasks registered by the JS SDK:
/// - services (terminal, webllm, wasm kernel)
/// - workers (Web Worker pool)
/// - tasks (spawned background trait calls)
/// Plus WASM kernel metadata (callable/registered counts, dispatch cascade).
fn wasm_background_tasks() -> serde_json::Value {
    let callable: Vec<&str> = wasm_traits::WASM_CALLABLE.to_vec();
    let registered = get_registry().len();
    let helper = is_helper_connected();

    // Read task registry → processes array (shared platform registry)
    let processes = kernel_logic::platform::list_tasks();

    serde_json::json!({
        "ok": true,
        "runtime": "wasm",
        "processes": processes,
        "wasm": {
            "callable": callable.len(),
            "registered": registered,
            "traits": callable,
            "threading": "single-threaded (browser main thread)",
            "helper_connected": helper,
            "dispatch_cascade": [
                "1. WASM local (instant, in-browser)",
                format!("2. Local helper ({})", if helper { "connected" } else { "not connected" }),
                "3. Server REST (if origin has backend)",
            ],
        },
    })
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

/// Set the helper URL for worker contexts where localStorage is unavailable.
/// Called from JS SDK via RPC to sync the helper URL into WASM memory.
#[wasm_bindgen]
pub fn set_helper_url(url: &str) {
    if let Ok(mut guard) = HELPER_URL.write() {
        if url.is_empty() {
            *guard = None;
        } else {
            *guard = Some(url.to_string());
        }
    }
}

/// Check if a trait is registered (even if not WASM-callable).
#[wasm_bindgen]
pub fn is_registered(trait_path: &str) -> bool {
    get_registry().get(trait_path).is_some()
}

/// Call a trait by dot-notation path with JSON args.
/// Tries WASM-local dispatch first, then helper REST fallback if connected.
/// For non-WASM traits without a helper, use the traits.js REST client.
#[wasm_bindgen]
pub fn call(trait_path: &str, args_json: &str) -> Result<String, JsValue> {
    let args: Vec<Value> = match serde_json::from_str(args_json) {
        Ok(Value::Array(a)) => a,
        Ok(v) => vec![v],
        Err(e) => return Err(JsValue::from_str(&format!("Invalid JSON args: {}", e))),
    };

    // Try WASM-local dispatch first
    if let Some(result) = wasm_traits::dispatch(trait_path, &args) {
        return Ok(serde_json::to_string(&result).unwrap_or_default());
    }

    // Try helper REST fallback for non-WASM traits
    if let Some(result) = helper_dispatch(trait_path, &args) {
        return Ok(serde_json::to_string(&result).unwrap_or_default());
    }

    // Neither WASM nor helper could handle it
    let reg = get_registry();
    if reg.get(trait_path).is_some() {
        Err(JsValue::from_str(&format!(
            "Trait '{}' exists but requires a connected helper or REST dispatch",
            trait_path
        )))
    } else {
        Err(JsValue::from_str(&format!("Trait '{}' not found", trait_path)))
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
            "source": e.source_type,
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
use wasm_traits::cli::{CliCallBackend, CliHistoryBackend, CliExamplesBackend, self as cli_core};

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

impl CliCallBackend for WasmCliBackend {
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

}

impl CliExamplesBackend for WasmCliBackend {
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

impl CliHistoryBackend for WasmCliBackend {}

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

/// Serialise the VFS to JSON for localStorage persistence.
#[wasm_bindgen]
pub fn vfs_dump() -> String {
    with_session(|session| session.vfs_dump())
}

/// Restore the VFS from a JSON string (e.g. from localStorage on page load).
#[wasm_bindgen]
pub fn vfs_load(json: &str) {
    with_session(|session| session.vfs_load(json))
}

/// Read a single file from the VFS.  Returns empty string if not found.
#[wasm_bindgen]
pub fn vfs_read(path: &str) -> String {
    with_session(|session| session.vfs_read(path).unwrap_or_default())
}

/// Write a single file to the VFS.
#[wasm_bindgen]
pub fn vfs_write(path: &str, content: &str) {
    with_session(|session| session.vfs_write(path, content))
}

/// Re-read the persistent VFS user layer from localStorage.
/// Call this on the main-thread WASM before reading sys.vfs / sys.canvas
/// so writes made by the Worker WASM become visible.
#[wasm_bindgen]
pub fn pvfs_refresh() {
    if let Some(json) = ls_get("traits.pvfs") {
        PERSISTENT_VFS.with(|cell| {
            if let Some(vfs) = cell.borrow_mut().as_mut() {
                vfs.load(&json);
            }
        });
    }
}

/// Dump the persistent VFS user layer as JSON.
/// Used by the Worker to send VFS state to the main thread for localStorage
/// persistence (Workers can't access localStorage directly).
#[wasm_bindgen]
pub fn pvfs_dump() -> String {
    ensure_pvfs();
    PERSISTENT_VFS.with(|cell| {
        cell.borrow().as_ref().map(|vfs| vfs.dump()).unwrap_or_else(|| "{}".to_string())
    })
}

/// Load JSON into the persistent VFS user layer.
/// Used by the Worker at init time to seed VFS from main-thread localStorage
/// (Workers can't access localStorage directly, so the main thread sends the data).
#[wasm_bindgen]
pub fn pvfs_load(json: &str) {
    ensure_pvfs();
    PERSISTENT_VFS.with(|cell| {
        if let Some(vfs) = cell.borrow_mut().as_mut() {
            vfs.load(json);
        }
    });
}

// ────────────────── MCP JSON-RPC handler (browser-only) ──────────────────

/// Process a single MCP JSON-RPC message and return a JSON response string.
/// Returns empty string for notifications (no "id" field).
///
/// This enables full MCP server functionality in the browser without any
/// server or relay — the WASM kernel handles tool listing and dispatch locally.
///
/// Usage from JS:
///   const response = wasm.mcp_message('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}');
#[wasm_bindgen]
pub fn mcp_message(json_message: &str) -> String {
    let request: Value = match serde_json::from_str(json_message) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::to_string(&mcp_error(Value::Null, -32700, &format!("Parse error: {}", e)))
                .unwrap_or_default();
        }
    };

    // Notifications (no "id") — acknowledge silently
    if request.get("id").is_none() {
        return String::new();
    }

    let id = request["id"].clone();
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

    let response = match method {
        "initialize" => mcp_initialize(id),
        "tools/list" => mcp_tools_list(id),
        "tools/call" => mcp_tools_call(id, &request),
        "ping" => mcp_result(id, serde_json::json!({})),
        _ => mcp_error(id, -32601, &format!("Method not found: {}", method)),
    };

    serde_json::to_string(&response).unwrap_or_default()
}

fn mcp_initialize(id: Value) -> Value {
    let reg = get_registry();
    mcp_result(id, serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "traits-wasm-mcp",
            "version": env!("CARGO_PKG_VERSION")
        },
        "info": {
            "runtime": "wasm",
            "traits_registered": reg.len(),
            "wasm_callable": wasm_traits::WASM_CALLABLE.len()
        }
    }))
}

fn mcp_tools_list(id: Value) -> Value {
    let reg = get_registry();
    let mut tools: Vec<Value> = Vec::new();
    let mut entries = reg.all();
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    for entry in &entries {
        if entry.path == "sys.mcp" || entry.path == "kernel.main" {
            continue;
        }

        let tool_name = entry.path.replace('.', "_");

        // Build inputSchema from params (Vec<Value> of {name, type, description, required})
        let mut properties = serde_json::Map::new();
        let mut required: Vec<Value> = Vec::new();
        for p in &entry.params {
            let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("arg");
            let desc = p.get("description").and_then(|d| d.as_str()).unwrap_or("");
            let ptype = p.get("type").and_then(|t| t.as_str()).unwrap_or("string");
            let is_required = p.get("required").and_then(|r| r.as_bool()).unwrap_or(true);

            let schema_type = match ptype {
                "int" | "integer" => "integer",
                "float" | "number" => "number",
                "bool" | "boolean" => "boolean",
                _ => "string",
            };
            let mut prop = serde_json::Map::new();
            prop.insert("type".to_string(), serde_json::json!(schema_type));
            if !desc.is_empty() {
                prop.insert("description".to_string(), serde_json::json!(desc));
            }
            properties.insert(name.to_string(), Value::Object(prop));
            if is_required {
                required.push(serde_json::json!(name));
            }
        }

        let mut schema = serde_json::Map::new();
        schema.insert("type".to_string(), serde_json::json!("object"));
        schema.insert("properties".to_string(), Value::Object(properties));
        if !required.is_empty() {
            schema.insert("required".to_string(), Value::Array(required));
        }

        tools.push(serde_json::json!({
            "name": tool_name,
            "description": entry.description,
            "inputSchema": Value::Object(schema)
        }));
    }

    mcp_result(id, serde_json::json!({ "tools": tools }))
}

fn mcp_tools_call(id: Value, request: &Value) -> Value {
    let params = match request.get("params") {
        Some(p) => p,
        None => return mcp_error(id, -32602, "Missing params"),
    };

    let tool_name = match params.get("name").and_then(|n| n.as_str()) {
        Some(n) => n,
        None => return mcp_error(id, -32602, "Missing tool name"),
    };

    let trait_path = tool_name.replace('_', ".");
    let reg = get_registry();
    let entry = match reg.get(&trait_path) {
        Some(e) => e,
        None => return mcp_error(id, -32602, &format!("Unknown tool: {}", tool_name)),
    };

    // Build ordered arg array from arguments object
    let arguments = params.get("arguments").and_then(|a| a.as_object());
    let args: Vec<Value> = entry.params.iter().map(|p| {
        let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
        arguments.and_then(|a| a.get(name)).cloned().unwrap_or(Value::Null)
    }).collect();

    // Try WASM dispatch first, then platform dispatch (which covers WASM too,
    // but wasm_traits::dispatch is the canonical WASM path)
    let result = wasm_traits::dispatch(&trait_path, &args)
        .or_else(|| kernel_logic::platform::dispatch(&trait_path, &args));

    match result {
        Some(value) => {
            let text = match &value {
                Value::String(s) => s.clone(),
                other => serde_json::to_string_pretty(other).unwrap_or_default(),
            };
            mcp_result(id, serde_json::json!({
                "content": [{ "type": "text", "text": text }]
            }))
        }
        None => mcp_error(id, -32602, &format!("Dispatch failed for: {}", trait_path)),
    }
}

fn mcp_result(id: Value, result: Value) -> Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn mcp_error(id: Value, code: i32, message: &str) -> Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
