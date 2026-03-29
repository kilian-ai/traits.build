use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// sys.chat — Chat session management.
///
/// Provides persistent session CRUD so chat can resume across invocations
/// and agents can manage their own sessions via MCP tools.
///
/// Sessions are stored as JSON files in `.chat/sessions/` under the working
/// directory or TRAITS_DIR, with a `current` pointer in sys.config.
///
/// Actions:
///   list                       — List all chat sessions  
///   current                    — Show the current (active) session  
///   get <session_id>           — Get full session data with messages  
///   new [agent] [model]        — Create a new session, make it current  
///   switch <session_id>        — Switch to an existing session  
///   delete <session_id>        — Delete a session  
///   append <session_id> <role> <content> — Append a message to a session  
///
/// Args: [action, session_id?, data?, extra?]
pub fn chat_session(args: &[Value]) -> Value {
    let action = match args.first().and_then(|v| v.as_str()) {
        Some(a) if !a.is_empty() => a,
        _ => return json!({"ok": false, "error": "action is required: list, current, get, new, switch, delete, append"}),
    };
    let arg1 = args.get(1).and_then(|v| v.as_str()).filter(|s| !s.is_empty());
    let arg2 = args.get(2).and_then(|v| v.as_str()).filter(|s| !s.is_empty());
    let arg3 = args.get(3).and_then(|v| v.as_str()).filter(|s| !s.is_empty());

    match action {
        "list" => action_list(),
        "current" => action_current(),
        "get" => {
            let sid = match arg1 {
                Some(s) => s,
                None => return json!({"ok": false, "error": "session_id is required"}),
            };
            action_get(sid)
        }
        "new" => {
            let agent = arg1.unwrap_or("opencode");
            let model = arg2.unwrap_or("");
            action_new(agent, model)
        }
        "switch" => {
            let sid = match arg1 {
                Some(s) => s,
                None => return json!({"ok": false, "error": "session_id is required"}),
            };
            action_switch(sid)
        }
        "delete" => {
            let sid = match arg1 {
                Some(s) => s,
                None => return json!({"ok": false, "error": "session_id is required"}),
            };
            action_delete(sid)
        }
        "append" => {
            let sid = match arg1 {
                Some(s) => s,
                None => return json!({"ok": false, "error": "session_id is required"}),
            };
            let role = match arg2 {
                Some(r) => r,
                None => return json!({"ok": false, "error": "role is required (user/assistant)"}),
            };
            let content = match arg3 {
                Some(c) => c,
                None => return json!({"ok": false, "error": "message content is required"}),
            };
            action_append(sid, role, content)
        }
        _ => json!({"ok": false, "error": format!("Unknown action: {action}. Use: list, current, get, new, switch, delete, append")}),
    }
}

// ═══════════════════════════════════════════
// ── Storage paths ──────────────────────────
// ═══════════════════════════════════════════

/// Base directory for chat sessions.
fn sessions_dir() -> PathBuf {
    // Use .chat/sessions/ in the working directory
    let base = if Path::new("/data").is_dir() {
        PathBuf::from("/data/.chat/sessions")
    } else {
        PathBuf::from(".chat/sessions")
    };
    let _ = std::fs::create_dir_all(&base);
    base
}

/// Path to a specific session file.
fn session_path(session_id: &str) -> PathBuf {
    // Sanitize session_id to prevent path traversal
    let safe_id: String = session_id.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
        .collect();
    sessions_dir().join(format!("{safe_id}.json"))
}

/// Read the current session ID from sys.config.
fn get_current_session_id() -> Option<String> {
    let val = kernel_logic::platform::config_get("sys.chat", "current_session", "");
    if val.is_empty() { None } else { Some(val) }
}

/// Write the current session ID to sys.config.
fn set_current_session_id(session_id: &str) {
    kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("set"), json!("sys.chat"), json!("current_session"), json!(session_id)],
    );
}

// ═══════════════════════════════════════════
// ── Session data model ─────────────────────
// ═══════════════════════════════════════════

/// Read a session from disk. Returns (metadata, messages) JSON.
fn read_session(session_id: &str) -> Option<Value> {
    let path = session_path(session_id);
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write a session to disk.
fn write_session(session_id: &str, data: &Value) -> Result<(), String> {
    let path = session_path(session_id);
    let content = serde_json::to_string_pretty(data)
        .map_err(|e| format!("JSON serialize error: {e}"))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

// ═══════════════════════════════════════════
// ── Actions ────────────────────────────────
// ═══════════════════════════════════════════

fn action_list() -> Value {
    let dir = sessions_dir();
    let mut sessions: Vec<Value> = Vec::new();
    let current = get_current_session_id();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = match path.file_name().and_then(|f| f.to_str()) {
                Some(f) if f.ends_with(".json") => f.to_string(),
                _ => continue,
            };
            let sid = fname.trim_end_matches(".json").to_string();

            // Read session metadata without loading all messages
            let meta = if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(data) = serde_json::from_str::<Value>(&content) {
                    let agent = data.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
                    let model = data.get("model").and_then(|v| v.as_str()).unwrap_or("");
                    let msg_count = data.get("messages").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                    let created = data.get("created").and_then(|v| v.as_str()).unwrap_or(&sid);
                    json!({
                        "session_id": sid,
                        "agent": agent,
                        "model": model,
                        "messages": msg_count,
                        "created": created,
                        "current": current.as_deref() == Some(sid.as_str()),
                    })
                } else {
                    json!({"session_id": sid, "messages": 0})
                }
            } else {
                json!({"session_id": sid, "messages": 0})
            };
            sessions.push(meta);
        }
    }

    // Sort by session_id (timestamp-based) descending (newest first)
    sessions.sort_by(|a, b| {
        let sa = a.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
        let sb = b.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
        sb.cmp(sa)
    });

    json!({"ok": true, "sessions": sessions, "count": sessions.len()})
}

fn action_current() -> Value {
    match get_current_session_id() {
        Some(sid) => {
            match read_session(&sid) {
                Some(data) => {
                    let agent = data.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
                    let model = data.get("model").and_then(|v| v.as_str()).unwrap_or("");
                    let msg_count = data.get("messages").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                    json!({
                        "ok": true,
                        "session_id": sid,
                        "agent": agent,
                        "model": model,
                        "messages": msg_count,
                    })
                }
                None => json!({"ok": true, "session_id": null, "info": "Last session not found on disk; a new one will be created"}),
            }
        }
        None => json!({"ok": true, "session_id": null, "info": "No current session; a new one will be created on next chat start"}),
    }
}

fn action_get(session_id: &str) -> Value {
    match read_session(session_id) {
        Some(data) => json!({"ok": true, "session": data}),
        None => json!({"ok": false, "error": format!("Session not found: {session_id}")}),
    }
}

fn action_new(agent: &str, model: &str) -> Value {
    let (y, mo, d, h, m, s) = kernel_logic::platform::time::now_utc();
    let session_id = format!("{y:04}{mo:02}{d:02}_{h:02}{m:02}{s:02}");

    let data = json!({
        "session_id": session_id,
        "agent": agent,
        "model": model,
        "created": session_id,
        "messages": [],
    });

    if let Err(e) = write_session(&session_id, &data) {
        return json!({"ok": false, "error": e});
    }

    set_current_session_id(&session_id);

    json!({
        "ok": true,
        "session_id": session_id,
        "agent": agent,
        "model": model,
    })
}

fn action_switch(session_id: &str) -> Value {
    let path = session_path(session_id);
    if !path.exists() {
        return json!({"ok": false, "error": format!("Session not found: {session_id}")});
    }

    set_current_session_id(session_id);

    match read_session(session_id) {
        Some(data) => {
            let agent = data.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
            let model = data.get("model").and_then(|v| v.as_str()).unwrap_or("");
            let msg_count = data.get("messages").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
            json!({
                "ok": true,
                "session_id": session_id,
                "agent": agent,
                "model": model,
                "messages": msg_count,
            })
        }
        None => json!({"ok": true, "session_id": session_id}),
    }
}

fn action_delete(session_id: &str) -> Value {
    let path = session_path(session_id);
    if !path.exists() {
        return json!({"ok": false, "error": format!("Session not found: {session_id}")});
    }

    if let Err(e) = std::fs::remove_file(&path) {
        return json!({"ok": false, "error": format!("Failed to delete: {e}")});
    }

    // If this was the current session, clear the pointer
    if get_current_session_id().as_deref() == Some(session_id) {
        set_current_session_id("");
    }

    json!({"ok": true, "deleted": session_id})
}

fn action_append(session_id: &str, role: &str, content: &str) -> Value {
    // Validate role
    if role != "user" && role != "assistant" && role != "system" {
        return json!({"ok": false, "error": "role must be user, assistant, or system"});
    }

    let path = session_path(session_id);
    let mut data = if path.exists() {
        match read_session(session_id) {
            Some(d) => d,
            None => return json!({"ok": false, "error": "Failed to read session"}),
        }
    } else {
        // Auto-create session if it doesn't exist
        json!({
            "session_id": session_id,
            "agent": "opencode",
            "model": "",
            "created": session_id,
            "messages": [],
        })
    };

    // Append message
    if let Some(msgs) = data.get_mut("messages").and_then(|v| v.as_array_mut()) {
        msgs.push(json!({"role": role, "content": content}));
    }

    if let Err(e) = write_session(session_id, &data) {
        return json!({"ok": false, "error": e});
    }

    let msg_count = data.get("messages").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    json!({"ok": true, "session_id": session_id, "messages": msg_count})
}
