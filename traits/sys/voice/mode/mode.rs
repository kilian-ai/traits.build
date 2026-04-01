use serde_json::{json, Value};

/// Voice mode management — get/set preferred voice mode and check API key availability.
///
/// Actions:
///   get         → returns current mode ("local", "realtime", or "local-realtime") and whether API key exists
///   set <mode>  → set preferred mode to "local", "realtime", or "local-realtime"
///   has_key     → check if OpenAI API key is available (WASM: always false, resolved by JS bridge)
///
/// In WASM, mode is stored in localStorage. The JS bridge resolves has_key from secrets.
pub fn mode(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("get");

    match action {
        "get" => {
            // Return action descriptor — JS bridge resolves localStorage + key check
            json!({
                "ok": true,
                "voice_mode_action": "get"
            })
        }
        "set" => {
            let new_mode = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            if !["local", "realtime", "local-realtime"].contains(&new_mode) {
                return json!({"ok": false, "error": "mode must be 'local', 'realtime', or 'local-realtime'"});
            }
            json!({
                "ok": true,
                "voice_mode_action": "set",
                "mode": new_mode
            })
        }
        "has_key" => {
            // In WASM: returns descriptor, JS bridge checks localStorage/secrets
            json!({
                "ok": true,
                "voice_mode_action": "has_key"
            })
        }
        _ => json!({
            "ok": false,
            "error": format!("Unknown action: {}. Use: get, set, has_key", action)
        }),
    }
}
