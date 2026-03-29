use serde_json::Value;

pub fn format_cli(result: &Value) -> String {
    let obj = match result.as_object() {
        Some(o) => o,
        None => return format!("{}\n", result),
    };

    // Error response
    if obj.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        let err = obj.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
        return format!("\x1b[31mError: {}\x1b[0m\n", err);
    }

    let current = obj.get("currentModelId").and_then(|v| v.as_str()).unwrap_or("");
    let models = match obj.get("availableModels").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return format!("{}\n", result),
    };

    if models.is_empty() {
        return "No models available.\n".to_string();
    }

    let mut out = String::new();
    out.push_str("\x1b[1m\x1b[97mAvailable Models\x1b[0m\n");
    for m in models {
        let id = m.get("modelId").and_then(|v| v.as_str()).unwrap_or("?");
        let name = m.get("name").and_then(|v| v.as_str()).unwrap_or(id);
        let usage = m.pointer("/_meta/copilotUsage").and_then(|v| v.as_str()).unwrap_or("");

        let is_current = id == current;
        let marker = if is_current { "\x1b[96m● " } else { "  " };
        let id_color = if is_current { "\x1b[96m" } else { "\x1b[0m" };
        let suffix = if usage.is_empty() {
            String::new()
        } else {
            format!(" \x1b[90m({})\x1b[0m", usage)
        };

        out.push_str(&format!("{}{}{}\x1b[0m{}\n", marker, id_color, id, suffix));
        if name != id {
            out.push_str(&format!("    \x1b[90m{}\x1b[0m\n", name));
        }
    }

    if !current.is_empty() {
        out.push_str(&format!("\n\x1b[90mCurrent: \x1b[96m{}\x1b[0m\n", current));
    }

    out
}
