use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use rusqlite::{types::ValueRef, Connection};

const JSON_METHOD: &str = "json";
const STATE_DB_METHOD: &str = "state_vscdb";

pub fn vscode(args: &[Value]) -> Value {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = args;
        return json!({
            "ok": false,
            "error": "sys.chat_protocols.vscode is not available in WASM"
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let workspace_id = match args.first().and_then(|value| value.as_str()).map(str::trim) {
            Some(value) if !value.is_empty() => value,
            _ => {
                return json!({
                    "ok": false,
                    "error": "workspace_id is required"
                });
            }
        };

        let method = args
            .get(1)
            .and_then(|value| value.as_str())
            .map(normalize_method)
            .unwrap_or_else(|| "auto".to_string());

        if !matches!(method.as_str(), "auto" | "all" | JSON_METHOD | STATE_DB_METHOD) {
            return json!({
                "ok": false,
                "error": format!(
                    "Unknown method: {}. Use auto, all, json, or state_vscdb",
                    method
                )
            });
        }

        let storage_root = match workspace_storage_root(args.get(2).and_then(|value| value.as_str())) {
            Ok(path) => path,
            Err(error) => {
                return json!({
                    "ok": false,
                    "error": error
                });
            }
        };

        let workspace_dir = storage_root.join(workspace_id);
        if !workspace_dir.is_dir() {
            return json!({
                "ok": false,
                "workspace_id": workspace_id,
                "storage_root": storage_root.display().to_string(),
                "error": format!("Workspace storage folder not found: {}", workspace_dir.display())
            });
        }

        let workspace_metadata = read_workspace_metadata(&workspace_dir);
        let json_result = if wants_method(&method, JSON_METHOD) {
            Some(read_json_logs(&workspace_dir))
        } else {
            None
        };
        let state_db_result = if wants_method(&method, STATE_DB_METHOD) {
            Some(read_state_vscdb(&workspace_dir))
        } else {
            None
        };

        let available_methods: Vec<&str> = [
            json_result
                .as_ref()
                .filter(|result| result["available"].as_bool().unwrap_or(false))
                .map(|_| JSON_METHOD),
            state_db_result
                .as_ref()
                .filter(|result| result["available"].as_bool().unwrap_or(false))
                .map(|_| STATE_DB_METHOD),
        ]
        .into_iter()
        .flatten()
        .collect();

        json!({
            "ok": true,
            "workspace_id": workspace_id,
            "requested_method": method,
            "storage_root": storage_root.display().to_string(),
            "workspace_storage_dir": workspace_dir.display().to_string(),
            "workspace": workspace_metadata,
            "available_methods": available_methods,
            "sources": {
                "json": json_result,
                "state_vscdb": state_db_result,
            },
            "notes": {
                "json": "Legacy chatSessions/*.json files usually contain closed or persisted transcript history.",
                "state_vscdb": {
                    "path": "workspaceStorage/<hash>/state.vscdb",
                    "why": "VS Code keeps active workspace chat state here before older JSON exports are updated.",
                    "update_rhythm": [
                        "Buffered writes are typically flushed on about a 60 second cadence.",
                        "Focus changes and other UI events can trigger earlier writes.",
                        "Shutdown persists the latest in-memory state."
                    ],
                    "warning": "Copy the SQLite file to a temporary path before querying it to avoid lock contention with VS Code."
                }
            }
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn normalize_method(method: &str) -> String {
    match method.trim().to_ascii_lowercase().as_str() {
        "db" | "sqlite" | "state" | "state-db" | "state_vscdb" => STATE_DB_METHOD.to_string(),
        "chat" | "chatsessions" | "chat_sessions" | "json" => JSON_METHOD.to_string(),
        other => other.to_string(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn wants_method(requested: &str, candidate: &str) -> bool {
    requested == "auto" || requested == "all" || requested == candidate
}

#[cfg(not(target_arch = "wasm32"))]
fn workspace_storage_root(override_root: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = override_root.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if let Ok(path) = env::var("VSCODE_WORKSPACESTORAGE_DIR") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = env::var("HOME").map_err(|_| "HOME is not set and no base_dir override was provided".to_string())?;

    let root = match env::consts::OS {
        "macos" => PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Code")
            .join("User")
            .join("workspaceStorage"),
        "windows" => {
            let appdata = env::var("APPDATA").map_err(|_| "APPDATA is not set and no base_dir override was provided".to_string())?;
            PathBuf::from(appdata).join("Code").join("User").join("workspaceStorage")
        }
        _ => PathBuf::from(home)
            .join(".config")
            .join("Code")
            .join("User")
            .join("workspaceStorage"),
    };

    Ok(root)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_workspace_metadata(workspace_dir: &Path) -> Value {
    let path = workspace_dir.join("workspace.json");
    read_json_file(&path).unwrap_or(Value::Null)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_json_logs(workspace_dir: &Path) -> Value {
    let chat_sessions_dir = workspace_dir.join("chatSessions");
    if !chat_sessions_dir.is_dir() {
        return json!({
            "available": false,
            "path": chat_sessions_dir.display().to_string(),
            "session_count": 0,
            "sessions": []
        });
    }

    let mut sessions = Vec::new();
    let mut files = read_sorted_json_files(&chat_sessions_dir);
    files.sort();

    for path in files {
        let data = match read_json_file(&path) {
            Ok(value) => value,
            Err(error) => {
                sessions.push(json!({
                    "file": path.display().to_string(),
                    "error": error
                }));
                continue;
            }
        };

        let request_count = data
            .get("requests")
            .and_then(Value::as_array)
            .map(|entries| entries.len())
            .unwrap_or(0);

        let title = data
            .get("requests")
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("message"))
            .and_then(|message| message.get("text"))
            .and_then(Value::as_str)
            .map(truncate_text);

        sessions.push(json!({
            "file": path.display().to_string(),
            "session_id": path.file_stem().and_then(|value| value.to_str()).unwrap_or_default(),
            "request_count": request_count,
            "title": title,
            "data": data
        }));
    }

    json!({
        "available": true,
        "path": chat_sessions_dir.display().to_string(),
        "session_count": sessions.len(),
        "sessions": sessions
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn read_state_vscdb(workspace_dir: &Path) -> Value {
    let db_path = workspace_dir.join("state.vscdb");
    if !db_path.is_file() {
        return json!({
            "available": false,
            "path": db_path.display().to_string(),
            "keys": [],
            "entries": []
        });
    }

    match query_state_vscdb(&db_path) {
        Ok(result) => result,
        Err(error) => json!({
            "available": false,
            "path": db_path.display().to_string(),
            "error": error,
            "keys": [],
            "entries": []
        }),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn query_state_vscdb(db_path: &Path) -> Result<Value, String> {
    let temp_path = temp_copy_path(db_path);
    fs::copy(db_path, &temp_path)
        .map_err(|error| format!("Failed to copy {}: {}", db_path.display(), error))?;

    let query_result = (|| -> Result<Value, String> {
        let connection = Connection::open(&temp_path)
            .map_err(|error| format!("Failed to open copied database {}: {}", temp_path.display(), error))?;

        let mut statement = connection
            .prepare("SELECT key, value FROM ItemTable ORDER BY key")
            .map_err(|error| format!("Failed to prepare ItemTable query: {}", error))?;

        let rows = statement
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let raw_value = row.get_ref(1)?;
                Ok((key, sqlite_value_to_json(raw_value)))
            })
            .map_err(|error| format!("Failed to query ItemTable: {}", error))?;

        let mut keys = Vec::new();
        let mut entries = Vec::new();
        for row in rows {
            let (key, value) = row.map_err(|error| format!("Failed to read ItemTable row: {}", error))?;
            if is_chat_key(&key) {
                keys.push(key.clone());
                entries.push(json!({
                    "key": key,
                    "value": value
                }));
            }
        }

        Ok(json!({
            "available": true,
            "path": db_path.display().to_string(),
            "copied_to": temp_path.display().to_string(),
            "key_count": keys.len(),
            "keys": keys,
            "entries": entries,
            "session_index": select_entry(&entries, "chat.ChatSessionStore.index"),
            "live_history": select_entry(&entries, "memento/interactive-session"),
            "active_view": select_entry(&entries, "memento/interactive-session-view-copilot")
        }))
    })();

    let _ = fs::remove_file(&temp_path);
    query_result
}

#[cfg(not(target_arch = "wasm32"))]
fn temp_copy_path(db_path: &Path) -> PathBuf {
    let file_name = format!(
        "{}-{}.tmp",
        db_path.file_name().and_then(|value| value.to_str()).unwrap_or("state.vscdb"),
        uuid::Uuid::new_v4()
    );
    env::temp_dir().join(file_name)
}

#[cfg(not(target_arch = "wasm32"))]
fn select_entry(entries: &[Value], key: &str) -> Value {
    entries
        .iter()
        .find(|entry| entry.get("key").and_then(Value::as_str) == Some(key))
        .cloned()
        .unwrap_or(Value::Null)
}

#[cfg(not(target_arch = "wasm32"))]
fn is_chat_key(key: &str) -> bool {
    key.starts_with("chat.")
        || key.contains("interactive-session")
        || key.contains("copilot-chat")
        || key.contains("panel.chat")
        || key.contains("terminalChat")
        || key.contains("agentSessions")
        || key.contains("memento/chat")
}

#[cfg(not(target_arch = "wasm32"))]
fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(number) => json!(number),
        ValueRef::Real(number) => json!(number),
        ValueRef::Text(bytes) => decode_string_value(std::str::from_utf8(bytes).unwrap_or_default()),
        ValueRef::Blob(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => decode_string_value(text),
            Err(_) => json!({
                "encoding": "blob",
                "byte_len": bytes.len()
            }),
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_string_value(text: &str) -> Value {
    serde_json::from_str::<Value>(text).unwrap_or_else(|_| Value::String(text.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_json_file(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("Failed to parse {}: {}", path.display(), error))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_sorted_json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return files,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("json") {
            files.push(path);
        }
    }

    files.sort();
    files
}

#[cfg(not(target_arch = "wasm32"))]
fn truncate_text(text: &str) -> String {
    const LIMIT: usize = 80;
    let trimmed = text.trim();
    if trimmed.chars().count() <= LIMIT {
        return trimmed.to_string();
    }

    trimmed.chars().take(LIMIT).collect::<String>() + "..."
}