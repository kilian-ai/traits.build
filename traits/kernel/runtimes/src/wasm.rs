// traits/kernel/runtimes/src/wasm.rs
// WASM runtime backend using relay bridge

use crate::{File, Filesystem, Listener, Process, ProcessOutput, RuntimeContext, RuntimeError, Socket, SocketFactory, Timer, DirEntry, FileMetadata};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use serde_json::json;

// ============================================================================
// WASM Relay Bridge Client
// ============================================================================

pub struct RelayClient {
    relay_url: String,
    pairing_code: String,
}

impl RelayClient {
    pub fn new(relay_url: String, pairing_code: String) -> Self {
        Self { relay_url, pairing_code }
    }

    async fn call(&self, path: &str, args: Vec<serde_json::Value>) -> Result<serde_json::Value, RuntimeError> {
        let _body = json!({
            "code": self.pairing_code,
            "path": path,
            "args": args
        });

        // In a real browser, this would use wasm_bindgen to call fetch
        // For now, return a simulated response for testing
        log::info!("Relay call: {} with args: {:?}", path, args);
        Ok(json!({"ok": true}))
    }
}

// ============================================================================
// WASM Socket Implementation (stub for relay bridge)
// ============================================================================

pub struct WasmSocket {
    relay: Arc<RelayClient>,
    peer_addr: SocketAddr,
    buffer: Vec<u8>,
    closed: bool,
}

#[async_trait]
impl Socket for WasmSocket {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, RuntimeError> {
        if self.closed {
            return Ok(0);
        }

        // Simulated read from buffer
        let n = std::cmp::min(buf.len(), self.buffer.len());
        buf[..n].copy_from_slice(&self.buffer[..n]);
        self.buffer.drain(..n);
        Ok(n)
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), RuntimeError> {
        if self.closed {
            return Err(RuntimeError::Io("Socket closed".into()));
        }

        // In a real implementation, this would dispatch through relay
        log::info!("WASM write {} bytes to {}", buf.len(), self.peer_addr);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), RuntimeError> {
        self.closed = true;
        Ok(())
    }

    fn peer_addr(&self) -> Result<SocketAddr, RuntimeError> {
        Ok(self.peer_addr)
    }
}

// ============================================================================
// WASM Listener Implementation (stub)
// ============================================================================

pub struct WasmListener {
    relay: Arc<RelayClient>,
    addr: SocketAddr,
}

#[async_trait]
impl Listener for WasmListener {
    async fn accept(&mut self) -> Result<(Box<dyn Socket>, SocketAddr), RuntimeError> {
        let peer_addr = "127.0.0.1:9999".parse().map_err(|_| RuntimeError::Io("Parse error".into()))?;
        let socket = WasmSocket {
            relay: self.relay.clone(),
            peer_addr,
            buffer: vec![],
            closed: false,
        };
        Ok((Box::new(socket), peer_addr))
    }

    fn local_addr(&self) -> Result<SocketAddr, RuntimeError> {
        Ok(self.addr)
    }
}

// ============================================================================
// WASM Socket Factory
// ============================================================================

pub struct WasmSocketFactory {
    relay: Arc<RelayClient>,
}

impl WasmSocketFactory {
    pub fn new(relay: Arc<RelayClient>) -> Self {
        Self { relay }
    }
}

#[async_trait]
impl SocketFactory for WasmSocketFactory {
    async fn bind(&self, addr: SocketAddr) -> Result<Box<dyn Listener>, RuntimeError> {
        Ok(Box::new(WasmListener {
            relay: self.relay.clone(),
            addr,
        }))
    }

    async fn connect(&self, addr: SocketAddr) -> Result<Box<dyn Socket>, RuntimeError> {
        Ok(Box::new(WasmSocket {
            relay: self.relay.clone(),
            peer_addr: addr,
            buffer: vec![],
            closed: false,
        }))
    }
}

// ============================================================================
// WASM File Implementation
// ============================================================================

pub struct WasmFile {
    relay: Arc<RelayClient>,
    path: String,
    buffer: Vec<u8>,
    position: u64,
}

#[async_trait]
impl File for WasmFile {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, RuntimeError> {
        // Call relay to read from linux.vfs
        let result = self.relay.call("linux.vfs.read", vec![
            serde_json::Value::String(self.path.clone()),
        ]).await?;

        let content = result.get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_str())
            .ok_or(RuntimeError::Io("Invalid response".into()))?;

        let bytes = content.as_bytes();
        let n = std::cmp::min(buf.len(), bytes.len().saturating_sub(self.position as usize));
        buf[..n].copy_from_slice(&bytes[self.position as usize..self.position as usize + n]);
        self.position += n as u64;
        Ok(n)
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), RuntimeError> {
        let content = String::from_utf8_lossy(buf).into_owned();
        self.relay.call("linux.vfs.write", vec![
            serde_json::Value::String(self.path.clone()),
            serde_json::Value::String(content),
            serde_json::Value::String("truncate".into()),
        ]).await?;
        Ok(())
    }

    async fn seek(&mut self, pos: u64) -> Result<u64, RuntimeError> {
        self.position = pos;
        Ok(pos)
    }

    async fn flush(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }
}

// ============================================================================
// WASM Filesystem Implementation
// ============================================================================

pub struct WasmFilesystem {
    relay: Arc<RelayClient>,
}

impl WasmFilesystem {
    pub fn new(relay: Arc<RelayClient>) -> Self {
        Self { relay }
    }
}

#[async_trait]
impl Filesystem for WasmFilesystem {
    async fn open(&self, path: &str, _write: bool) -> Result<Box<dyn File>, RuntimeError> {
        Ok(Box::new(WasmFile {
            relay: self.relay.clone(),
            path: path.into(),
            buffer: vec![],
            position: 0,
        }))
    }

    async fn create(&self, path: &str) -> Result<Box<dyn File>, RuntimeError> {
        self.relay.call("linux.vfs.write", vec![
            serde_json::Value::String(path.into()),
            serde_json::Value::String("".into()),
            serde_json::Value::String("truncate".into()),
        ]).await?;

        Ok(Box::new(WasmFile {
            relay: self.relay.clone(),
            path: path.into(),
            buffer: vec![],
            position: 0,
        }))
    }

    async fn read_to_string(&self, path: &str) -> Result<String, RuntimeError> {
        let result = self.relay.call("linux.vfs.read", vec![
            serde_json::Value::String(path.into()),
        ]).await?;

        result.get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            .ok_or(RuntimeError::Io("Failed to read file".into()))
    }

    async fn write(&self, path: &str, data: &[u8]) -> Result<(), RuntimeError> {
        let content = String::from_utf8_lossy(data).into_owned();
        self.relay.call("linux.vfs.write", vec![
            serde_json::Value::String(path.into()),
            serde_json::Value::String(content),
            serde_json::Value::String("truncate".into()),
        ]).await?;
        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<(), RuntimeError> {
        self.relay.call("linux.vfs.mkdir", vec![
            serde_json::Value::String(path.into()),
        ]).await?;
        Ok(())
    }

    async fn remove_file(&self, path: &str) -> Result<(), RuntimeError> {
        self.relay.call("linux.vfs.delete", vec![
            serde_json::Value::String(path.into()),
        ]).await?;
        Ok(())
    }

    async fn remove_dir(&self, path: &str) -> Result<(), RuntimeError> {
        self.relay.call("linux.vfs.delete", vec![
            serde_json::Value::String(path.into()),
        ]).await?;
        Ok(())
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, RuntimeError> {
        let result = self.relay.call("linux.vfs.list", vec![
            serde_json::Value::String(path.into()),
        ]).await?;

        let mut entries = Vec::new();
        if let Some(files) = result.get("result").and_then(|r| r.get("files")).and_then(|f| f.as_array()) {
            for file in files {
                if let (Some(name), Some(bytes)) = (file.get("name").and_then(|n| n.as_str()), file.get("bytes").and_then(|b| b.as_u64())) {
                    entries.push(DirEntry {
                        name: name.to_string(),
                        is_dir: false,
                        size: bytes,
                    });
                }
            }
        }
        Ok(entries)
    }

    async fn metadata(&self, path: &str) -> Result<FileMetadata, RuntimeError> {
        let result = self.relay.call("linux.vfs.list", vec![
            serde_json::Value::String(path.into()),
        ]).await?;

        Ok(FileMetadata {
            is_dir: result.get("result").and_then(|r| r.get("is_dir")).and_then(|d| d.as_bool()).unwrap_or(false),
            size: result.get("result").and_then(|r| r.get("size")).and_then(|s| s.as_u64()).unwrap_or(0),
            is_readable: true,
            is_writable: true,
        })
    }
}

// ============================================================================
// WASM Timer Implementation
// ============================================================================

pub struct WasmTimer;

#[async_trait]
impl Timer for WasmTimer {
    async fn sleep(&self, millis: u64) -> Result<(), RuntimeError> {
        // In real WASM, this would use web_sys::window and Promise
        log::info!("Simulated sleep: {} ms", millis);
        Ok(())
    }
}

// ============================================================================
// WASM Process Implementation
// ============================================================================

pub struct WasmProcess {
    relay: Arc<RelayClient>,
}

#[async_trait]
impl Process for WasmProcess {
    async fn spawn(&self, cmd: &str, args: &[&str]) -> Result<ProcessOutput, RuntimeError> {
        let full_cmd = format!("{} {}", cmd, args.join(" "));
        let result = self.relay.call("linux.exec", vec![
            serde_json::Value::String(full_cmd),
        ]).await?;

        Ok(ProcessOutput {
            exit_code: result.get("result").and_then(|r| r.get("exitCode")).and_then(|e| e.as_i64()).unwrap_or(-1) as i32,
            stdout: result.get("result").and_then(|r| r.get("output")).and_then(|o| o.as_str()).unwrap_or("").into(),
            stderr: "".into(),
        })
    }
}

// ============================================================================
// Factory Function
// ============================================================================

pub fn init_wasm_runtime(relay_url: String, pairing_code: String) -> RuntimeContext {
    let relay = Arc::new(RelayClient::new(relay_url, pairing_code));
    RuntimeContext {
        socket_factory: Arc::new(WasmSocketFactory::new(relay.clone())),
        filesystem: Arc::new(WasmFilesystem::new(relay.clone())),
        timer: Arc::new(WasmTimer),
        process: Arc::new(WasmProcess { relay }),
    }
}
