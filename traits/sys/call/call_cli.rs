use serde_json::Value;

pub fn format_cli(value: &Value) -> String {
    let mut out = String::new();

    let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let status = value.get("status").and_then(|v| v.as_u64()).unwrap_or(0);

    let status_icon = if ok { "✓" } else { "✗" };
    out.push_str(&format!("{} HTTP {} {}\n", status_icon, status,
        if ok { "OK" } else { "Error" }));

    if let Some(body) = value.get("body") {
        // For OpenAI-style chat completions, show the message content directly
        if let Some(choices) = body.get("choices").and_then(|v| v.as_array()) {
            if let Some(first) = choices.first() {
                if let Some(content) = first.pointer("/message/content").and_then(|v| v.as_str()) {
                    out.push_str("\n");
                    out.push_str(content);
                    out.push_str("\n");

                    // Show token usage if available
                    if let Some(usage) = body.get("usage") {
                        let prompt = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let completion = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let total = usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        out.push_str(&format!("\n[tokens: {} prompt + {} completion = {} total]\n",
                            prompt, completion, total));
                    }
                    if let Some(model) = body.get("model").and_then(|v| v.as_str()) {
                        out.push_str(&format!("[model: {}]\n", model));
                    }
                    return out;
                }
            }
        }

        // For error responses, show the error message
        if let Some(err) = body.get("error") {
            if let Some(msg) = err.get("message").and_then(|v| v.as_str()) {
                out.push_str(&format!("\nError: {}\n", msg));
                return out;
            }
        }

        // Generic JSON body — pretty print (truncated)
        if body.is_string() {
            let s = body.as_str().unwrap();
            if s.len() > 500 {
                out.push_str(&format!("\n{}...\n", &s[..500]));
            } else {
                out.push_str(&format!("\n{}\n", s));
            }
        } else {
            let pretty = serde_json::to_string_pretty(body).unwrap_or_default();
            if pretty.len() > 2000 {
                out.push_str(&format!("\n{}...\n", &pretty[..2000]));
            } else {
                out.push_str(&format!("\n{}\n", pretty));
            }
        }
    }

    out
}
