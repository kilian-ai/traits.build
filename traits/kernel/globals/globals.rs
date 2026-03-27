//! Global statics for trait implementations.
//!
//! Trait .rs files are compiled as submodules of stable_traits and need access
//! to the Registry, Config, etc. These OnceLock statics are set during
//! bootstrap and accessed by trait implementations at call time.

use crate::config::Config;
use crate::registry::Registry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;
use tokio::sync::Mutex;
use std::sync::Arc;

pub static REGISTRY: OnceLock<Registry> = OnceLock::new();
pub static TRAITS_DIR: OnceLock<PathBuf> = OnceLock::new();
pub static CONFIG: OnceLock<Config> = OnceLock::new();
pub static START_TIME: OnceLock<Instant> = OnceLock::new();
pub static HANDLES: OnceLock<Arc<Mutex<HashMap<String, HandleEntry>>>> = OnceLock::new();

/// Server state — set by sys.serve when HTTP server starts.
pub static SERVER_BIND: OnceLock<String> = OnceLock::new();
pub static SERVER_PORT: OnceLock<u16> = OnceLock::new();

/// Relay connection state — updated by sys.serve when relay client connects.
pub static RELAY_URL: OnceLock<String> = OnceLock::new();
pub static RELAY_CODE: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);
pub static RELAY_CONNECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Opaque handle storage entry
pub struct HandleEntry {
    pub type_name: String,
    pub summary: String,
    pub created: f64,
}

pub fn now_epoch() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn format_uptime(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

/// Returns uptime in seconds, or 0.0 if not yet initialized.
pub fn uptime_secs() -> f64 {
    START_TIME.get().map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0)
}

/// Returns true if globals have been initialized (registry is set).
pub fn is_initialized() -> bool {
    REGISTRY.get().is_some()
}

// ── Trait dispatch entry point ──

/// kernel.globals introspection: returns global state summary.
pub fn globals(args: &[serde_json::Value]) -> serde_json::Value {
    let _ = args; // no params
    let uptime = START_TIME.get()
        .map(|t| t.elapsed().as_secs_f64())
        .unwrap_or(0.0);
    serde_json::json!({
        "registry_initialized": REGISTRY.get().is_some(),
        "config_initialized": CONFIG.get().is_some(),
        "traits_dir": TRAITS_DIR.get().map(|p| p.display().to_string()),
        "handles_initialized": HANDLES.get().is_some(),
        "uptime_seconds": uptime,
        "uptime_human": format_uptime(uptime),
        "epoch": now_epoch()
    })
}

/// Initialize all globals. Called once during bootstrap.
pub fn init(registry: Registry, traits_dir: PathBuf, config: Config) {
    let _ = REGISTRY.set(registry);
    let _ = TRAITS_DIR.set(traits_dir);
    let _ = CONFIG.set(config);
    let _ = START_TIME.set(Instant::now());
    let _ = HANDLES.set(Arc::new(Mutex::new(HashMap::new())));
}
