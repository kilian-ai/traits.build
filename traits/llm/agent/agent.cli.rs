use serde_json::Value;

/// CLI formatter for llm.agent — shows the agent's final response text.
/// On error, shows the error message. When run with --verbose, shows tool calls too.
pub fn format_cli(result: &Value) -> String {
    // Error case
    if let Some(err) = result.get("error").and_then(|v| v.as_str()) {
        return format!("Error: {}\n", err);
    }

    let mut out = String::new();

    // Final response text
    if let Some(response) = result.get("response").and_then(|v| v.as_str()) {
        if !response.is_empty() {
            out.push_str(response);
            if !response.ends_with('\n') {
                out.push('\n');
            }
        }
    }

    // Summary of tool calls (if any)
    if let Some(tool_calls) = result.get("tool_calls").and_then(|v| v.as_array()) {
        if !tool_calls.is_empty() {
            out.push('\n');
            for tc in tool_calls {
                let name = tc.get("trait").and_then(|v| v.as_str()).unwrap_or("");
                let ok = tc.pointer("/result/ok").and_then(|v| v.as_bool()).unwrap_or(true);
                let status = if ok { "✓" } else { "✗" };
                out.push_str(&format!("  {} {}\n", status, name));
            }
        }
    }

    out
}
