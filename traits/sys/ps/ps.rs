use serde_json::Value;

/// sys.ps — list running background tasks via the platform abstraction layer.
///
/// On native: scans `.run/*.pid` files, checks process alive status, reports
/// PID, uptime, and RSS memory.
///
/// On WASM: reports kernel runtime state — callable/registered trait counts,
/// helper connection status, dispatch cascade.
pub fn ps(_args: &[Value]) -> Value {
    kernel_logic::platform::background_tasks()
}
