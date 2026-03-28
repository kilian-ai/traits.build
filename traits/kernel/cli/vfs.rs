// ── Virtual filesystem abstraction ──
//
// `Vfs` is the interface that decouples the CLI builtins (`cat`, `write`,
// `rm`, `ls`) from any particular storage backend.
//
// Today's implementation — `MemVfs` — keeps everything in a `HashMap`.
// On the browser side `terminal.js` serialises it to `localStorage` after
// every command.  In the future this seam lets you bind a richer backend:
//
//   session.set_vfs(Box::new(MyOriginPrivateFs::new()));
//
// without touching `exec_line` or the builtin implementations.

use std::collections::HashMap;

// ── Vfs trait ────────────────────────────────────────────────────────────────

/// Virtual filesystem interface.
///
/// All paths are treated as flat keys after `normalize_path` strips leading
/// slashes.  Sub-directory semantics are left to richer implementations.
pub trait Vfs {
    fn read(&self, path: &str) -> Option<String>;
    fn write(&mut self, path: &str, content: &str);
    fn append(&mut self, path: &str, content: &str);
    fn delete(&mut self, path: &str) -> bool;
    fn list(&self) -> Vec<String>;
    fn exists(&self, path: &str) -> bool;
    /// Serialise the entire VFS to a JSON string for persistence.
    fn dump(&self) -> String;
    /// Restore state from a JSON string produced by `dump`.
    fn load(&mut self, json: &str);
}

// ── MemVfs — HashMap-backed in-memory implementation ────────────────────────

#[derive(Default)]
pub struct MemVfs {
    files: HashMap<String, String>,
}

impl Vfs for MemVfs {
    fn read(&self, path: &str) -> Option<String> {
        self.files.get(normalize_path(path)).cloned()
    }

    fn write(&mut self, path: &str, content: &str) {
        self.files.insert(normalize_path(path).to_string(), content.to_string());
    }

    fn append(&mut self, path: &str, content: &str) {
        self.files
            .entry(normalize_path(path).to_string())
            .or_default()
            .push_str(content);
    }

    fn delete(&mut self, path: &str) -> bool {
        self.files.remove(normalize_path(path)).is_some()
    }

    fn list(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.files.keys().cloned().collect();
        keys.sort();
        keys
    }

    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(normalize_path(path))
    }

    fn dump(&self) -> String {
        serde_json::to_string(&self.files).unwrap_or_else(|_| "{}".to_string())
    }

    fn load(&mut self, json: &str) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(json) {
            self.files = map;
        }
    }
}

fn normalize_path(path: &str) -> &str {
    path.trim_start_matches('/')
}
