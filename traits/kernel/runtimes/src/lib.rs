// traits/kernel/runtimes/src/lib.rs
// Core abstract I/O traits that work across native, WASM, WASI, and guest targets

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::net::SocketAddr;
use std::path::Path;
use thiserror::Error;

pub mod native;
pub mod wasm;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeError {
    #[error("IO error: {0}")]
    Io(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Timeout")]
    Timeout,

    #[error("Unsupported operation: {0}")]
    Unsupported(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Path not allowed: {0}")]
    PathNotAllowed(String),
}

// ============================================================================
// Socket Abstraction
// ============================================================================

#[async_trait]
pub trait Socket: Send + Sync {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, RuntimeError>;
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), RuntimeError>;
    async fn shutdown(&mut self) -> Result<(), RuntimeError>;
    fn peer_addr(&self) -> Result<SocketAddr, RuntimeError>;
}

#[async_trait]
pub trait Listener: Send + Sync {
    async fn accept(&mut self) -> Result<(Box<dyn Socket>, SocketAddr), RuntimeError>;
    fn local_addr(&self) -> Result<SocketAddr, RuntimeError>;
}

#[async_trait]
pub trait SocketFactory: Send + Sync {
    async fn bind(&self, addr: SocketAddr) -> Result<Box<dyn Listener>, RuntimeError>;
    async fn connect(&self, addr: SocketAddr) -> Result<Box<dyn Socket>, RuntimeError>;
}

// ============================================================================
// Filesystem Abstraction
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub is_dir: bool,
    pub size: u64,
    pub is_readable: bool,
    pub is_writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[async_trait]
pub trait File: Send + Sync {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, RuntimeError>;
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), RuntimeError>;
    async fn seek(&mut self, pos: u64) -> Result<u64, RuntimeError>;
    async fn flush(&mut self) -> Result<(), RuntimeError>;
}

#[async_trait]
pub trait Filesystem: Send + Sync {
    async fn open(&self, path: &str, write: bool) -> Result<Box<dyn File>, RuntimeError>;
    async fn create(&self, path: &str) -> Result<Box<dyn File>, RuntimeError>;
    async fn read_to_string(&self, path: &str) -> Result<String, RuntimeError>;
    async fn write(&self, path: &str, data: &[u8]) -> Result<(), RuntimeError>;
    async fn mkdir(&self, path: &str) -> Result<(), RuntimeError>;
    async fn remove_file(&self, path: &str) -> Result<(), RuntimeError>;
    async fn remove_dir(&self, path: &str) -> Result<(), RuntimeError>;
    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, RuntimeError>;
    async fn metadata(&self, path: &str) -> Result<FileMetadata, RuntimeError>;
}

// ============================================================================
// Timer Abstraction
// ============================================================================

#[async_trait]
pub trait Timer: Send + Sync {
    async fn sleep(&self, millis: u64) -> Result<(), RuntimeError>;
}

// ============================================================================
// Process Abstraction
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait Process: Send + Sync {
    async fn spawn(&self, cmd: &str, args: &[&str]) -> Result<ProcessOutput, RuntimeError>;
}

// ============================================================================
// Runtime Context (main entry point)
// ============================================================================

pub struct RuntimeContext {
    pub socket_factory: std::sync::Arc<dyn SocketFactory>,
    pub filesystem: std::sync::Arc<dyn Filesystem>,
    pub timer: std::sync::Arc<dyn Timer>,
    pub process: std::sync::Arc<dyn Process>,
}

pub static RUNTIME: OnceLock<RuntimeContext> = OnceLock::new();

pub fn get_runtime() -> Result<&'static RuntimeContext, RuntimeError> {
    RUNTIME.get().ok_or(RuntimeError::InvalidState("Runtime not initialized".into()))
}

pub fn init_runtime(ctx: RuntimeContext) -> Result<(), RuntimeError> {
    RUNTIME
        .set(ctx)
        .map_err(|_| RuntimeError::InvalidState("Runtime already initialized".into()))
}

// ============================================================================
// Test Utilities
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = RuntimeError::Io("test".into());
        assert_eq!(err.to_string(), "IO error: test");
    }
}
