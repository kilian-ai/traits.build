//! Platform abstraction layer.
//!
//! Provides a unified API for platform-specific capabilities (dispatch, registry,
//! config, secrets, time) so trait source files can be platform-agnostic.
//!
//! - **Compile-time**: `time` module uses cfg-gated implementations (zero overhead).
//! - **Runtime**: `Platform` struct with function pointers, initialized once at startup
//!   via `init()`. Native binary and WASM kernel each provide their own adapters.

pub mod time;

use serde_json::Value;
use std::sync::OnceLock;

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
