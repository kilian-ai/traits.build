use serde_json::{json, Value};

/// VFS operations: read, write, list, delete files from the persistent virtual filesystem.
///
/// On WASM: backed by a global VFS with auto-persistence to localStorage.
/// On native: backed by the `data/vfs/` directory + project root for reads.
///
/// Actions:
///   read  <path>            — read file content
///   write <path> <content>  — write file content
///   list  [prefix]          — list files (optional prefix filter)
///   delete <path>           — delete a file
///   exists <path>           — check if file exists
pub fn vfs(args: &[Value]) -> Value {
    let action = args.get(0).and_then(|v| v.as_str()).unwrap_or("list");
    let path = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let content = args.get(2).and_then(|v| v.as_str()).unwrap_or("");

    eprintln!("[vfs] action={} path={} content_len={}", action, path, content.len());

    match action {
        "read" => {
            if path.is_empty() {
                return json!({"ok": false, "error": "Path required"});
            }
            match kernel_logic::platform::vfs_read(path) {
                Some(data) => {
                    eprintln!("[vfs] read OK path={} len={}", path, data.len());
                    json!({"ok": true, "path": path, "content": data})
                }
                None => {
                    eprintln!("[vfs] read FAIL path={} (not found)", path);
                    json!({"ok": false, "error": format!("File not found: {}", path)})
                }
            }
        }
        "write" => {
            if path.is_empty() {
                return json!({"ok": false, "error": "Path required"});
            }
            eprintln!("[vfs] write path={} bytes={}", path, content.len());
            kernel_logic::platform::vfs_write(path, content);
            json!({"ok": true, "path": path, "bytes": content.len()})
        }
        "list" => {
            let all = kernel_logic::platform::vfs_list();
            let filtered: Vec<&str> = if path.is_empty() {
                all.iter().map(|s| s.as_str()).collect()
            } else {
                let prefix = path.trim_end_matches('/');
                all.iter()
                    .filter(|f| f.starts_with(prefix))
                    .map(|s| s.as_str())
                    .collect()
            };
            json!({"ok": true, "files": filtered, "count": filtered.len()})
        }
        "delete" => {
            if path.is_empty() {
                return json!({"ok": false, "error": "Path required"});
            }
            let deleted = kernel_logic::platform::vfs_delete(path);
            json!({"ok": true, "deleted": deleted, "path": path})
        }
        "exists" => {
            if path.is_empty() {
                return json!({"ok": false, "error": "Path required"});
            }
            let exists = kernel_logic::platform::vfs_read(path).is_some();
            json!({"ok": true, "exists": exists, "path": path})
        }
        _ => json!({"ok": false, "error": format!("Unknown action: {}. Use: read, write, list, delete, exists", action)}),
    }
}
