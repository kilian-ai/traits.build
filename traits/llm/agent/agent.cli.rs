use serde_json::Value;

/// CLI formatter for llm.agent — shows the agent's final response text,
/// token usage, compaction stats, and tool call summary.
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

    // Usage summary
    if let Some(usage) = result.get("usage") {
        let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let output = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        if input > 0 || output > 0 {
            out.push_str(&format!("\nTokens: {} in / {} out\n", input, output));
        }
    }

    // Compaction info
    if let Some(compacted) = result.get("compacted_messages").and_then(|v| v.as_u64()) {
        if compacted > 0 {
            out.push_str(&format!("Compacted: {} messages\n", compacted));
        }
    }

    // Turn mode indicator
    if let Some(done) = result.get("done").and_then(|v| v.as_bool()) {
        if !done {
            out.push_str("Status: awaiting next turn (buddy mode)\n");
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
