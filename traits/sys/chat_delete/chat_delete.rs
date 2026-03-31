use serde_json::{json, Value};

pub fn chat_delete(args: &[Value]) -> Value {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = args;
        return json!({
            "ok": false,
            "error": "sys.chat_delete is not available in WASM"
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::env;
        use std::fs;
        use std::path::PathBuf;
        use rusqlite::Connection;

        let workspace_id = match args.first().and_then(|v| v.as_str()).map(str::trim) {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => return json!({ "ok": false, "error": "workspace_id is required" }),
        };

        let session_id = match args.get(1).and_then(|v| v.as_str()).map(str::trim) {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => return json!({ "ok": false, "error": "session_id is required" }),
        };

        let storage_root = match workspace_storage_root(args.get(2).and_then(|v| v.as_str())) {
            Ok(p) => p,
            Err(e) => return json!({ "ok": false, "error": e }),
        };

        let workspace_dir = storage_root.join(&workspace_id);
        if !workspace_dir.is_dir() {
            return json!({
                "ok": false,
                "error": format!("Workspace storage folder not found: {}", workspace_dir.display())
            });
        }

        let db_path = workspace_dir.join("state.vscdb");
        let mut removed_from_db = false;
        let mut removed_json = false;

        // Remove from state.vscdb index
        if db_path.is_file() {
            match remove_from_db(&db_path, &session_id) {
                Ok(true) => removed_from_db = true,
                Ok(false) => {}
                Err(e) => return json!({ "ok": false, "error": format!("DB error: {}", e) }),
            }
        }

        // Remove JSON file from chatSessions/
        let json_path = workspace_dir.join("chatSessions").join(format!("{}.json", session_id));
        if json_path.is_file() {
            match fs::remove_file(&json_path) {
                Ok(()) => removed_json = true,
                Err(e) => return json!({
                    "ok": false,
                    "error": format!("Failed to remove JSON file: {}", e)
                }),
            }
        }

        if !removed_from_db && !removed_json {
            return json!({
                "ok": false,
                "error": format!("Session '{}' not found in workspace '{}'", session_id, workspace_id)
            });
        }

        json!({
            "ok": true,
            "session_id": session_id,
            "workspace_id": workspace_id,
            "removed_from_db": removed_from_db,
            "removed_json": removed_json
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn remove_from_db(db_path: &std::path::Path, session_id: &str) -> Result<bool, String> {
    use rusqlite::Connection;

    let conn = Connection::open(db_path)
        .map_err(|e| format!("Failed to open {}: {}", db_path.display(), e))?;

    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = 'chat.ChatSessionStore.index'",
            [],
            |row| row.get(0),
        )
        .ok();

    let raw = match raw {
        Some(v) => v,
        None => return Ok(false),
    };

    let mut index: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse session index: {}", e))?;

    let entries = match index.get_mut("entries").and_then(|e| e.as_object_mut()) {
        Some(e) => e,
        None => return Ok(false),
    };

    if entries.remove(session_id).is_none() {
        return Ok(false);
    }

    let updated = serde_json::to_string(&index)
        .map_err(|e| format!("Failed to serialize updated index: {}", e))?;

    conn.execute(
        "UPDATE ItemTable SET value = ?1 WHERE key = 'chat.ChatSessionStore.index'",
        rusqlite::params![updated],
    )
    .map_err(|e| format!("Failed to write updated index: {}", e))?;

    Ok(true)
}

#[cfg(not(target_arch = "wasm32"))]
fn workspace_storage_root(override_root: Option<&str>) -> Result<std::path::PathBuf, String> {
    use std::env;

    if let Some(path) = override_root.map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(std::path::PathBuf::from(path));
    }

    if let Ok(path) = env::var("VSCODE_WORKSPACESTORAGE_DIR") {
        if !path.trim().is_empty() {
            return Ok(std::path::PathBuf::from(path));
        }
    }

    let home = env::var("HOME")
        .map_err(|_| "HOME is not set and no base_dir override was provided".to_string())?;

    let root = match env::consts::OS {
        "macos" => std::path::PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Code")
            .join("User")
            .join("workspaceStorage"),
        "windows" => {
            let appdata = env::var("APPDATA")
                .map_err(|_| "APPDATA is not set".to_string())?;
            std::path::PathBuf::from(appdata).join("Code").join("User").join("workspaceStorage")
        }
        _ => std::path::PathBuf::from(home)
            .join(".config")
            .join("Code")
            .join("User")
            .join("workspaceStorage"),
    };

    Ok(root)
}
