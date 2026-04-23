//! Per-connection 9P2000.L request handler.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use crate::fs::{qid_from_meta, safe_join, DirSnapshot, Fid, FidTable, Handle};
use crate::proto::{self, errno_from_io, read_frame, rlerror, write_frame, Qid, Reader, Writer};

const MIN_MSIZE: u32 = 4096;
const MAX_MSIZE: u32 = 1 << 20; // 1 MiB
const DEFAULT_MSIZE: u32 = 64 * 1024;

pub struct Conn {
    root: PathBuf,
    readonly: bool,
    msize: u32,
    fids: FidTable,
    // Cached root qid so Rattach is cheap.
    root_qid: Option<Qid>,
}

impl Conn {
    pub fn new(root: PathBuf, readonly: bool) -> Self {
        Self {
            root,
            readonly,
            msize: DEFAULT_MSIZE,
            fids: FidTable::default(),
            root_qid: None,
        }
    }

    pub fn serve(mut self, mut stream: TcpStream) -> io::Result<()> {
        stream.set_nodelay(true).ok();
        loop {
            let (kind, tag, body) = match read_frame(&mut stream, MAX_MSIZE) {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            };
            let reply = self.handle(kind, tag, &body);
            write_frame(&mut stream, &reply)?;
        }
    }

    fn handle(&mut self, kind: u8, tag: u16, body: &[u8]) -> Vec<u8> {
        let mut r = Reader::new(body);
        let result = match kind {
            proto::T_VERSION => self.t_version(tag, &mut r),
            proto::T_AUTH => Err(proto::EACCES), // no auth supported
            proto::T_ATTACH => self.t_attach(tag, &mut r),
            proto::T_FLUSH => Ok(Writer::new(proto::R_FLUSH, tag).finish()),
            proto::T_WALK => self.t_walk(tag, &mut r),
            proto::T_LOPEN => self.t_lopen(tag, &mut r),
            proto::T_LCREATE => self.t_lcreate(tag, &mut r),
            proto::T_READ => self.t_read(tag, &mut r),
            proto::T_WRITE => self.t_write(tag, &mut r),
            proto::T_READDIR => self.t_readdir(tag, &mut r),
            proto::T_GETATTR => self.t_getattr(tag, &mut r),
            proto::T_SETATTR => self.t_setattr(tag, &mut r),
            proto::T_MKDIR => self.t_mkdir(tag, &mut r),
            proto::T_RENAME => self.t_rename(tag, &mut r),
            proto::T_RENAMEAT => self.t_renameat(tag, &mut r),
            proto::T_UNLINKAT => self.t_unlinkat(tag, &mut r),
            proto::T_REMOVE => self.t_remove(tag, &mut r),
            proto::T_CLUNK => self.t_clunk(tag, &mut r),
            proto::T_FSYNC => self.t_fsync(tag, &mut r),
            proto::T_STATFS => self.t_statfs(tag, &mut r),
            proto::T_XATTRWALK => Err(proto::ENOTSUP),
            _ => Err(proto::ENOTSUP),
        };
        match result {
            Ok(frame) => frame,
            Err(ecode) => rlerror(tag, ecode),
        }
    }

    // --- Handlers -----------------------------------------------------------

    fn t_version(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        let msize = r.u32().map_err(|_| proto::EINVAL)?;
        let version = r.string().map_err(|_| proto::EINVAL)?;
        let chosen = if version == "9P2000.L" {
            "9P2000.L"
        } else {
            "unknown"
        };
        self.msize = msize.clamp(MIN_MSIZE, MAX_MSIZE);
        // Reset state on new version negotiation.
        self.fids = FidTable::default();
        let mut w = Writer::new(proto::R_VERSION, tag);
        w.u32(self.msize);
        w.string(chosen);
        Ok(w.finish())
    }

    fn t_attach(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let _afid = r.u32().map_err(|_| proto::EINVAL)?;
        let _uname = r.string().map_err(|_| proto::EINVAL)?;
        let _aname = r.string().map_err(|_| proto::EINVAL)?;
        let _n_uname = r.u32().ok(); // may be absent in some dialects, tolerate
        let meta = fs::metadata(&self.root).map_err(|e| errno_from_io(&e))?;
        let qid = qid_from_meta(&meta);
        self.root_qid = Some(qid);
        self.fids.insert(
            fid,
            Fid {
                path: self.root.clone(),
                handle: Handle::Unopened,
            },
        );
        let mut w = Writer::new(proto::R_ATTACH, tag);
        w.qid(qid);
        Ok(w.finish())
    }

    fn t_walk(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let newfid = r.u32().map_err(|_| proto::EINVAL)?;
        let nwname = r.u16().map_err(|_| proto::EINVAL)? as usize;
        let mut names = Vec::with_capacity(nwname);
        for _ in 0..nwname {
            names.push(r.string().map_err(|_| proto::EINVAL)?);
        }
        let src = self.fids.get(fid).ok_or(proto::EBADF)?;
        let src_path = src.path.clone();

        let mut qids = Vec::with_capacity(nwname);
        let mut cur = src_path.clone();
        for name in &names {
            let joined = match safe_join(&self.root, &cur, std::slice::from_ref(name)) {
                Some(p) => p,
                None => return Err(proto::EINVAL),
            };
            match fs::symlink_metadata(&joined) {
                Ok(m) => {
                    qids.push(qid_from_meta(&m));
                    cur = joined;
                }
                Err(_) if !qids.is_empty() => {
                    // 9P2000.L convention: partial walk — return collected qids,
                    // do NOT bind newfid.
                    break;
                }
                Err(e) => {
                    return Err(errno_from_io(&e));
                }
            }
        }

        if qids.len() == nwname {
            if fid != newfid && self.fids.contains(newfid) {
                return Err(proto::EBADF);
            }
            self.fids.insert(
                newfid,
                Fid {
                    path: cur,
                    handle: Handle::Unopened,
                },
            );
        }

        let mut w = Writer::new(proto::R_WALK, tag);
        w.u16(qids.len() as u16);
        for q in qids {
            w.qid(q);
        }
        Ok(w.finish())
    }

    fn t_lopen(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let flags = r.u32().map_err(|_| proto::EINVAL)?;
        let f = self.fids.get_mut(fid).ok_or(proto::EBADF)?;
        let path = f.path.clone();
        let meta = fs::symlink_metadata(&path).map_err(|e| errno_from_io(&e))?;
        let qid = qid_from_meta(&meta);

        if meta.is_dir() {
            let snap = snapshot_dir(&path)?;
            f.handle = Handle::Dir(snap);
        } else {
            let needs_write = (flags & 0x3) != 0 || (flags & 0x400) != 0; // O_WRONLY|O_RDWR|O_APPEND
            if self.readonly && needs_write {
                return Err(proto::EACCES);
            }
            let mut opts = OpenOptions::new();
            let acc = flags & 0x3;
            opts.read(acc == 0 || acc == 2);
            opts.write(acc == 1 || acc == 2);
            opts.append((flags & 0o2000) != 0); // O_APPEND
            if (flags & 0o1000) != 0 {
                // O_TRUNC
                opts.truncate(true);
            }
            let file = opts.open(&path).map_err(|e| errno_from_io(&e))?;
            f.handle = Handle::File(file);
        }

        let mut w = Writer::new(proto::R_LOPEN, tag);
        w.qid(qid);
        w.u32(0); // iounit — 0 lets the client use msize-based default
        Ok(w.finish())
    }

    fn t_lcreate(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        if self.readonly {
            return Err(proto::EACCES);
        }
        let dfid = r.u32().map_err(|_| proto::EINVAL)?;
        let name = r.string().map_err(|_| proto::EINVAL)?;
        let flags = r.u32().map_err(|_| proto::EINVAL)?;
        let mode = r.u32().map_err(|_| proto::EINVAL)?;
        let _gid = r.u32().ok();

        let parent = self.fids.get(dfid).ok_or(proto::EBADF)?.path.clone();
        let target = safe_join(&self.root, &parent, std::slice::from_ref(&name))
            .ok_or(proto::EINVAL)?;

        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create_new(true);
        if (flags & 0o1000) != 0 {
            opts.truncate(true);
        }
        let file = opts.open(&target).map_err(|e| errno_from_io(&e))?;
        set_mode(&target, mode).ok();

        let meta = file.metadata().map_err(|e| errno_from_io(&e))?;
        let qid = qid_from_meta(&meta);

        let f = self.fids.get_mut(dfid).ok_or(proto::EBADF)?;
        f.path = target;
        f.handle = Handle::File(file);

        let mut w = Writer::new(proto::R_LCREATE, tag);
        w.qid(qid);
        w.u32(0);
        Ok(w.finish())
    }

    fn t_read(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let offset = r.u64().map_err(|_| proto::EINVAL)?;
        let count = r.u32().map_err(|_| proto::EINVAL)?;
        let count = count.min(self.msize.saturating_sub(11)) as usize;

        let f = self.fids.get_mut(fid).ok_or(proto::EBADF)?;
        let mut buf = vec![0u8; count];
        let n = match &mut f.handle {
            Handle::File(file) => {
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| errno_from_io(&e))?;
                file.read(&mut buf).map_err(|e| errno_from_io(&e))?
            }
            _ => return Err(proto::EBADF),
        };

        let mut w = Writer::new(proto::R_READ, tag);
        w.u32(n as u32);
        w.raw(&buf[..n]);
        Ok(w.finish())
    }

    fn t_write(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        if self.readonly {
            return Err(proto::EACCES);
        }
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let offset = r.u64().map_err(|_| proto::EINVAL)?;
        let count = r.u32().map_err(|_| proto::EINVAL)? as usize;
        let data = r.bytes(count).map_err(|_| proto::EINVAL)?;

        let f = self.fids.get_mut(fid).ok_or(proto::EBADF)?;
        let n = match &mut f.handle {
            Handle::File(file) => {
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| errno_from_io(&e))?;
                file.write(data).map_err(|e| errno_from_io(&e))?
            }
            _ => return Err(proto::EBADF),
        };

        let mut w = Writer::new(proto::R_WRITE, tag);
        w.u32(n as u32);
        Ok(w.finish())
    }

    fn t_readdir(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let offset = r.u64().map_err(|_| proto::EINVAL)?;
        let count = r.u32().map_err(|_| proto::EINVAL)?;
        let budget = count.min(self.msize.saturating_sub(11)) as usize;

        let f = self.fids.get_mut(fid).ok_or(proto::EBADF)?;
        let snap = match &f.handle {
            Handle::Dir(s) => s,
            _ => return Err(proto::EBADF),
        };

        let mut data = Vec::with_capacity(budget.min(64 * 1024));
        // Each entry: qid[13] offset[8] type[1] name[str].
        let mut idx = offset as usize;
        while idx < snap.entries.len() {
            let (name, qid, dtype) = &snap.entries[idx];
            let entry_size = 13 + 8 + 1 + 2 + name.len();
            if data.len() + entry_size > budget {
                break;
            }
            data.push(qid.qtype);
            data.extend_from_slice(&qid.version.to_le_bytes());
            data.extend_from_slice(&qid.path.to_le_bytes());
            let next_off = (idx + 1) as u64;
            data.extend_from_slice(&next_off.to_le_bytes());
            data.push(*dtype);
            data.extend_from_slice(&(name.len() as u16).to_le_bytes());
            data.extend_from_slice(name.as_bytes());
            idx += 1;
        }

        let mut w = Writer::new(proto::R_READDIR, tag);
        w.u32(data.len() as u32);
        w.raw(&data);
        Ok(w.finish())
    }

    fn t_getattr(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let _request_mask = r.u64().map_err(|_| proto::EINVAL)?;
        let path = self.fids.get(fid).ok_or(proto::EBADF)?.path.clone();
        let meta = fs::symlink_metadata(&path).map_err(|e| errno_from_io(&e))?;
        let qid = qid_from_meta(&meta);

        #[cfg(unix)]
        let (mode, uid, gid, nlink, rdev, size, blksize, blocks, at_s, at_ns, mt_s, mt_ns, ct_s, ct_ns) = {
            use std::os::unix::fs::MetadataExt;
            (
                meta.mode() as u32,
                meta.uid(),
                meta.gid(),
                meta.nlink() as u64,
                meta.rdev() as u64,
                meta.size(),
                meta.blksize(),
                meta.blocks(),
                meta.atime() as u64,
                meta.atime_nsec() as u64,
                meta.mtime() as u64,
                meta.mtime_nsec() as u64,
                meta.ctime() as u64,
                meta.ctime_nsec() as u64,
            )
        };
        #[cfg(not(unix))]
        let (mode, uid, gid, nlink, rdev, size, blksize, blocks, at_s, at_ns, mt_s, mt_ns, ct_s, ct_ns) =
            (0o755u32, 0u32, 0u32, 1u64, 0u64, meta.len(), 4096u64, (meta.len() / 512) as u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);

        let mut w = Writer::new(proto::R_GETATTR, tag);
        w.u64(0x3fff); // valid mask (P9_GETATTR_BASIC)
        w.qid(qid);
        w.u32(mode);
        w.u32(uid);
        w.u32(gid);
        w.u64(nlink);
        w.u64(rdev);
        w.u64(size);
        w.u64(blksize);
        w.u64(blocks);
        w.u64(at_s);
        w.u64(at_ns);
        w.u64(mt_s);
        w.u64(mt_ns);
        w.u64(ct_s);
        w.u64(ct_ns);
        w.u64(0); // btime
        w.u64(0);
        w.u64(0); // gen
        w.u64(0); // data_version
        Ok(w.finish())
    }

    fn t_setattr(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        if self.readonly {
            return Err(proto::EACCES);
        }
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let valid = r.u32().map_err(|_| proto::EINVAL)?;
        let mode = r.u32().map_err(|_| proto::EINVAL)?;
        let _uid = r.u32().map_err(|_| proto::EINVAL)?;
        let _gid = r.u32().map_err(|_| proto::EINVAL)?;
        let size = r.u64().map_err(|_| proto::EINVAL)?;
        // atime/mtime fields follow; we consume but mostly ignore.
        let _ = r.u64().ok();
        let _ = r.u64().ok();
        let _ = r.u64().ok();
        let _ = r.u64().ok();

        let path = self.fids.get(fid).ok_or(proto::EBADF)?.path.clone();

        if (valid & 0x1) != 0 {
            // P9_SETATTR_MODE
            set_mode(&path, mode).map_err(|e| errno_from_io(&e))?;
        }
        if (valid & 0x8) != 0 {
            // P9_SETATTR_SIZE
            let f = OpenOptions::new()
                .write(true)
                .open(&path)
                .map_err(|e| errno_from_io(&e))?;
            f.set_len(size).map_err(|e| errno_from_io(&e))?;
        }

        Ok(Writer::new(proto::R_SETATTR, tag).finish())
    }

    fn t_mkdir(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        if self.readonly {
            return Err(proto::EACCES);
        }
        let dfid = r.u32().map_err(|_| proto::EINVAL)?;
        let name = r.string().map_err(|_| proto::EINVAL)?;
        let mode = r.u32().map_err(|_| proto::EINVAL)?;
        let _gid = r.u32().ok();

        let parent = self.fids.get(dfid).ok_or(proto::EBADF)?.path.clone();
        let target = safe_join(&self.root, &parent, std::slice::from_ref(&name))
            .ok_or(proto::EINVAL)?;

        fs::create_dir(&target).map_err(|e| errno_from_io(&e))?;
        set_mode(&target, mode).ok();
        let meta = fs::metadata(&target).map_err(|e| errno_from_io(&e))?;

        let mut w = Writer::new(proto::R_MKDIR, tag);
        w.qid(qid_from_meta(&meta));
        Ok(w.finish())
    }

    fn t_rename(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        if self.readonly {
            return Err(proto::EACCES);
        }
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let dfid = r.u32().map_err(|_| proto::EINVAL)?;
        let name = r.string().map_err(|_| proto::EINVAL)?;

        let src = self.fids.get(fid).ok_or(proto::EBADF)?.path.clone();
        let new_parent = self.fids.get(dfid).ok_or(proto::EBADF)?.path.clone();
        let dest = safe_join(&self.root, &new_parent, std::slice::from_ref(&name))
            .ok_or(proto::EINVAL)?;

        fs::rename(&src, &dest).map_err(|e| errno_from_io(&e))?;
        // Update the source fid's path so further ops target the new location.
        if let Some(f) = self.fids.get_mut(fid) {
            f.path = dest;
        }
        Ok(Writer::new(proto::R_RENAME, tag).finish())
    }

    fn t_renameat(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        if self.readonly {
            return Err(proto::EACCES);
        }
        let olddfid = r.u32().map_err(|_| proto::EINVAL)?;
        let oldname = r.string().map_err(|_| proto::EINVAL)?;
        let newdfid = r.u32().map_err(|_| proto::EINVAL)?;
        let newname = r.string().map_err(|_| proto::EINVAL)?;

        let old_parent = self.fids.get(olddfid).ok_or(proto::EBADF)?.path.clone();
        let new_parent = self.fids.get(newdfid).ok_or(proto::EBADF)?.path.clone();
        let src = safe_join(&self.root, &old_parent, std::slice::from_ref(&oldname))
            .ok_or(proto::EINVAL)?;
        let dest = safe_join(&self.root, &new_parent, std::slice::from_ref(&newname))
            .ok_or(proto::EINVAL)?;

        fs::rename(&src, &dest).map_err(|e| errno_from_io(&e))?;
        Ok(Writer::new(proto::R_RENAMEAT, tag).finish())
    }

    fn t_unlinkat(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        if self.readonly {
            return Err(proto::EACCES);
        }
        let dfid = r.u32().map_err(|_| proto::EINVAL)?;
        let name = r.string().map_err(|_| proto::EINVAL)?;
        let flags = r.u32().map_err(|_| proto::EINVAL)?;

        let parent = self.fids.get(dfid).ok_or(proto::EBADF)?.path.clone();
        let target = safe_join(&self.root, &parent, std::slice::from_ref(&name))
            .ok_or(proto::EINVAL)?;

        let meta = fs::symlink_metadata(&target).map_err(|e| errno_from_io(&e))?;
        // AT_REMOVEDIR = 0x200
        if (flags & 0x200) != 0 || meta.is_dir() {
            fs::remove_dir(&target).map_err(|e| errno_from_io(&e))?;
        } else {
            fs::remove_file(&target).map_err(|e| errno_from_io(&e))?;
        }
        Ok(Writer::new(proto::R_UNLINKAT, tag).finish())
    }

    fn t_remove(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        if self.readonly {
            return Err(proto::EACCES);
        }
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let f = self.fids.remove(fid).ok_or(proto::EBADF)?;
        let res = if f.path == self.root {
            Err(proto::EACCES)
        } else {
            drop(f.handle); // close before unlink (matters on Windows, harmless on Unix)
            let meta = fs::symlink_metadata(&f.path).map_err(|e| errno_from_io(&e))?;
            if meta.is_dir() {
                fs::remove_dir(&f.path).map_err(|e| errno_from_io(&e))
            } else {
                fs::remove_file(&f.path).map_err(|e| errno_from_io(&e))
            }
        };
        res?;
        Ok(Writer::new(proto::R_REMOVE, tag).finish())
    }

    fn t_clunk(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        self.fids.remove(fid);
        Ok(Writer::new(proto::R_CLUNK, tag).finish())
    }

    fn t_fsync(&mut self, tag: u16, r: &mut Reader) -> Result<Vec<u8>, u32> {
        let fid = r.u32().map_err(|_| proto::EINVAL)?;
        let _datasync = r.u32().ok();
        let f = self.fids.get(fid).ok_or(proto::EBADF)?;
        if let Handle::File(file) = &f.handle {
            file.sync_all().map_err(|e| errno_from_io(&e))?;
        }
        Ok(Writer::new(proto::R_FSYNC, tag).finish())
    }

    fn t_statfs(&mut self, tag: u16, _r: &mut Reader) -> Result<Vec<u8>, u32> {
        // Minimal fake values. Guest only really cares that this returns.
        let mut w = Writer::new(proto::R_STATFS, tag);
        w.u32(0x01021997); // V9FS_MAGIC
        w.u32(4096); // bsize
        w.u64(1 << 40); // blocks
        w.u64(1 << 39); // bfree
        w.u64(1 << 39); // bavail
        w.u64(1 << 30); // files
        w.u64(1 << 29); // ffree
        w.u64(0); // fsid
        w.u32(255); // namelen
        Ok(w.finish())
    }
}

fn snapshot_dir(path: &Path) -> Result<DirSnapshot, u32> {
    let mut entries = Vec::new();

    // "." and ".." entries — many clients expect them in 9P2000.L readdir.
    if let Ok(m) = fs::metadata(path) {
        let q = qid_from_meta(&m);
        entries.push((".".to_string(), q, 4)); // DT_DIR
    }
    if let Some(parent) = path.parent() {
        if let Ok(m) = fs::metadata(parent) {
            let q = qid_from_meta(&m);
            entries.push(("..".to_string(), q, 4));
        }
    }

    let rd = fs::read_dir(path).map_err(|e| errno_from_io(&e))?;
    let mut kids: Vec<_> = Vec::new();
    for entry in rd.flatten() {
        let name = match entry.file_name().into_string() {
            Ok(s) => s,
            Err(_) => continue, // skip non-utf8 names
        };
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let qid = qid_from_meta(&meta);
        let dtype = if meta.is_dir() {
            4u8 // DT_DIR
        } else if meta.file_type().is_symlink() {
            10u8 // DT_LNK
        } else {
            8u8 // DT_REG
        };
        kids.push((name, qid, dtype));
    }
    kids.sort_by(|a, b| a.0.cmp(&b.0));
    entries.extend(kids);

    Ok(DirSnapshot { entries })
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(mode & 0o7777);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}
