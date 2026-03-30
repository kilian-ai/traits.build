use serde_json::{json, Value};
use std::io::Write;

/// Max notes to keep (older ones must be explicitly removed).
const MAX_NOTES: usize = 50;
/// Max length of a single note.
const MAX_NOTE_LEN: usize = 500;

/// sys.voice.memory — persistent cross-session memory for the voice model.
///
/// The model uses this to store facts about the user, project context,
/// preferences, or anything it wants to remember across voice sessions.
/// All notes are automatically loaded into the system instructions.
///
/// Actions:
///   add <text>    — store a new note
///   list          — list all notes
///   remove <id>   — remove a note by its numeric ID
///   clear         — remove all notes
pub fn voice_memory(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let text = args.get(1).and_then(|v| v.as_str()).unwrap_or("");

    match action {
        "add" => {
            if text.is_empty() {
                return json!({ "ok": false, "error": "text is required for add" });
            }
            if text.len() > MAX_NOTE_LEN {
                return json!({ "ok": false, "error": format!("Note too long (max {} chars)", MAX_NOTE_LEN) });
            }
            let mut notes = load_notes();
            if notes.len() >= MAX_NOTES {
                return json!({ "ok": false, "error": format!("Memory full ({} notes max). Remove some first.", MAX_NOTES) });
            }
            let id = next_id(&notes);
            let created = now_stamp();
            notes.push(json!({ "id": id, "text": text, "created": created }));
            save_notes(&notes);
            json!({ "ok": true, "action": "add", "id": id, "count": notes.len() })
        }
        "list" => {
            let notes = load_notes();
            json!({ "ok": true, "notes": notes, "count": notes.len() })
        }
        "remove" => {
            if text.is_empty() {
                return json!({ "ok": false, "error": "note ID is required for remove" });
            }
            let target_id: u64 = match text.parse() {
                Ok(id) => id,
                Err(_) => return json!({ "ok": false, "error": "ID must be a number" }),
            };
            let mut notes = load_notes();
            let before = notes.len();
            notes.retain(|n| n.get("id").and_then(|v| v.as_u64()) != Some(target_id));
            if notes.len() == before {
                return json!({ "ok": false, "error": format!("Note {} not found", target_id) });
            }
            save_notes(&notes);
            json!({ "ok": true, "action": "remove", "removed_id": target_id, "count": notes.len() })
        }
        "clear" => {
            save_notes(&Vec::new());
            json!({ "ok": true, "action": "clear", "count": 0 })
        }
        _ => json!({ "ok": false, "error": format!("Unknown action: '{}'. Use add, list, remove, or clear.", action) }),
    }
}

/// Read all memory notes for use in instructions.
pub fn read_memory_notes() -> Vec<String> {
    load_notes()
        .iter()
        .filter_map(|n| n.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect()
}

// ── Storage ──

fn memory_path() -> std::path::PathBuf {
    let base = if std::path::Path::new("/data").is_dir() {
        std::path::PathBuf::from("/data")
    } else {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    };
    base.join("voice_memory.json")
}

fn load_notes() -> Vec<Value> {
    let path = memory_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_notes(notes: &[Value]) {
    let path = memory_path();
    if let Ok(json) = serde_json::to_string_pretty(notes) {
        if let Ok(mut f) = std::fs::File::create(&path) {
            let _ = f.write_all(json.as_bytes());
        }
    }
}

fn next_id(notes: &[Value]) -> u64 {
    notes
        .iter()
        .filter_map(|n| n.get("id").and_then(|v| v.as_u64()))
        .max()
        .unwrap_or(0)
        + 1
}

fn now_stamp() -> String {
    let (y, mo, d, h, mi, s) = kernel_logic::platform::time::now_utc();
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}
