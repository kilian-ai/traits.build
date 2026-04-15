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
//!     Each user-layer file carries `created` and `modified` timestamps
//!     (Unix seconds) so user edits are never overwritten by older deploy files.
//!
//! The active implementation is selected via `Platform::make_vfs` so the CLI
//! session automatically gets the right backend without any conditional
//! compilation in `cli.rs`.

use std::collections::{HashMap, HashSet};

// ── Timestamp helpers (compile-once, not a heavy dep) ─────────────────────────

/// Current Unix timestamp in seconds.
/// Falls back to 0 in environments where time is not available.
fn now_unix() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        // js_sys::Date::now() returns milliseconds since epoch as f64
        (js_sys::Date::now() / 1000.0) as u64
    }
}

// ── Per-file metadata ─────────────────────────────────────────────────────────

/// A file entry in the user layer, carrying content + timestamps.
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub content: String,
    /// Unix seconds when the file was first created in this session.
    pub created: u64,
    /// Unix seconds of the last write/append to this file.
    pub modified: u64,
}

impl FileEntry {
    fn new(content: impl Into<String>) -> Self {
        let t = now_unix();
        Self { content: content.into(), created: t, modified: t }
    }

    fn with_timestamps(content: impl Into<String>, created: u64, modified: u64) -> Self {
        Self { content: content.into(), created, modified }
    }
}

// ── Vfs trait ─────────────────────────────────────────────────────────────────

/// Virtual filesystem interface used by the CLI session and exec_line builtins.
///
/// All paths are normalised (leading `/` stripped) before storage/lookup.
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
    /// Return `(created_unix, modified_unix)` for a file, or `None` if it doesn't exist
    /// or timestamps are not tracked by this implementation.
    fn stat(&self, path: &str) -> Option<(u64, u64)> { let _ = path; None }
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

fn infer_dirs_from_files_entries(files: &HashMap<String, FileEntry>) -> HashSet<String> {
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
        let k = normalize_owned(path);
        if k.is_empty() {
            return false;
        }

        let mut removed = self.files.remove(&k).is_some();
        let prefix = format!("{}/", k);

        // Remove all files under this directory path.
        let file_children: Vec<String> = self
            .files
            .keys()
            .filter(|p| p.starts_with(&prefix))
            .cloned()
            .collect();
        for child in file_children {
            removed = self.files.remove(&child).is_some() || removed;
        }

        // Remove explicit directory entries for this subtree.
        let dir_children: Vec<String> = self
            .dirs
            .iter()
            .filter(|d| *d == &k || d.starts_with(&prefix))
            .cloned()
            .collect();
        for d in dir_children {
            removed = self.dirs.remove(&d) || removed;
        }

        removed
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
///
/// Each user-layer file stores `created` + `modified` Unix timestamps.
/// `seed_with_mtime` uses the builtin's mtime to decide whether to overwrite
/// a user-layer file: builtin wins only if its mtime is strictly newer.
pub struct LayeredVfs {
    /// Read-only files seeded at init.  Owned strings so both WASM
    /// (`&'static str` converted) and native (`fs::read_to_string`) can seed.
    builtins: HashMap<String, String>,
    /// Real on-disk mtime (Unix seconds) for each builtin, embedded at build time.
    builtin_mtimes: HashMap<String, u64>,
    builtin_dirs: HashSet<String>,
    /// Ephemeral writes from the terminal session, with timestamps.
    user: HashMap<String, FileEntry>,
    user_dirs: HashSet<String>,
}

impl LayeredVfs {
    pub fn new() -> Self {
        Self {
            builtins: HashMap::new(),
            builtin_mtimes: HashMap::new(),
            builtin_dirs: HashSet::new(),
            user: HashMap::new(),
            user_dirs: HashSet::new(),
        }
    }

    /// Seed a builtin (read-only) file without a known mtime (mtime = 0).
    pub fn seed(&mut self, path: &str, content: impl Into<String>) {
        self.seed_with_mtime(path, content, 0);
    }

    /// Seed a builtin (read-only) file with its real on-disk mtime.
    ///
    /// If the user layer already has this path, it is **only overwritten when
    /// `builtin_mtime > user.modified`** (i.e. the deploy file is newer than
    /// the user's last edit).  A mtime of `0` means "unknown / always keep user".
    pub fn seed_with_mtime(&mut self, path: &str, content: impl Into<String>, builtin_mtime: u64) {
        let k = normalize_owned(path);
        if k.is_empty() {
            return;
        }
        for d in parent_dirs(&k) {
            self.builtin_dirs.insert(d);
        }
        let content_str = content.into();
        // Decide whether to promote a newer builtin into the user layer.
        if builtin_mtime > 0 {
            if let Some(user_entry) = self.user.get_mut(&k) {
                if builtin_mtime > user_entry.modified {
                    // Deploy file is newer — overwrite user layer, preserve created time.
                    let created = user_entry.created;
                    *user_entry = FileEntry::with_timestamps(&content_str, created, builtin_mtime);
                }
                // else: user edit is newer — keep it, don't touch user layer.
            }
            // (If no user entry exists, the builtin layer itself serves the read.)
        }
        self.builtin_mtimes.insert(k.clone(), builtin_mtime);
        self.builtins.insert(k, content_str);
    }
}

impl Default for LayeredVfs {
    fn default() -> Self { Self::new() }
}

impl Vfs for LayeredVfs {
    fn read(&self, path: &str) -> Option<String> {
        let k = normalize(path);
        self.user.get(k).map(|e| e.content.clone())
            .or_else(|| self.builtins.get(k).cloned())
    }

    fn write(&mut self, path: &str, content: &str) {
        let k = normalize_owned(path);
        for d in parent_dirs(&k) {
            self.user_dirs.insert(d);
        }
        let now = now_unix();
        self.user.entry(k).and_modify(|e| { e.content = content.to_string(); e.modified = now; })
            .or_insert_with(|| FileEntry::new(content));
    }

    fn append(&mut self, path: &str, content: &str) {
        let k = normalize_owned(path);
        for d in parent_dirs(&k) {
            self.user_dirs.insert(d);
        }
        let base = self.user.get(&k).map(|e| e.content.clone())
            .or_else(|| self.builtins.get(&k).cloned())
            .unwrap_or_default();
        let new_content = base + content;
        let now = now_unix();
        self.user.entry(k).and_modify(|e| { e.content = new_content.clone(); e.modified = now; })
            .or_insert_with(|| FileEntry::new(new_content));
    }

    fn delete(&mut self, path: &str) -> bool {
        let k = normalize_owned(path);
        if k.is_empty() {
            return false;
        }

        let mut removed = self.user.remove(&k).is_some();
        let prefix = format!("{}/", k);

        // Remove all user-layer files under this directory path.
        let user_children: Vec<String> = self
            .user
            .keys()
            .filter(|p| p.starts_with(&prefix))
            .cloned()
            .collect();
        for child in user_children {
            removed = self.user.remove(&child).is_some() || removed;
        }

        // Remove explicit user directories for this subtree.
        let dir_children: Vec<String> = self
            .user_dirs
            .iter()
            .filter(|d| *d == &k || d.starts_with(&prefix))
            .cloned()
            .collect();
        for d in dir_children {
            removed = self.user_dirs.remove(&d) || removed;
        }

        removed
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
        let mut keys: HashSet<String> =
            self.user.keys().chain(self.builtins.keys()).cloned().collect();
        let mut v: Vec<String> = keys.drain().collect();
        v.sort();
        v
    }

    fn list_dirs(&self) -> Vec<String> {
        let mut dirs: HashSet<String> = self.builtin_dirs.clone();
        dirs.extend(self.user_dirs.clone());
        dirs.extend(infer_dirs_from_files_entries(&self.user));
        dirs.extend(infer_dirs_from_files(&self.builtins));
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
            || infer_dirs_from_files_entries(&self.user).contains(&k)
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

    fn stat(&self, path: &str) -> Option<(u64, u64)> {
        let k = normalize(path);
        self.user.get(k).map(|e| (e.created, e.modified))
    }

    /// Only the user layer is serialised.  Builtins are reconstructed at init.
    fn dump(&self) -> String {
        let files: HashMap<&str, serde_json::Value> = self.user.iter().map(|(k, e)| {
            (k.as_str(), serde_json::json!({
                "content": e.content,
                "created": e.created,
                "modified": e.modified,
            }))
        }).collect();
        serde_json::json!({
            "files": files,
            "dirs": self.user_dirs,
        })
        .to_string()
    }

    /// Only the user layer is restored.  Builtins remain intact.
    fn load(&mut self, json: &str) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(files_val) = v.get("files").and_then(|f| f.as_object()) {
                self.user.clear();
                for (k, entry_val) in files_val {
                    let entry = if let (Some(content), created, modified) = (
                        entry_val.get("content").and_then(|c| c.as_str()),
                        entry_val.get("created").and_then(|t| t.as_u64()).unwrap_or(0),
                        entry_val.get("modified").and_then(|t| t.as_u64()).unwrap_or(0),
                    ) {
                        FileEntry::with_timestamps(content, created, modified)
                    } else if let Some(s) = entry_val.as_str() {
                        // Legacy format: plain string value
                        FileEntry::new(s)
                    } else {
                        continue;
                    };
                    self.user.insert(k.clone(), entry);
                }
                self.user_dirs = v
                    .get("dirs")
                    .and_then(|d| serde_json::from_value(d.clone()).ok())
                    .unwrap_or_default();
                return;
            }
            // Legacy format: files was a plain string map
            if let Some(files_val) = v.get("files") {
                if let Ok(m) = serde_json::from_value::<HashMap<String, String>>(files_val.clone()) {
                    self.user = m.into_iter().map(|(k, c)| (k, FileEntry::new(c))).collect();
                    self.user_dirs = v.get("dirs")
                        .and_then(|d| serde_json::from_value(d.clone()).ok())
                        .unwrap_or_default();
                    return;
                }
            }
        }
        // Legacy v0: bare flat map of path→content
        if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(json) {
            self.user = m.into_iter().map(|(k, c)| (k, FileEntry::new(c))).collect();
            self.user_dirs.clear();
        }
    }
}
