//! Virtual filesystem abstraction.
//!
//! Provides the `Vfs` trait and two implementations:
//!
//! - **`MemVfs`** — pure in-memory HashMap, the zero-dep fallback used before
//!   the platform layer is initialised.
//! - **`LayeredVfs`** — two-layer VFS:
//!   - *Builtin layer* (read-only) — seeded at init time from either embedded
//!     binary assets (WASM) or the real filesystem (native).  Never included in
//!     `dump()`.
//!   - *User layer* (read-write) — ephemeral writes from `cat`/`write`/`>>`/`rm`.
//!     Serialised to/from JSON for localStorage persistence on WASM.
//!
//! The active implementation is selected via `Platform::make_vfs` so the CLI
//! session automatically gets the right backend without any conditional
//! compilation in `cli.rs`.

use std::collections::HashMap;

// ── Vfs trait ─────────────────────────────────────────────────────────────────

/// Virtual filesystem interface used by the CLI session and exec_line builtins.
///
/// All paths are normalised (leading `/` stripped) before storage/lookup.
/// Sub-directory semantics are left to richer implementations; both built-in
/// implementations treat paths as flat keys.
pub trait Vfs {
    fn read(&self, path: &str) -> Option<String>;
    fn write(&mut self, path: &str, content: &str);
    fn append(&mut self, path: &str, content: &str);
    fn delete(&mut self, path: &str) -> bool;
    fn list(&self) -> Vec<String>;
    fn exists(&self, path: &str) -> bool;
    /// Serialise writable state to a JSON string for persistence (e.g. localStorage).
    fn dump(&self) -> String;
    /// Restore writable state from a JSON string produced by `dump`.
    fn load(&mut self, json: &str);
}

// ── helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn normalize(path: &str) -> &str {
    path.trim_start_matches('/')
}

// ── MemVfs ────────────────────────────────────────────────────────────────────

/// Minimal HashMap-backed VFS.  Used as the fallback before `platform::init()`.
#[derive(Default)]
pub struct MemVfs {
    files: HashMap<String, String>,
}

impl Vfs for MemVfs {
    fn read(&self, path: &str) -> Option<String> {
        self.files.get(normalize(path)).cloned()
    }

    fn write(&mut self, path: &str, content: &str) {
        self.files.insert(normalize(path).to_string(), content.to_string());
    }

    fn append(&mut self, path: &str, content: &str) {
        self.files.entry(normalize(path).to_string()).or_default().push_str(content);
    }

    fn delete(&mut self, path: &str) -> bool {
        self.files.remove(normalize(path)).is_some()
    }

    fn list(&self) -> Vec<String> {
        let mut v: Vec<String> = self.files.keys().cloned().collect();
        v.sort();
        v
    }

    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(normalize(path))
    }

    fn dump(&self) -> String {
        serde_json::to_string(&self.files).unwrap_or_else(|_| "{}".to_string())
    }

    fn load(&mut self, json: &str) {
        if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(json) {
            self.files = m;
        }
    }
}

// ── LayeredVfs ────────────────────────────────────────────────────────────────

/// Two-layer VFS: a read-only *builtin* layer seeded at init + a read-write
/// *user* layer persisted via `dump`/`load`.
///
/// Read priority: user layer first, then builtin layer.
/// `dump`/`load` only touch the user layer — builtins are always reconstructed
/// at init time from the binary (WASM) or the real filesystem (native) so they
/// add zero bytes to localStorage or any other persistence store.
pub struct LayeredVfs {
    /// Read-only files seeded at init.  Owned strings so both WASM
    /// (`&'static str` converted) and native (`fs::read_to_string`) can seed.
    builtins: HashMap<String, String>,
    /// Ephemeral writes from the terminal session.
    user: HashMap<String, String>,
}

impl LayeredVfs {
    pub fn new() -> Self {
        Self { builtins: HashMap::new(), user: HashMap::new() }
    }

    /// Seed a builtin (read-only) file.  Overwrites any previous entry with the
    /// same path.  The content is cloned so callers may pass `&'static str` or
    /// a freshly-read `String` transparently.
    pub fn seed(&mut self, path: &str, content: impl Into<String>) {
        self.builtins.insert(normalize(path).to_string(), content.into());
    }
}

impl Default for LayeredVfs {
    fn default() -> Self { Self::new() }
}

impl Vfs for LayeredVfs {
    fn read(&self, path: &str) -> Option<String> {
        let k = normalize(path);
        self.user.get(k).cloned()
            .or_else(|| self.builtins.get(k).cloned())
    }

    fn write(&mut self, path: &str, content: &str) {
        self.user.insert(normalize(path).to_string(), content.to_string());
    }

    fn append(&mut self, path: &str, content: &str) {
        let k = normalize(path).to_string();
        let base = self.user.get(&k).cloned()
            .or_else(|| self.builtins.get(&k).cloned())
            .unwrap_or_default();
        self.user.insert(k, base + content);
    }

    fn delete(&mut self, path: &str) -> bool {
        self.user.remove(normalize(path)).is_some()
    }

    fn list(&self) -> Vec<String> {
        let mut keys: std::collections::HashSet<String> =
            self.user.keys().chain(self.builtins.keys()).cloned().collect();
        let mut v: Vec<String> = keys.drain().collect();
        v.sort();
        v
    }

    fn exists(&self, path: &str) -> bool {
        let k = normalize(path);
        self.user.contains_key(k) || self.builtins.contains_key(k)
    }

    /// Only the user layer is serialised.  Builtins are reconstructed at init.
    fn dump(&self) -> String {
        serde_json::to_string(&self.user).unwrap_or_else(|_| "{}".to_string())
    }

    /// Only the user layer is restored.  Builtins remain intact.
    fn load(&mut self, json: &str) {
        if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(json) {
            self.user = m;
        }
    }
}
