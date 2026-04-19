// traits/kernel/runtimes/src/native.rs
// Native runtime backend using tokio and std::fs

use crate::{File, Filesystem, Listener, Process, ProcessOutput, RuntimeContext, RuntimeError, Socket, SocketFactory, Timer, DirEntry, FileMetadata};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout, Duration};

// ============================================================================
// Native Socket Implementation
// ============================================================================

pub struct NativeSocket {
    stream: TcpStream,
}

#[async_trait]
impl Socket for NativeSocket {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, RuntimeError> {
        self.stream.read(buf).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), RuntimeError> {
        self.stream.write_all(buf).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn shutdown(&mut self) -> Result<(), RuntimeError> {
        self.stream.shutdown().await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    fn peer_addr(&self) -> Result<SocketAddr, RuntimeError> {
        self.stream.peer_addr().map_err(|e| RuntimeError::Io(e.to_string()))
    }
}

// ============================================================================
// Native Listener Implementation
// ============================================================================

pub struct NativeListener {
    listener: TcpListener,
}

#[async_trait]
impl Listener for NativeListener {
    async fn accept(&mut self) -> Result<(Box<dyn Socket>, SocketAddr), RuntimeError> {
        let (stream, addr) = self.listener.accept().await.map_err(|e| RuntimeError::Io(e.to_string()))?;
        Ok((Box::new(NativeSocket { stream }), addr))
    }

    fn local_addr(&self) -> Result<SocketAddr, RuntimeError> {
        self.listener.local_addr().map_err(|e| RuntimeError::Io(e.to_string()))
    }
}

// ============================================================================
// Native Socket Factory
// ============================================================================

pub struct NativeSocketFactory;

#[async_trait]
impl SocketFactory for NativeSocketFactory {
    async fn bind(&self, addr: SocketAddr) -> Result<Box<dyn Listener>, RuntimeError> {
        let listener = TcpListener::bind(addr).await.map_err(|e| RuntimeError::Io(e.to_string()))?;
        Ok(Box::new(NativeListener { listener }))
    }

    async fn connect(&self, addr: SocketAddr) -> Result<Box<dyn Socket>, RuntimeError> {
        let stream = TcpStream::connect(addr).await.map_err(|e| RuntimeError::Io(e.to_string()))?;
        Ok(Box::new(NativeSocket { stream }))
    }
}

// ============================================================================
// Native File Implementation
// ============================================================================

pub struct NativeFile {
    file: tokio::fs::File,
}

#[async_trait]
impl File for NativeFile {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, RuntimeError> {
        self.file.read(buf).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), RuntimeError> {
        self.file.write_all(buf).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn seek(&mut self, pos: u64) -> Result<u64, RuntimeError> {
        use tokio::io::AsyncSeekExt;
        self.file.seek(std::io::SeekFrom::Start(pos)).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn flush(&mut self) -> Result<(), RuntimeError> {
        self.file.flush().await.map_err(|e| RuntimeError::Io(e.to_string()))
    }
}

// ============================================================================
// Native Filesystem Implementation
// ============================================================================

pub struct NativeFilesystem {
    root: PathBuf,
}

impl NativeFilesystem {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            root: root.unwrap_or_else(|| PathBuf::from(".")),
        }
    }

    fn resolve_path(&self, path: &str) -> Result<PathBuf, RuntimeError> {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            return Err(RuntimeError::PathNotAllowed(format!("Absolute paths not allowed: {}", path)));
        }
        let resolved = self.root.join(&p);
        if !resolved.starts_with(&self.root) {
            return Err(RuntimeError::PathNotAllowed(format!("Path escape attempt: {}", path)));
        }
        Ok(resolved)
    }
}

#[async_trait]
impl Filesystem for NativeFilesystem {
    async fn open(&self, path: &str, write: bool) -> Result<Box<dyn File>, RuntimeError> {
        let resolved = self.resolve_path(path)?;
        let file = if write {
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&resolved)
                .await
        } else {
            fs::File::open(&resolved).await
        }
        .map_err(|e| RuntimeError::Io(e.to_string()))?;
        Ok(Box::new(NativeFile { file }))
    }

    async fn create(&self, path: &str) -> Result<Box<dyn File>, RuntimeError> {
        let resolved = self.resolve_path(path)?;
        let file = fs::File::create(&resolved).await.map_err(|e| RuntimeError::Io(e.to_string()))?;
        Ok(Box::new(NativeFile { file }))
    }

    async fn read_to_string(&self, path: &str) -> Result<String, RuntimeError> {
        let resolved = self.resolve_path(path)?;
        fs::read_to_string(&resolved).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn write(&self, path: &str, data: &[u8]) -> Result<(), RuntimeError> {
        let resolved = self.resolve_path(path)?;
        fs::write(&resolved, data).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn mkdir(&self, path: &str) -> Result<(), RuntimeError> {
        let resolved = self.resolve_path(path)?;
        fs::create_dir_all(&resolved).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn remove_file(&self, path: &str) -> Result<(), RuntimeError> {
        let resolved = self.resolve_path(path)?;
        fs::remove_file(&resolved).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn remove_dir(&self, path: &str) -> Result<(), RuntimeError> {
        let resolved = self.resolve_path(path)?;
        fs::remove_dir_all(&resolved).await.map_err(|e| RuntimeError::Io(e.to_string()))
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, RuntimeError> {
        let resolved = self.resolve_path(path)?;
        let mut entries = Vec::new();
        let mut dir = fs::read_dir(&resolved).await.map_err(|e| RuntimeError::Io(e.to_string()))?;

        while let Some(entry) = dir.next_entry().await.map_err(|e| RuntimeError::Io(e.to_string()))? {
            let metadata = entry.metadata().await.map_err(|e| RuntimeError::Io(e.to_string()))?;
            entries.push(DirEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
            });
        }

        Ok(entries)
    }

    async fn metadata(&self, path: &str) -> Result<FileMetadata, RuntimeError> {
        let resolved = self.resolve_path(path)?;
        let m = fs::metadata(&resolved).await.map_err(|e| RuntimeError::Io(e.to_string()))?;
        Ok(FileMetadata {
            is_dir: m.is_dir(),
            size: m.len(),
            is_readable: true,
            is_writable: !m.permissions().readonly(),
        })
    }
}

// ============================================================================
// Native Timer Implementation
// ============================================================================

pub struct NativeTimer;

#[async_trait]
impl Timer for NativeTimer {
    async fn sleep(&self, millis: u64) -> Result<(), RuntimeError> {
        sleep(Duration::from_millis(millis)).await;
        Ok(())
    }
}

// ============================================================================
// Native Process Implementation
// ============================================================================

pub struct NativeProcess;

#[async_trait]
impl Process for NativeProcess {
    async fn spawn(&self, cmd: &str, args: &[&str]) -> Result<ProcessOutput, RuntimeError> {
        let output = tokio::process::Command::new(cmd)
            .args(args)
            .output()
            .await
            .map_err(|e| RuntimeError::Io(e.to_string()))?;

        Ok(ProcessOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

// ============================================================================
// Factory Function
// ============================================================================

pub fn init_native_runtime(root: Option<PathBuf>) -> RuntimeContext {
    RuntimeContext {
        socket_factory: Arc::new(NativeSocketFactory),
        filesystem: Arc::new(NativeFilesystem::new(root)),
        timer: Arc::new(NativeTimer),
        process: Arc::new(NativeProcess),
    }
}
