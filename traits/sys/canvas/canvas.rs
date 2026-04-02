use serde_json::{json, Value};

/// Default VFS path for the canvas SPA file.
const CANVAS_VFS_PATH: &str = "canvas/app.html";

pub fn canvas(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("get");

    match action {
        "set" => {
            let content = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let len = content.len();
            kernel_logic::platform::vfs_write(CANVAS_VFS_PATH, content);
            json!({"ok": true, "action": "set", "length": len})
        }
        "append" => {
            let content = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let existing = kernel_logic::platform::vfs_read(CANVAS_VFS_PATH).unwrap_or_default();
            let combined = format!("{}{}", existing, content);
            let len = combined.len();
            kernel_logic::platform::vfs_write(CANVAS_VFS_PATH, &combined);
            json!({"ok": true, "action": "append", "length": len})
        }
        "get" => {
            let content = kernel_logic::platform::vfs_read(CANVAS_VFS_PATH).unwrap_or_default();
            let len = content.len();
            json!({"ok": true, "content": content, "length": len})
        }
        "clear" => {
            kernel_logic::platform::vfs_delete(CANVAS_VFS_PATH);
            json!({"ok": true, "action": "clear"})
        }
        "path" => {
            // Return the VFS path so the agent knows where to write directly
            json!({"ok": true, "vfs_path": CANVAS_VFS_PATH})
        }
        // ── Project management — stored in VFS under canvas/projects/ ──
        "save" => {
            let name = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                return json!({"ok": false, "error": "Project name required"});
            }
            let content = kernel_logic::platform::vfs_read(CANVAS_VFS_PATH).unwrap_or_default();
            let project_path = format!("canvas/projects/{}.html", name);
            kernel_logic::platform::vfs_write(&project_path, &content);
            json!({"ok": true, "action": "save", "name": name, "length": content.len()})
        }
        "load" => {
            let name = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                return json!({"ok": false, "error": "Project name required"});
            }
            let project_path = format!("canvas/projects/{}.html", name);
            match kernel_logic::platform::vfs_read(&project_path) {
                Some(content) => {
                    let len = content.len();
                    kernel_logic::platform::vfs_write(CANVAS_VFS_PATH, &content);
                    json!({"ok": true, "action": "load", "name": name, "length": len})
                }
                None => json!({"ok": false, "error": format!("Project not found: {}", name)}),
            }
        }
        "projects" => {
            let all = kernel_logic::platform::vfs_list();
            let prefix = "canvas/projects/";
            let projects: Vec<&str> = all.iter()
                .filter(|f| f.starts_with(prefix) && f.ends_with(".html"))
                .map(|f| &f[prefix.len()..f.len()-5])
                .collect();
            json!({"ok": true, "projects": projects, "count": projects.len()})
        }
        "delete_project" => {
            let name = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                return json!({"ok": false, "error": "Project name required"});
            }
            let project_path = format!("canvas/projects/{}.html", name);
            let deleted = kernel_logic::platform::vfs_delete(&project_path);
            json!({"ok": true, "deleted": deleted, "name": name})
        }
        _ => json!({"ok": false, "error": format!("Unknown action: {}. Use: set, append, get, clear, path, save, load, projects, delete_project", action)}),
    }
}
