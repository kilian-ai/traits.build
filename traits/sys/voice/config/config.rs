use serde_json::{json, Value};

/// Valid voice names for OpenAI Realtime API.
const VALID_VOICES: &[&str] = &[
    "alloy", "ash", "ballad", "coral", "echo", "sage", "shimmer", "verse", "marin", "cedar",
];

/// Valid realtime models.
const VALID_MODELS: &[&str] = &[
    "gpt-4o-realtime-preview", "gpt-realtime-mini-2025-12-15",
];

/// Config trait path used for persistent storage.
const CONFIG_TRAIT: &str = "sys.voice";

/// sys.voice.config — get or set persistent voice chat preferences.
///
/// Keys: voice, model, agent
/// All changes are stored via sys.config and become defaults for future sessions.
/// During a live voice session, changes to voice/agent take effect immediately.
pub fn voice_config(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let key = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let value = args.get(2).and_then(|v| v.as_str()).unwrap_or("");

    match action {
        "set" => set_pref(key, value),
        "get" => get_pref(key),
        _ => json!({"ok": false, "error": format!("Unknown action: '{}'. Use 'get' or 'set'.", action)}),
    }
}

fn set_pref(key: &str, value: &str) -> Value {
    if value.is_empty() {
        return json!({"ok": false, "error": "value is required for set"});
    }

    match key {
        "voice" => {
            if !VALID_VOICES.contains(&value) {
                return json!({
                    "ok": false,
                    "error": format!("Invalid voice '{}'. Valid: {}", value, VALID_VOICES.join(", "))
                });
            }
        }
        "model" => {
            if !VALID_MODELS.contains(&value) {
                return json!({
                    "ok": false,
                    "error": format!("Invalid model '{}'. Valid: {}", value, VALID_MODELS.join(", "))
                });
            }
        }
        "agent" => {
            // Any non-empty string is valid for agent
        }
        _ => {
            return json!({"ok": false, "error": format!("Unknown key '{}'. Valid keys: voice, model, agent", key)});
        }
    }

    // Read previous value
    let previous = read_config(key);

    // Persist via sys.config
    kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("set"), json!(CONFIG_TRAIT), json!(key), json!(value)],
    );

    json!({
        "ok": true,
        "action": "set",
        "key": key,
        "value": value,
        "previous": previous.unwrap_or_default(),
        "note": match key {
            "voice" => "Voice will change on the next response.",
            "model" => "Model change takes effect on next voice session.",
            "agent" => "Agent context updated for this and future sessions.",
            _ => "",
        }
    })
}

fn get_pref(key: &str) -> Value {
    match key {
        "voice" | "model" | "agent" => {
            let val = read_config(key);
            let default = match key {
                "voice" => "shimmer",
                "model" => "gpt-realtime-mini-2025-12-15",
                "agent" => "",
                _ => "",
            };
            json!({
                "ok": true,
                "key": key,
                "value": val.as_deref().unwrap_or(default),
                "source": if val.is_some() { "persistent" } else { "default" }
            })
        }
        "" => {
            // Return all preferences
            let voice = read_config("voice");
            let model = read_config("model");
            let agent = read_config("agent");
            json!({
                "ok": true,
                "voice": voice.as_deref().unwrap_or("shimmer"),
                "model": model.as_deref().unwrap_or("gpt-realtime-mini-2025-12-15"),
                "agent": agent.as_deref().unwrap_or(""),
            })
        }
        _ => json!({"ok": false, "error": format!("Unknown key '{}'. Valid keys: voice, model, agent (or omit for all)", key)}),
    }
}

/// Read a config value from persistent store.
pub fn read_config(key: &str) -> Option<String> {
    kernel_logic::platform::dispatch(
        "sys.config",
        &[serde_json::json!("get"), serde_json::json!(CONFIG_TRAIT), serde_json::json!(key)],
    )
    .and_then(|r| r.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()))
    .filter(|s| !s.is_empty())
}
