use serde_json::Value;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const MAGENTA: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";

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
            let rendered = render_markdown_for_terminal(response);
            out.push_str(&rendered);
            if !rendered.ends_with('\n') {
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

fn render_markdown_for_terminal(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::new();
    let mut in_code = false;
    let mut fence_lang = String::new();

    for raw_line in normalized.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            if !in_code {
                in_code = true;
                fence_lang = trimmed.trim_start_matches("```").trim().to_string();
                if !fence_lang.is_empty() {
                    out.push_str(&format!("{MAGENTA}{BOLD}{} code:{RESET}\n", fence_lang));
                }
            } else {
                in_code = false;
                fence_lang.clear();
                out.push('\n');
            }
            continue;
        }

        if in_code {
            out.push_str(&format!("{DIM}  {}{RESET}\n", line));
            continue;
        }

        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }

        if trimmed.starts_with("# ") {
            out.push_str(&format!("{CYAN}{BOLD}{}{RESET}\n", &trimmed[2..]));
            continue;
        }
        if trimmed.starts_with("## ") {
            out.push_str(&format!("{CYAN}{BOLD}{}{RESET}\n", &trimmed[3..]));
            continue;
        }

        let list_line = if let Some(rest) = trimmed.strip_prefix("- ") {
            format!("{GREEN}*{RESET} {}", render_inline_code(rest))
        } else {
            render_inline_code(trimmed)
        };
        out.push_str(&list_line);
        out.push('\n');
    }

    to_crlf(out.trim_end_matches('\n'))
}

fn to_crlf(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

fn render_inline_code(line: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    let mut buf = String::new();

    for ch in line.chars() {
        if ch == '`' {
            if in_code {
                out.push_str(&format!("{YELLOW}{}{RESET}", buf));
                buf.clear();
                in_code = false;
            } else {
                if !buf.is_empty() {
                    out.push_str(&buf);
                    buf.clear();
                }
                in_code = true;
            }
            continue;
        }
        buf.push(ch);
    }

    if !buf.is_empty() {
        if in_code {
            out.push_str(&format!("{YELLOW}`{}{RESET}", buf));
        } else {
            out.push_str(&buf);
        }
    }

    out
}
