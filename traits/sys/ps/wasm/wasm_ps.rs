use serde_json::{json, Value};

/// sys.ps.wasm — WASM runtime process status.
///
/// In the browser WASM context there are no OS threads or processes.
/// Instead, this reports the WASM kernel's runtime state:
/// - How many traits are registered vs WASM-callable
/// - The dispatch cascade status (WASM / helper / REST / none)
/// - An empty processes array (no OS processes exist in WASM)
///
/// This gives the user visibility into *why* `ps` behaves differently
/// in the browser compared to the native binary.
pub fn wasm_ps(_args: &[Value]) -> Value {
    let callable: Vec<&str> = super::WASM_CALLABLE.to_vec();
    let registered = crate::get_registry().len();

    json!({
        "ok": true,
        "runtime": "wasm",
        "processes": [],
        "note": "WASM runs in a single-threaded browser context — no OS processes or background threads exist.",
        "wasm": {
            "callable": callable.len(),
            "registered": registered,
            "traits": callable,
            "threading": "single-threaded (browser main thread)",
            "dispatch_cascade": [
                "1. WASM local (instant, in-browser)",
                "2. Local helper (localhost, if running)",
                "3. Server REST (if origin has backend)",
            ],
        },
    })
}
