use serde_json::{json, Value};

/// The compiled-in default instructions.
const DEFAULT_INSTRUCTIONS: &str = include_str!("../realtime_instructions.md");

/// Config namespace for storing custom instructions.
const CONFIG_NS: &str = "sys.voice";
const CONFIG_KEY: &str = "custom_instructions";

/// sys.voice.instruct — read, replace, or reset the voice agent instructions.
///
/// Actions:
///   get    — return current instructions (custom if set, else default)
///   set    — replace instructions entirely with provided text
///   reset  — remove custom override, revert to compiled-in default
///   append — add text to the end of current instructions
pub fn voice_instruct(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let text = args.get(1).and_then(|v| v.as_str()).unwrap_or("");

    match action {
        "get" => {
            let (instructions, source) = read_instructions();
            json!({ "ok": true, "instructions": instructions, "source": source })
        }
        "set" => {
            if text.is_empty() {
                return json!({ "ok": false, "error": "text is required for set" });
            }
            if text.len() > 10000 {
                return json!({ "ok": false, "error": "Instructions too long (max 10000 chars)" });
            }
            write_instructions(text);
            json!({ "ok": true, "action": "set", "length": text.len(), "source": "custom" })
        }
        "reset" => {
            clear_instructions();
            json!({ "ok": true, "action": "reset", "source": "default", "length": DEFAULT_INSTRUCTIONS.len() })
        }
        "append" => {
            if text.is_empty() {
                return json!({ "ok": false, "error": "text is required for append" });
            }
            let (current, _) = read_instructions();
            let new_text = format!("{}\n\n{}", current, text);
            if new_text.len() > 10000 {
                return json!({ "ok": false, "error": "Combined instructions too long (max 10000 chars)" });
            }
            write_instructions(&new_text);
            json!({ "ok": true, "action": "append", "length": new_text.len(), "source": "custom" })
        }
        _ => json!({ "ok": false, "error": format!("Unknown action: '{}'. Use get, set, reset, or append.", action) }),
    }
}

/// Read current instructions. Returns (text, "custom"|"default").
pub fn read_instructions() -> (String, &'static str) {
    if let Some(custom) = kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("get"), json!(CONFIG_NS), json!(CONFIG_KEY)],
    ) {
        if let Some(val) = custom.get("value").and_then(|v| v.as_str()) {
            if !val.is_empty() {
                return (val.to_string(), "custom");
            }
        }
    }
    (DEFAULT_INSTRUCTIONS.to_string(), "default")
}

fn write_instructions(text: &str) {
    kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("set"), json!(CONFIG_NS), json!(CONFIG_KEY), json!(text)],
    );
}

fn clear_instructions() {
    kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("delete"), json!(CONFIG_NS), json!(CONFIG_KEY)],
    );
}
