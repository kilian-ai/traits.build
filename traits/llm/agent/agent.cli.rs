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
                let tool_name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let has_error = tc.pointer("/result/error").is_some();
                let has_ok = tc.pointer("/result/ok").is_some();
                let ok = if has_error {
                    false
                } else {
                    tc.pointer("/result/ok").and_then(|v| v.as_bool()).unwrap_or(!has_ok)
                };
                let status = if ok { "✓" } else { "✗" };
                out.push_str(&format!("  {} {}", status, name));

                // Show error detail on failure
                if !ok {
                    if let Some(err) = tc.pointer("/result/error").and_then(|v| v.as_str()) {
                        out.push_str(&format!(" — {}", err));
                    }
                }
                out.push('\n');

                // Debug trace: tool name mapping, args, result, humanized
                out.push_str(&format!("    tool_name: {} → trait: {}\n", tool_name, name));
                if let Some(args) = tc.get("args") {
                    out.push_str(&format!("    args: {}\n", args));
                }
                if let Some(pos_args) = tc.get("positional_args") {
                    out.push_str(&format!("    positional: {}\n", pos_args));
                }
                if let Some(result_val) = tc.get("result") {
                    out.push_str(&format!("    result: {}\n", result_val));
                }
                if let Some(humanized) = tc.get("humanized").and_then(|v| v.as_str()) {
                    out.push_str(&format!("    → model sees: \"{}\"\n", humanized));
                }
            }
        }
    }

    out
}
