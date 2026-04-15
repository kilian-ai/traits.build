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

use std::collections::{HashMap, HashSet};

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
    fn mkdir(&mut self, path: &str) -> bool;
    fn list(&self) -> Vec<String>;
    fn list_dirs(&self) -> Vec<String>;
    fn is_dir(&self, path: &str) -> bool;
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

fn normalize_owned(path: &str) -> String {
    normalize(path).trim_end_matches('/').to_string()
}

fn parent_dirs(path: &str) -> Vec<String> {
    let p = normalize(path).trim_end_matches('/');
    if p.is_empty() {
        return vec![];
    }
    let parts: Vec<&str> = p.split('/').collect();
    let mut dirs = Vec::new();
    let mut acc = String::new();
    for seg in parts.iter().take(parts.len().saturating_sub(1)) {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(seg);
        dirs.push(acc.clone());
    }
    dirs
}

fn infer_dirs_from_files(files: &HashMap<String, String>) -> HashSet<String> {
    let mut dirs = HashSet::new();
    for key in files.keys() {
        for d in parent_dirs(key) {
            dirs.insert(d);
        }
    }
    dirs
}

// ── MemVfs ────────────────────────────────────────────────────────────────────

/// Minimal HashMap-backed VFS.  Used as the fallback before `platform::init()`.
#[derive(Default)]
pub struct MemVfs {
    files: HashMap<String, String>,
    dirs: HashSet<String>,
}

impl Vfs for MemVfs {
    fn read(&self, path: &str) -> Option<String> {
        self.files.get(normalize(path)).cloned()
    }

    fn write(&mut self, path: &str, content: &str) {
        let k = normalize_owned(path);
        for d in parent_dirs(&k) {
            self.dirs.insert(d);
        }
        self.files.insert(k, content.to_string());
    }

    fn append(&mut self, path: &str, content: &str) {
        let k = normalize_owned(path);
        for d in parent_dirs(&k) {
            self.dirs.insert(d);
        }
        self.files.entry(k).or_default().push_str(content);
    }

    fn delete(&mut self, path: &str) -> bool {
        self.files.remove(normalize(path)).is_some()
    }

    fn mkdir(&mut self, path: &str) -> bool {
        let k = normalize_owned(path);
        if k.is_empty() {
            return true;
        }
        for d in parent_dirs(&k) {
            self.dirs.insert(d);
        }
        self.dirs.insert(k)
    }

    fn list(&self) -> Vec<String> {
        let mut v: Vec<String> = self.files.keys().cloned().collect();
        v.sort();
        v
    }

    fn list_dirs(&self) -> Vec<String> {
        let mut all = self.dirs.clone();
        all.extend(infer_dirs_from_files(&self.files));
        let mut v: Vec<String> = all.into_iter().collect();
        v.sort();
        v
    }

    fn is_dir(&self, path: &str) -> bool {
        let k = normalize_owned(path);
        if k.is_empty() {
            return true;
        }
        if self.dirs.contains(&k) || infer_dirs_from_files(&self.files).contains(&k) {
            return true;
        }
        let prefix = format!("{}/", k);
        self.files.keys().any(|p| p.starts_with(&prefix))
    }

    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(normalize(path)) || self.is_dir(path)
    }

    fn dump(&self) -> String {
        serde_json::json!({
            "files": self.files,
            "dirs": self.dirs,
        })
        .to_string()
    }

    fn load(&mut self, json: &str) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(files) = v.get("files") {
                self.files = serde_json::from_value(files.clone()).unwrap_or_default();
                self.dirs = v
                    .get("dirs")
                    .and_then(|d| serde_json::from_value(d.clone()).ok())
                    .unwrap_or_default();
                return;
            }
        }
        if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(json) {
            self.files = m;
            self.dirs.clear();
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
    builtin_dirs: HashSet<String>,
    /// Ephemeral writes from the terminal session.
    user: HashMap<String, String>,
    user_dirs: HashSet<String>,
}

impl LayeredVfs {
    pub fn new() -> Self {
        Self {
            builtins: HashMap::new(),
            builtin_dirs: HashSet::new(),
            user: HashMap::new(),
            user_dirs: HashSet::new(),
        }
    }

    /// Seed a builtin (read-only) file.  Overwrites any previous entry with the
    /// same path.  The content is cloned so callers may pass `&'static str` or
    /// a freshly-read `String` transparently.
    pub fn seed(&mut self, path: &str, content: impl Into<String>) {
        let k = normalize_owned(path);
        if k.is_empty() {
            return;
        }
        for d in parent_dirs(&k) {
            self.builtin_dirs.insert(d);
        }
        self.builtins.insert(k, content.into());
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
        let k = normalize_owned(path);
        for d in parent_dirs(&k) {
            self.user_dirs.insert(d);
        }
        self.user.insert(k, content.to_string());
    }

    fn append(&mut self, path: &str, content: &str) {
        let k = normalize_owned(path);
        for d in parent_dirs(&k) {
            self.user_dirs.insert(d);
        }
        let base = self.user.get(&k).cloned()
            .or_else(|| self.builtins.get(&k).cloned())
            .unwrap_or_default();
        self.user.insert(k, base + content);
    }

    fn delete(&mut self, path: &str) -> bool {
        self.user.remove(normalize(path)).is_some()
    }

    fn mkdir(&mut self, path: &str) -> bool {
        let k = normalize_owned(path);
        if k.is_empty() {
            return true;
        }
        for d in parent_dirs(&k) {
            self.user_dirs.insert(d);
        }
        self.user_dirs.insert(k)
    }

    fn list(&self) -> Vec<String> {
        let mut keys: std::collections::HashSet<String> =
            self.user.keys().chain(self.builtins.keys()).cloned().collect();
        let mut v: Vec<String> = keys.drain().collect();
        v.sort();
        v
    }

    fn list_dirs(&self) -> Vec<String> {
        let mut dirs: HashSet<String> = self.builtin_dirs.clone();
        dirs.extend(self.user_dirs.clone());
        dirs.extend(infer_dirs_from_files(&self.builtins));
        dirs.extend(infer_dirs_from_files(&self.user));
        let mut v: Vec<String> = dirs.into_iter().collect();
        v.sort();
        v
    }

    fn is_dir(&self, path: &str) -> bool {
        let k = normalize_owned(path);
        if k.is_empty() {
            return true;
        }
        if self.user_dirs.contains(&k)
            || self.builtin_dirs.contains(&k)
            || infer_dirs_from_files(&self.user).contains(&k)
            || infer_dirs_from_files(&self.builtins).contains(&k)
        {
            return true;
        }
        let prefix = format!("{}/", k);
        self.user.keys().any(|p| p.starts_with(&prefix))
            || self.builtins.keys().any(|p| p.starts_with(&prefix))
    }

    fn exists(&self, path: &str) -> bool {
        let k = normalize(path);
        self.user.contains_key(k) || self.builtins.contains_key(k) || self.is_dir(path)
    }

    /// Only the user layer is serialised.  Builtins are reconstructed at init.
    fn dump(&self) -> String {
        serde_json::json!({
            "files": self.user,
            "dirs": self.user_dirs,
        })
        .to_string()
    }

    /// Only the user layer is restored.  Builtins remain intact.
    fn load(&mut self, json: &str) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(files) = v.get("files") {
                self.user = serde_json::from_value(files.clone()).unwrap_or_default();
                self.user_dirs = v
                    .get("dirs")
                    .and_then(|d| serde_json::from_value(d.clone()).ok())
                    .unwrap_or_default();
                return;
            }
        }
        if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(json) {
            self.user = m;
            self.user_dirs.clear();
        }
    }
}
