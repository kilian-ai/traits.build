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

// ── LayeredVfs — builtin (read-only) layer + user (read-write) layer ─────────
//
// Builtin files are seeded from the WASM binary at init time (all `.trait.toml`
// and `.features.json` files embedded via `include_str!` in `wasm_builtin_traits.rs`).
// They are always available from the embedded binary so they are NOT included in
// `dump()` — only the user layer is round-tripped through localStorage.
//
// Priority: user layer > builtin layer.  `delete()` removes from the user layer;
// if the path is builtin-only, `delete()` is a no-op (file re-appears from builtins).

pub struct LayeredVfs {
    builtins: HashMap<String, &'static str>,
    user: HashMap<String, String>,
}

impl LayeredVfs {
    pub fn new() -> Self {
        Self { builtins: HashMap::new(), user: HashMap::new() }
    }

    /// Seed a static (compile-time) read-only file.
    /// Called during WASM `init()` for every embedded `.trait.toml` / `.features.json`.
    pub fn seed(&mut self, path: &str, content: &'static str) {
        self.builtins.insert(normalize_path(path).to_string(), content);
    }
}

impl Default for LayeredVfs {
    fn default() -> Self { Self::new() }
}

impl Vfs for LayeredVfs {
    fn read(&self, path: &str) -> Option<String> {
        let k = normalize_path(path);
        self.user.get(k).cloned()
            .or_else(|| self.builtins.get(k).map(|s| (*s).to_string()))
    }

    fn write(&mut self, path: &str, content: &str) {
        self.user.insert(normalize_path(path).to_string(), content.to_string());
    }

    fn append(&mut self, path: &str, content: &str) {
        let k = normalize_path(path).to_string();
        let base = self.user.get(&k).cloned()
            .or_else(|| self.builtins.get(k.as_str()).map(|s| s.to_string()))
            .unwrap_or_default();
        self.user.insert(k, base + content);
    }

    fn delete(&mut self, path: &str) -> bool {
        self.user.remove(normalize_path(path)).is_some()
    }

    fn list(&self) -> Vec<String> {
        let mut keys: std::collections::HashSet<String> = self.user.keys().cloned().collect();
        keys.extend(self.builtins.keys().cloned());
        let mut v: Vec<String> = keys.into_iter().collect();
        v.sort();
        v
    }

    fn exists(&self, path: &str) -> bool {
        let k = normalize_path(path);
        self.user.contains_key(k) || self.builtins.contains_key(k)
    }

    /// Serialise only the user layer — builtins are always reconstructed from the binary.
    fn dump(&self) -> String {
        serde_json::to_string(&self.user).unwrap_or_else(|_| "{}".to_string())
    }

    /// Restore only the user layer; builtins stay intact.
    fn load(&mut self, json: &str) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(json) {
            self.user = map;
        }
    }
}
