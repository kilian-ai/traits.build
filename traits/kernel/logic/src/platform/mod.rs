//! Platform abstraction layer.
//!
//! Provides a unified API for platform-specific capabilities (dispatch, registry,
//! config, secrets, time, task tracking) so trait source files can be platform-agnostic.
//!
//! - **Compile-time**: `time` module uses cfg-gated implementations (zero overhead).
//! - **Runtime**: `Platform` struct with function pointers, initialized once at startup
//!   via `init()`. Native binary and WASM kernel each provide their own adapters.
//! - **Task registry**: In-memory registry for background tasks/services visible in `sys.ps`.
//!   Shared across both native and WASM — any code can register tasks.

pub mod time;

use crate::vfs;
use serde_json::Value;
use std::sync::{Mutex, OnceLock};

/// Platform adapter — runtime-initialized platform services.
///
/// All fields are function pointers (zero-capture closures or bare function refs).
/// Set once at startup via [`init()`].
pub struct Platform {
    /// Dispatch a trait call by path. Returns None if trait not found.
    pub dispatch: fn(&str, &[Value]) -> Option<Value>,
    /// Return all registered traits as JSON summary objects.
    pub registry_all: fn() -> Vec<Value>,
    /// Count of registered traits.
    pub registry_count: fn() -> usize,
    /// Detailed JSON for a single trait, or None if not found.
    pub registry_detail: fn(&str) -> Option<Value>,
    /// Read a per-trait config value, returning default if absent.
    pub config_get: fn(&str, &str, &str) -> String,
    /// Retrieve a stored secret by key.
    pub secret_get: fn(&str) -> Option<String>,
    /// Create a new VFS backend for a CLI session.
    /// - Native: `LayeredVfs` seeded by walking the real `TRAITS_DIR` on disk.
    /// - WASM:   `LayeredVfs` seeded from `include_str!` embedded assets.
    /// Falls back to `MemVfs` if this field is not set (pre-init callers).
    pub make_vfs: fn() -> Box<dyn vfs::Vfs>,
    /// Return the platform-specific process/task status for `sys.ps`.
    /// - Native: scans `.run/*.pid` files for background trait processes.
    /// - WASM:   reports kernel runtime state (callable traits, dispatch cascade).
    pub background_tasks: fn() -> Value,
    /// Read a file from the persistent VFS.
    /// - Native: checks `data/vfs/` then project root.
    /// - WASM:   reads from global VFS (seeded from builtins + localStorage).
    pub vfs_read: fn(&str) -> Option<String>,
    /// Write a file to the persistent VFS.
    /// - Native: writes to `data/vfs/`.
    /// - WASM:   writes to global VFS + auto-persists to localStorage.
    pub vfs_write: fn(&str, &str),
    /// List files in the persistent VFS (user-written files).
    /// - Native: walks `data/vfs/` directory.
    /// - WASM:   lists all VFS entries (builtins + user layer).
    pub vfs_list: fn() -> Vec<String>,
    /// Delete a file from the persistent VFS.
    pub vfs_delete: fn(&str) -> bool,
}

static PLATFORM: OnceLock<Platform> = OnceLock::new();

/// Initialize the platform layer. Call once at startup after registry init.
pub fn init(p: Platform) {
    let _ = PLATFORM.set(p);
}

/// Whether the platform has been initialized.
pub fn is_initialized() -> bool {
    PLATFORM.get().is_some()
}

fn platform() -> &'static Platform {
    PLATFORM.get().expect("kernel_logic::platform::init() not called")
}

// ── Convenience accessors ──

/// Dispatch a trait call. Returns None if trait not found.
pub fn dispatch(path: &str, args: &[Value]) -> Option<Value> {
    (platform().dispatch)(path, args)
}

/// All registered traits as JSON summary objects.
pub fn registry_all() -> Vec<Value> {
    (platform().registry_all)()
}

/// Count of registered traits.
pub fn registry_count() -> usize {
    (platform().registry_count)()
}

/// Detailed JSON for a single trait.
pub fn registry_detail(path: &str) -> Option<Value> {
    (platform().registry_detail)(path)
}

/// Read a per-trait config value with fallback default.
pub fn config_get(trait_path: &str, key: &str, default: &str) -> String {
    (platform().config_get)(trait_path, key, default)
}

/// Retrieve a stored secret by key.
pub fn secret_get(key: &str) -> Option<String> {
    (platform().secret_get)(key)
}

/// Create a VFS backend for a new CLI session.
/// Returns `MemVfs` if the platform has not been initialised yet.
pub fn make_vfs() -> Box<dyn vfs::Vfs> {
    PLATFORM.get()
        .map(|p| (p.make_vfs)())
        .unwrap_or_else(|| Box::new(vfs::MemVfs::default()))
}

/// Read a file from the persistent VFS.
pub fn vfs_read(path: &str) -> Option<String> {
    PLATFORM.get().and_then(|p| (p.vfs_read)(path))
}

/// Write a file to the persistent VFS.
pub fn vfs_write(path: &str, content: &str) {
    if let Some(p) = PLATFORM.get() {
        (p.vfs_write)(path, content);
    }
}

/// List files in the persistent VFS.
pub fn vfs_list() -> Vec<String> {
    PLATFORM.get().map(|p| (p.vfs_list)()).unwrap_or_default()
}

/// Delete a file from the persistent VFS.
pub fn vfs_delete(path: &str) -> bool {
    PLATFORM.get().map(|p| (p.vfs_delete)(path)).unwrap_or(false)
}

/// Return platform-specific process/task status (complete JSON for `sys.ps`).
pub fn background_tasks() -> Value {
    (platform().background_tasks)()
}

// ── In-memory task registry ──

/// A background task/service registered in the platform layer.
///
/// Visible in `sys.ps` output on both native and WASM.
/// - Native: merged with PID-file scan results.
/// - WASM: the sole source of "processes".
pub struct Task {
    pub id: String,
    /// Display name (e.g. "Terminal", "Web Worker #0", "sys.serve")
    pub name: String,
    /// "service" | "worker" | "task"
    pub task_type: String,
    /// "running" | "idle" | "done" | "stopped"
    pub status: String,
    /// Monotonic start time in seconds since platform init (native),
    /// or Date.now() milliseconds from JS (WASM).
    pub started: f64,
    /// Optional extra info (adapter name, trait path, etc.)
    pub detail: String,
}

static TASK_REGISTRY: Mutex<Vec<Task>> = Mutex::new(Vec::new());

/// Register a background task/service visible in `sys.ps`.
///
/// Idempotent: if a task with the same `id` already exists, updates its
/// status and detail instead of creating a duplicate.
pub fn register_task(id: &str, name: &str, task_type: &str, started: f64, detail: &str) {
    if let Ok(mut tasks) = TASK_REGISTRY.lock() {
        if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
            t.status = "running".to_string();
            t.detail = detail.to_string();
            return;
        }
        tasks.push(Task {
            id: id.to_string(),
            name: name.to_string(),
            task_type: task_type.to_string(),
            status: "running".to_string(),
            started,
            detail: detail.to_string(),
        });
    }
}

/// Remove a task from the registry.
pub fn unregister_task(id: &str) {
    if let Ok(mut tasks) = TASK_REGISTRY.lock() {
        tasks.retain(|t| t.id != id);
    }
}

/// Update the status of a registered task.
pub fn update_task_status(id: &str, status: &str) {
    if let Ok(mut tasks) = TASK_REGISTRY.lock() {
        if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
            t.status = status.to_string();
        }
    }
}

/// Return all registered tasks as JSON array.
pub fn list_tasks() -> Vec<Value> {
    let Ok(tasks) = TASK_REGISTRY.lock() else { return vec![] };
    tasks.iter().map(|t| {
        serde_json::json!({
            "id": t.id,
            "name": t.name,
            "type": t.task_type,
            "status": t.status,
            "started": t.started,
            "detail": t.detail,
        })
    }).collect()
}
