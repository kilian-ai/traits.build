use serde_json::Value;

/// CLI formatter for llm.prompt.acp.cli
pub fn format_cli(result: &Value) -> String {
    let mut out = String::new();

    if let Some(desc) = result.get("description").and_then(|v| v.as_str()) {
        out.push_str(&format!("\x1b[1m{}\x1b[0m\n\n", desc));
    }

    if let Some(hint) = result.get("hint").and_then(|v| v.as_str()) {
        out.push_str(&format!("  {}\n\n", hint));
    }

    if let Some(agent) = result.get("agent").and_then(|v| v.as_str()) {
        let model = result.get("model").and_then(|v| v.as_str()).unwrap_or("(agent default)");
        out.push_str(&format!("  Agent: \x1b[96m{}\x1b[0m\n", agent));
        out.push_str(&format!("  Model: \x1b[96m{}\x1b[0m\n\n", model));
    }

    if let Some(cmds) = result.get("commands").and_then(|v| v.as_array()) {
        out.push_str("\x1b[1mCommands:\x1b[0m\n");
        for cmd in cmds {
            if let Some(s) = cmd.as_str() {
                out.push_str(&format!("  \x1b[32m{}\x1b[0m\n", s));
            }
        }
    }

    out
}
