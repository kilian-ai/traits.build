use serde_json::Value;

// ── sys.cli.wasm — WASM terminal CLI backend (native stub) ──
//
// Provides the cli/backend interface for the browser terminal.
//
// In the native binary, this trait is a metadata stub — the real
// implementation lives in the WASM module (kernel/wasm/src/lib.rs)
// as WasmCliBackend, which is compiled directly into the .wasm binary.
//
// This trait exists so the registry knows about the WASM backend:
// - Shows up in `traits list` and `traits info`
// - Declares `provides = ["cli/backend"]`
// - Documents the interface contract alongside sys.cli.native

/// Dispatch entry: in the native binary, returns backend metadata.
/// The actual WASM implementation is compiled into the WASM module.
pub fn wasm(args: &[Value]) -> Value {
    let method = args.first().and_then(|v| v.as_str()).unwrap_or("");

    match method {
        // In native binary, all methods return a "not available" indicator
        "call" | "list_all" | "get_info" | "search" | "all_paths" | "version"
        | "load_examples" => {
            serde_json::json!({
                "ok": false,
                "error": "sys.cli.wasm is only available in the WASM module (browser terminal)"
            })
        }
        // Default: introspection
        _ => serde_json::json!({
            "provides": "cli/backend",
            "target": "wasm",
            "note": "Actual implementation is compiled into the WASM module (kernel/wasm/src/lib.rs)",
            "methods": [
                "call", "list_all", "get_info", "search", "all_paths",
                "version", "load_examples"
            ],
            "excluded_methods": ["load_param_history", "save_param_history"],
            "excluded_reason": "WASM has no filesystem persistence"
        }),
    }
}
