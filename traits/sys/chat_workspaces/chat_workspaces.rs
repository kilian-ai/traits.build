use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn chat_workspaces(args: &[Value]) -> Value {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = args;
        return json!({
            "ok": false,
            "error": "sys.chat_workspaces is not available in WASM"
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let storage_root = match workspace_storage_root(args.first().and_then(|value| value.as_str())) {
            Ok(path) => path,
            Err(error) => {
                return json!({
                    "ok": false,
                    "error": error
                });
            }
        };

        let mut workspaces = Vec::new();
        let read_dir = match fs::read_dir(&storage_root) {
            Ok(entries) => entries,
            Err(error) => {
                return json!({
                    "ok": false,
                    "storage_root": storage_root.display().to_string(),
                    "error": format!("Failed to read workspaceStorage: {}", error)
                });
            }
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let workspace_id = match path.file_name().and_then(|value| value.to_str()) {
                Some(value) => value.to_string(),
                None => continue,
            };

            let workspace_json = path.join("workspace.json");
            let state_db = path.join("state.vscdb");
            let chat_sessions = path.join("chatSessions");
            let session_count = count_json_files(&chat_sessions);
            let workspace = read_json_file(&workspace_json).unwrap_or(Value::Null);

            workspaces.push(json!({
                "workspace_id": workspace_id,
                "workspace": workspace,
                "workspace_storage_dir": path.display().to_string(),
                "has_workspace_json": workspace_json.is_file(),
                "has_state_vscdb": state_db.is_file(),
                "has_chat_sessions": chat_sessions.is_dir(),
                "chat_session_count": session_count
            }));
        }

        workspaces.sort_by(|left, right| {
            let left_count = left.get("chat_session_count").and_then(Value::as_u64).unwrap_or(0);
            let right_count = right.get("chat_session_count").and_then(Value::as_u64).unwrap_or(0);
            right_count.cmp(&left_count).then_with(|| {
                left.get("workspace_id")
                    .and_then(Value::as_str)
                    .cmp(&right.get("workspace_id").and_then(Value::as_str))
            })
        });

        json!({
            "ok": true,
            "storage_root": storage_root.display().to_string(),
            "workspace_count": workspaces.len(),
            "workspaces": workspaces
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn count_json_files(dir: &Path) -> usize {
    match fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
            .count(),
        Err(_) => 0,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_json_file(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("Failed to parse {}: {}", path.display(), error))
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