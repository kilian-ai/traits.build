//! 9P2000.L message framing, encode, decode.
//!
//! Wire format: little-endian. Every message starts with
//!   size[4] type[1] tag[2]
//! where `size` includes the 7-byte header. Strings are u16 length + utf8.

#![allow(dead_code)]

use std::io::{self, Read, Write};

pub const NOTAG: u16 = 0xFFFF;
pub const NOFID: u32 = 0xFFFFFFFF;

// Message type codes (9P2000.L subset).
pub const R_LERROR: u8 = 7;
pub const T_STATFS: u8 = 8;
pub const R_STATFS: u8 = 9;
pub const T_LOPEN: u8 = 12;
pub const R_LOPEN: u8 = 13;
pub const T_LCREATE: u8 = 14;
pub const R_LCREATE: u8 = 15;
pub const T_RENAME: u8 = 20;
pub const R_RENAME: u8 = 21;
pub const T_GETATTR: u8 = 24;
pub const R_GETATTR: u8 = 25;
pub const T_SETATTR: u8 = 26;
pub const R_SETATTR: u8 = 27;
pub const T_XATTRWALK: u8 = 30;
pub const T_READDIR: u8 = 40;
pub const R_READDIR: u8 = 41;
pub const T_FSYNC: u8 = 50;
pub const R_FSYNC: u8 = 51;
pub const T_MKDIR: u8 = 72;
pub const R_MKDIR: u8 = 73;
pub const T_RENAMEAT: u8 = 74;
pub const R_RENAMEAT: u8 = 75;
pub const T_UNLINKAT: u8 = 76;
pub const R_UNLINKAT: u8 = 77;
pub const T_VERSION: u8 = 100;
pub const R_VERSION: u8 = 101;
pub const T_AUTH: u8 = 102;
pub const T_ATTACH: u8 = 104;
pub const R_ATTACH: u8 = 105;
pub const T_FLUSH: u8 = 108;
pub const R_FLUSH: u8 = 109;
pub const T_WALK: u8 = 110;
pub const R_WALK: u8 = 111;
pub const T_READ: u8 = 116;
pub const R_READ: u8 = 117;
pub const T_WRITE: u8 = 118;
pub const R_WRITE: u8 = 119;
pub const T_CLUNK: u8 = 120;
pub const R_CLUNK: u8 = 121;
pub const T_REMOVE: u8 = 122;
pub const R_REMOVE: u8 = 123;

// Qid type bits.
pub const QT_DIR: u8 = 0x80;
pub const QT_SYMLINK: u8 = 0x02;
pub const QT_FILE: u8 = 0x00;

// errno values (Linux).
pub const EPERM: u32 = 1;
pub const ENOENT: u32 = 2;
pub const EIO: u32 = 5;
pub const EBADF: u32 = 9;
pub const EACCES: u32 = 13;
pub const EEXIST: u32 = 17;
pub const ENOTDIR: u32 = 20;
pub const EISDIR: u32 = 21;
pub const EINVAL: u32 = 22;
pub const ENOSPC: u32 = 28;
pub const ENOTEMPTY: u32 = 39;
pub const ENOTSUP: u32 = 95;

#[derive(Debug, Clone, Copy)]
pub struct Qid {
    pub qtype: u8,
    pub version: u32,
    pub path: u64,
}

pub fn errno_from_io(e: &io::Error) -> u32 {
    use io::ErrorKind::*;
    match e.kind() {
        NotFound => ENOENT,
        PermissionDenied => EACCES,
        AlreadyExists => EEXIST,
        InvalidInput | InvalidData => EINVAL,
        _ => e.raw_os_error().map(|n| n as u32).unwrap_or(EIO),
    }
}

/// Reader that advances through a message body.
pub struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }
    pub fn remaining(&self) -> usize {
        self.buf.len()
    }
    pub fn u8(&mut self) -> io::Result<u8> {
        if self.buf.is_empty() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "u8"));
        }
        let v = self.buf[0];
        self.buf = &self.buf[1..];
        Ok(v)
    }
    pub fn u16(&mut self) -> io::Result<u16> {
        if self.buf.len() < 2 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "u16"));
        }
        let v = u16::from_le_bytes([self.buf[0], self.buf[1]]);
        self.buf = &self.buf[2..];
        Ok(v)
    }
    pub fn u32(&mut self) -> io::Result<u32> {
        if self.buf.len() < 4 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "u32"));
        }
        let v = u32::from_le_bytes(self.buf[0..4].try_into().unwrap());
        self.buf = &self.buf[4..];
        Ok(v)
    }
    pub fn u64(&mut self) -> io::Result<u64> {
        if self.buf.len() < 8 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "u64"));
        }
        let v = u64::from_le_bytes(self.buf[0..8].try_into().unwrap());
        self.buf = &self.buf[8..];
        Ok(v)
    }
    pub fn string(&mut self) -> io::Result<String> {
        let n = self.u16()? as usize;
        if self.buf.len() < n {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "string"));
        }
        let s = std::str::from_utf8(&self.buf[..n])
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "utf8"))?
            .to_string();
        self.buf = &self.buf[n..];
        Ok(s)
    }
    pub fn bytes(&mut self, n: usize) -> io::Result<&'a [u8]> {
        if self.buf.len() < n {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "bytes"));
        }
        let s = &self.buf[..n];
        self.buf = &self.buf[n..];
        Ok(s)
    }
}

/// Builder for reply bodies. Prepends the 7-byte header on `finish`.
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new(kind: u8, tag: u16) -> Self {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(&0u32.to_le_bytes()); // size placeholder
        buf.push(kind);
        buf.extend_from_slice(&tag.to_le_bytes());
        Self { buf }
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn string(&mut self, s: &str) {
        let b = s.as_bytes();
        self.u16(b.len() as u16);
        self.buf.extend_from_slice(b);
    }
    pub fn raw(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }
    pub fn qid(&mut self, q: Qid) {
        self.u8(q.qtype);
        self.u32(q.version);
        self.u64(q.path);
    }
    pub fn finish(mut self) -> Vec<u8> {
        let size = self.buf.len() as u32;
        self.buf[0..4].copy_from_slice(&size.to_le_bytes());
        self.buf
    }
}

/// Build an Rlerror message for the given request tag.
pub fn rlerror(tag: u16, ecode: u32) -> Vec<u8> {
    let mut w = Writer::new(R_LERROR, tag);
    w.u32(ecode);
    w.finish()
}

/// Read one framed message. Returns (type, tag, body).
pub fn read_frame<R: Read>(r: &mut R, max: u32) -> io::Result<(u8, u16, Vec<u8>)> {
    let mut hdr = [0u8; 7];
    r.read_exact(&mut hdr)?;
    let size = u32::from_le_bytes(hdr[0..4].try_into().unwrap());
    let kind = hdr[4];
    let tag = u16::from_le_bytes([hdr[5], hdr[6]]);
    if size < 7 || size > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad frame size {size}"),
        ));
    }
    let body_len = (size - 7) as usize;
    let mut body = vec![0u8; body_len];
    r.read_exact(&mut body)?;
    Ok((kind, tag, body))
}

pub fn write_frame<W: Write>(w: &mut W, frame: &[u8]) -> io::Result<()> {
    w.write_all(frame)?;
    w.flush()
}
