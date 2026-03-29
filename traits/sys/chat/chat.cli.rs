use serde_json::Value;

/// CLI output formatter for sys.chat
pub fn format_cli(result: &Value) -> String {
    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if !ok {
        let err = result.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
        return format!("\x1b[31mError: {err}\x1b[0m\n");
    }

    let mut out = String::new();

    // ── list action ──
    if let Some(sessions) = result.get("sessions").and_then(|v| v.as_array()) {
        if sessions.is_empty() {
            out.push_str("\x1b[90mNo saved sessions\x1b[0m\n");
        } else {
            out.push_str("\x1b[1mSessions:\x1b[0m\n");
            for s in sessions {
                let sid = s.get("session_id").and_then(|v| v.as_str()).unwrap_or("?");
                let agent = s.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
                let mc = s.get("messages").and_then(|v| v.as_u64()).unwrap_or(0);
                let current = s.get("current").and_then(|v| v.as_bool()).unwrap_or(false);
                let marker = if current { " ◀" } else { "" };
                out.push_str(&format!("  \x1b[36m{sid}\x1b[0m \x1b[90m{agent} ({mc} msgs){marker}\x1b[0m\n"));
            }
        }
        return out;
    }

    // ── get action ──
    if let Some(session) = result.get("session") {
        let sid = session.get("session_id").and_then(|v| v.as_str()).unwrap_or("?");
        let agent = session.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
        let model = session.get("model").and_then(|v| v.as_str()).unwrap_or("");
        let msgs = session.get("messages").and_then(|v| v.as_array());
        let mc = msgs.map(|a| a.len()).unwrap_or(0);
        out.push_str(&format!("\x1b[1mSession:\x1b[0m \x1b[36m{sid}\x1b[0m \x1b[90m({agent}"));
        if !model.is_empty() {
            out.push_str(&format!(", {model}"));
        }
        out.push_str(&format!(", {mc} msgs)\x1b[0m\n"));
        if let Some(messages) = msgs {
            for msg in messages {
                let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("?");
                let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let (color, label) = match role {
                    "user" => ("\x1b[32m", "You"),
                    "assistant" => ("\x1b[36m", "AI"),
                    _ => ("\x1b[90m", role),
                };
                out.push_str(&format!("{color}\x1b[1m{label}:\x1b[0m {content}\n"));
            }
        }
        return out;
    }

    // ── current / new / switch / append / delete ──
    if let Some(sid) = result.get("session_id") {
        if sid.is_null() {
            if let Some(info) = result.get("info").and_then(|v| v.as_str()) {
                out.push_str(&format!("\x1b[90m{info}\x1b[0m\n"));
            }
            return out;
        }
        let sid = sid.as_str().unwrap_or("?");
        let agent = result.get("agent").and_then(|v| v.as_str());
        let mc = result.get("messages").and_then(|v| v.as_u64());
        out.push_str(&format!("\x1b[36m{sid}\x1b[0m"));
        if let Some(a) = agent {
            out.push_str(&format!(" \x1b[90m({a}"));
            if let Some(m) = mc {
                out.push_str(&format!(", {m} msgs"));
            }
            out.push_str("\x1b[90m)\x1b[0m");
        }
        out.push('\n');
        return out;
    }

    if let Some(deleted) = result.get("deleted").and_then(|v| v.as_str()) {
        out.push_str(&format!("\x1b[32mDeleted: {deleted}\x1b[0m\n"));
        return out;
    }

    // Fallback: pretty-print JSON
    out.push_str(&serde_json::to_string_pretty(result).unwrap_or_default());
    out.push('\n');
    out
}
