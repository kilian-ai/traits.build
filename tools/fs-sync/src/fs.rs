//! Fid table + path sandboxing.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Component, Path, PathBuf};

use crate::proto::{Qid, QT_DIR, QT_FILE, QT_SYMLINK};

/// What a fid currently references.
pub enum Handle {
    /// Not yet opened — just a path reference (after attach/walk).
    Unopened,
    /// Opened regular file.
    File(File),
    /// Opened directory — cached entry snapshot for offset-based reads.
    Dir(DirSnapshot),
}

pub struct DirSnapshot {
    /// Sorted entries: (name, qid, type_byte_for_readdir).
    /// type_byte_for_readdir matches Linux d_type constants (DT_DIR=4, DT_REG=8, DT_LNK=10).
    pub entries: Vec<(String, Qid, u8)>,
}

pub struct Fid {
    pub path: PathBuf,
    pub handle: Handle,
}

pub struct FidTable {
    map: HashMap<u32, Fid>,
}

impl Default for FidTable {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

impl FidTable {
    pub fn insert(&mut self, fid: u32, f: Fid) {
        self.map.insert(fid, f);
    }
    pub fn contains(&self, fid: u32) -> bool {
        self.map.contains_key(&fid)
    }
    pub fn get(&self, fid: u32) -> Option<&Fid> {
        self.map.get(&fid)
    }
    pub fn get_mut(&mut self, fid: u32) -> Option<&mut Fid> {
        self.map.get_mut(&fid)
    }
    pub fn remove(&mut self, fid: u32) -> Option<Fid> {
        self.map.remove(&fid)
    }
}

/// Clamp a joined path to stay inside `root`. Rejects absolute components and
/// `..` traversal that would escape the sandbox.
pub fn safe_join(root: &Path, base: &Path, components: &[String]) -> Option<PathBuf> {
    // Start from base relative to root. Guarantee base is inside root.
    let base_rel = base.strip_prefix(root).unwrap_or(Path::new("")).to_path_buf();
    let mut parts: Vec<String> = base_rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str().map(|s| s.to_string()),
            _ => None,
        })
        .collect();

    for comp in components {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." {
            if parts.pop().is_none() {
                // walking past the root — reject.
                return None;
            }
            continue;
        }
        if comp.contains('/') || comp.contains('\0') {
            return None;
        }
        parts.push(comp.clone());
    }

    let mut out = root.to_path_buf();
    for p in parts {
        out.push(p);
    }
    Some(out)
}

/// Build a qid from filesystem metadata. Uses (dev, ino) as a stable identity.
pub fn qid_from_meta(meta: &std::fs::Metadata) -> Qid {
    #[cfg(unix)]
    let (ino, mtime_ns) = {
        use std::os::unix::fs::MetadataExt;
        (meta.ino(), meta.mtime_nsec() as u32 ^ meta.mtime() as u32)
    };
    #[cfg(not(unix))]
    let (ino, mtime_ns): (u64, u32) = (0, 0);

    let qtype = if meta.is_dir() {
        QT_DIR
    } else if meta.file_type().is_symlink() {
        QT_SYMLINK
    } else {
        QT_FILE
    };

    Qid {
        qtype,
        version: mtime_ns,
        path: ino,
    }
}
