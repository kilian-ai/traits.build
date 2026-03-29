use crate::registry::Registry;
use crate::types::{TraitType, TraitValue};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use thiserror::Error;
use tokio::sync::mpsc;

// ── PID file management for background traits ──
// Only available on native targets (not WASM)

#[cfg(not(target_arch = "wasm32"))]
/// Directory for PID files (relative to working directory)
const RUN_DIR: &str = ".run";

#[cfg(not(target_arch = "wasm32"))]
/// Get the PID file path for a trait
fn pid_file_path(trait_path: &str) -> std::path::PathBuf {
    std::path::Path::new(RUN_DIR).join(format!("{}.pid", trait_path))
}

/// Write current process PID to the run file
#[cfg(not(target_arch = "wasm32"))]
fn write_pid_file(trait_path: &str) {
    let path = pid_file_path(trait_path);
    if let Err(e) = std::fs::create_dir_all(RUN_DIR) {
        tracing::warn!("Could not create {}: {}", RUN_DIR, e);
        return;
    }
    match std::fs::File::create(&path) {
        Ok(mut f) => {
            let _ = writeln!(f, "{}", std::process::id());
        }
        Err(e) => tracing::warn!("Could not write PID file {:?}: {}", path, e),
    }
}

/// Read PID from a run file, returns None if missing or unreadable
#[cfg(not(target_arch = "wasm32"))]
fn read_pid_file(trait_path: &str) -> Option<u32> {
    let path = pid_file_path(trait_path);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

/// Remove the PID file for a trait
#[cfg(not(target_arch = "wasm32"))]
pub fn remove_pid_file(trait_path: &str) {
    let path = pid_file_path(trait_path);
    let _ = std::fs::remove_file(path);
}

/// Check if a process is alive
#[cfg(not(target_arch = "wasm32"))]
fn process_alive(pid: u32) -> bool {
    // kill(pid, 0) checks existence without sending a signal
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// Result of a stop attempt — richer than bool so callers can report accurately.
#[cfg(not(target_arch = "wasm32"))]
pub enum StopResult {
    /// Process was alive and has been killed
    Killed { pid: u32 },
    /// PID file existed but process was already dead — cleaned up stale file
    StaleCleanup { pid: u32 },
    /// No PID file found for this trait
    NotFound,
    /// PID matches current process — won't kill self
    IsSelf,
}

/// Stop an existing background trait by PID file.
#[cfg(not(target_arch = "wasm32"))]
pub fn stop_existing(trait_path: &str) -> StopResult {
    let pid = match read_pid_file(trait_path) {
        Some(p) => p,
        None => return StopResult::NotFound,
    };
    // Don't kill ourselves
    if pid == std::process::id() {
        return StopResult::IsSelf;
    }
    if !process_alive(pid) {
        remove_pid_file(trait_path);
        return StopResult::StaleCleanup { pid };
    }
    kill_pid(pid);
    remove_pid_file(trait_path);
    StopResult::Killed { pid }
}

/// Kill a process: SIGTERM, wait up to 3s, then SIGKILL.
#[cfg(not(target_arch = "wasm32"))]
fn kill_pid(pid: u32) {
    tracing::info!("Sending SIGTERM to PID {}", pid);
    unsafe { libc::kill(pid as i32, libc::SIGTERM); }
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if !process_alive(pid) {
            return;
        }
    }
    tracing::warn!("Force-killing PID {}", pid);
    unsafe { libc::kill(pid as i32, libc::SIGKILL); }
    std::thread::sleep(std::time::Duration::from_millis(200));
}

/// Find which trait (if any) has a PID file containing the given PID.
#[cfg(not(target_arch = "wasm32"))]
fn find_trait_by_pid(pid: u32) -> Option<String> {
    let entries = std::fs::read_dir(RUN_DIR).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "pid").unwrap_or(false) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(file_pid) = content.trim().parse::<u32>() {
                    if file_pid == pid {
                        return path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string());
                    }
                }
            }
        }
    }
    None
}

// ── Compiled trait modules + dispatch — auto-discovered by build.rs ──
pub mod compiled {
    include!(concat!(env!("OUT_DIR"), "/compiled_traits.rs"));
}

// ── Static assets — auto-discovered .css/.js files from trait directories ──
pub mod static_assets {
    include!(concat!(env!("OUT_DIR"), "/static_assets.rs"));
}

// ── WASM static assets — binary files from wasm-pack output ──
pub mod wasm_static_assets {
    include!(concat!(env!("OUT_DIR"), "/wasm_static_assets.rs"));
}

// ────────────────── call configuration ──────────────────

/// Configuration for a trait call — carries overrides and param defaults.
#[derive(Debug, Clone, Default)]
pub struct CallConfig {
    /// Interface overrides: { "llm/prompt": "net.openai" }
    pub interface_overrides: HashMap<String, String>,
    /// Trait-level overrides: { "net.copilot_chat": "net.openai" }
    pub trait_overrides: HashMap<String, String>,
    /// Per-override param defaults: { "llm/prompt": { "model": String("gpt-5") } }
    pub load_params: HashMap<String, HashMap<String, TraitValue>>,
}

impl CallConfig {
    /// Create a call config with explicit interface/trait overrides.
    pub fn new(
        interface_overrides: HashMap<String, String>,
        trait_overrides: HashMap<String, String>,
    ) -> Self {
        Self {
            interface_overrides,
            trait_overrides,
            load_params: HashMap::new(),
        }
    }

    /// Merge another CallConfig as a base layer (self takes priority).
    /// Used to layer a trait's persistent [load] under per-call overrides.
    pub fn with_base(&self, base: &CallConfig) -> CallConfig {
        let mut merged = base.clone();
        // Per-call overrides take priority over persistent ones
        for (k, v) in &self.interface_overrides {
            merged.interface_overrides.insert(k.clone(), v.clone());
        }
        for (k, v) in &self.trait_overrides {
            merged.trait_overrides.insert(k.clone(), v.clone());
        }
        for (k, v) in &self.load_params {
            merged.load_params.insert(k.clone(), v.clone());
        }
        merged
    }
}

// ────────────────── errors ──────────────────

#[derive(Error, Debug)]
pub enum RouterError {
    #[error("Trait not found: {0}")]
    NotFound(String),
    #[error("Argument count mismatch: expected {expected}, got {got}")]
    ArgCount { expected: usize, got: usize },
    #[error("Type mismatch for parameter '{name}': expected {expected}, got {got}")]
    TypeMismatch {
        name: String,
        expected: String,
        got: String,
    },
    #[error("Execution error: {0}")]
    ExecError(String),
    #[error("Timeout: trait call exceeded {0}s")]
    Timeout(u64),
    #[error("Handle error: {0}")]
    HandleError(String),
}

/// Reserved handle protocol methods
const HANDLE_METHODS: &[&str] = &["__release__", "__inspect__", "__export__", "__handles__", "__log__", "__stop__"];

// ────────────────── dispatcher ──────────────────

/// The core dispatcher: resolves trait paths, validates args, and executes traits.
pub struct Dispatcher {
    registry: Registry,
    timeout: u64,
}

impl Dispatcher {
    pub fn new(registry: Registry, timeout: u64) -> Self {
        Self { registry, timeout }
    }

    /// Extract `{ load: { ... } }` from the last argument if present.
    /// Values can be strings ("target") or objects ({ impl: "target", param: val }).
    /// Keys containing `/` are interface overrides; keys containing `.` are trait overrides.
    /// Returns cleaned args and merged config.
    fn extract_load_config(
        args: Vec<TraitValue>,
        config: &CallConfig,
    ) -> (Vec<TraitValue>, CallConfig) {
        let mut cfg = config.clone();

        // Check if last arg is a Map with a "load" key
        if let Some(TraitValue::Map(last_map)) = args.last() {
            if let Some(TraitValue::Map(load_map)) = last_map.get("load") {
                for (key, val) in load_map {
                    match val {
                        // Simple form: "llm/prompt": "net.openai"
                        TraitValue::String(target) => {
                            if key.contains('/') {
                                cfg.interface_overrides.insert(key.clone(), target.clone());
                            } else {
                                cfg.trait_overrides.insert(key.clone(), target.clone());
                            }
                        }
                        // Object form: "llm/prompt": { impl: "net.openai", model: "gpt-5" }
                        TraitValue::Map(obj) => {
                            if let Some(TraitValue::String(target)) = obj.get("impl") {
                                if key.contains('/') {
                                    cfg.interface_overrides.insert(key.clone(), target.clone());
                                } else {
                                    cfg.trait_overrides.insert(key.clone(), target.clone());
                                }
                                // Collect remaining keys as param defaults
                                let params: HashMap<String, TraitValue> = obj.iter()
                                    .filter(|(k, _)| k.as_str() != "impl")
                                    .map(|(k, v)| (k.clone(), v.clone()))
                                    .collect();
                                if !params.is_empty() {
                                    cfg.load_params.insert(key.clone(), params);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                // Remove the config object from args
                let mut cleaned = args;
                if let Some(TraitValue::Map(m)) = cleaned.last() {
                    if m.len() == 1 {
                        cleaned.pop();
                    } else {
                        if let Some(TraitValue::Map(m)) = cleaned.last_mut() {
                            m.remove("load");
                        }
                    }
                }
                return (cleaned, cfg);
            }
        }
        (args, cfg)
    }

    /// Apply load_params to args based on the target trait's parameter signature.
    /// Fills in missing optional params or overrides existing ones by name.
    fn apply_load_params(
        &self,
        source_key: &str,
        target_path: &str,
        mut args: Vec<TraitValue>,
        config: &CallConfig,
    ) -> Vec<TraitValue> {
        let params = match config.load_params.get(source_key) {
            Some(p) if !p.is_empty() => p,
            _ => return args,
        };
        let entry = match self.registry.get(target_path) {
            Some(e) => e,
            None => return args,
        };
        let sig_params = &entry.signature.params;

        for (param_name, param_val) in params {
            // Find the positional index for this named param
            if let Some(idx) = sig_params.iter().position(|p| p.name == *param_name) {
                if idx < args.len() {
                    // Override existing arg
                    args[idx] = param_val.clone();
                } else {
                    // Extend args with Null padding up to this position, then set value
                    while args.len() < idx {
                        args.push(TraitValue::Null);
                    }
                    args.push(param_val.clone());
                }
            }
        }
        args
    }

    /// Call a trait by dot-notation path with the given arguments.
    /// Also handles reserved handle protocol methods (__release__, __inspect__, etc.)
    /// and interface dispatch (paths starting with "interface:").
    pub async fn call(
        &self,
        path: &str,
        args: Vec<TraitValue>,
        config: &CallConfig,
    ) -> Result<TraitValue, RouterError> {
        // Extract inline { load: { ... } } from last arg
        let (args, config) = Self::extract_load_config(args, config);

        // Handle reserved protocol methods
        if HANDLE_METHODS.contains(&path) {
            return self.call_handle_method(path, args).await;
        }

        // Merge persistent [load] from the trait's TOML definition (trait's config is base,
        // per-call overrides take priority)
        let config = if let Some(entry) = self.registry.get(path) {
            if let Some(ref base_load) = entry.load {
                config.with_base(base_load)
            } else {
                config
            }
        } else {
            config
        };

        // Handle trait-level override: redirect calls to one trait to another
        if let Some(redirect) = config.trait_overrides.get(path).cloned() {
            tracing::debug!("Trait override: '{}' → '{}'", path, redirect);
            let args = self.apply_load_params(path, &redirect, args, &config);
            return Box::pin(self.call(&redirect, args, &config)).await;
        }

        // Interface resolution: paths containing '/' are interface paths (e.g. www/website).
        // Resolve to the bound implementation trait using the resolution chain:
        // runtime overrides → global bindings → caller's local bindings → auto-discover.
        if path.contains('/') {
            if let Some(impl_path) = self.registry.resolve_interface(
                path,
                &config,
            ) {
                tracing::debug!("Interface resolution: '{}' → '{}'", path, impl_path);
                return Box::pin(self.call(&impl_path, args, &config)).await;
            }
            return Err(RouterError::NotFound(format!(
                "No implementation bound for interface '{}'", path
            )));
        }

        // Kernel binding/implementation resolution: if this dot-path has
        // implementations or bindings, resolve to the best implementation.
        // Skip if the path is already a concrete registered trait (not just a namespace prefix).
        let is_concrete = self.registry.get(path).is_some();
        if !is_concrete {
            if self.registry.get_binding(path).is_some() || self.registry.is_interface(path) {
                if let Some(resolved) = self.registry.resolve_with_bindings(path, &config) {
                    if resolved.path != path {
                        tracing::debug!("Binding/interface resolution: '{}' → '{}'", path, resolved.path);
                        return Box::pin(self.call(&resolved.path, args, &config)).await;
                    }
                }
            }
        }

        // 0. Resolve imports (dependencies) before calling the trait
        self.resolve_imports(path, &mut HashSet::new()).await?;

        // 1. Resolve trait
        let trait_entry = self
            .registry
            .get(path)
            .ok_or_else(|| RouterError::NotFound(path.to_string()))?;

        // 2. Validate argument count
        let required_count = trait_entry
            .signature
            .params
            .iter()
            .filter(|p| !p.optional)
            .count();
        let max_count = trait_entry.signature.params.len();

        if args.len() < required_count || args.len() > max_count {
            return Err(RouterError::ArgCount {
                expected: required_count,
                got: args.len(),
            });
        }

        // 3. Coerce args to match expected types before validation
        let mut args = args;
        for (i, arg) in args.iter_mut().enumerate() {
            if i < trait_entry.signature.params.len() {
                let param = &trait_entry.signature.params[i];
                match &param.param_type {
                    // Coerce non-string primitives → string
                    TraitType::String => {
                        match arg {
                            TraitValue::Int(n) => {
                                *arg = TraitValue::String(n.to_string());
                            }
                            TraitValue::Float(f) => {
                                *arg = TraitValue::String(f.to_string());
                            }
                            TraitValue::Bool(b) => {
                                *arg = TraitValue::String(b.to_string());
                            }
                            TraitValue::List(_) | TraitValue::Map(_) => {
                                if let Ok(json) = serde_json::to_string(&arg) {
                                    *arg = TraitValue::String(json);
                                }
                            }
                            _ => {}
                        }
                    }
                    // Coerce string → list by JSON.parse or comma-splitting
                    TraitType::List(_) => {
                        if let TraitValue::String(s) = arg {
                            if let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(s) {
                                let items: Vec<TraitValue> = parsed
                                    .into_iter()
                                    .map(|v| serde_json::from_value::<TraitValue>(v).unwrap_or(TraitValue::Null))
                                    .collect();
                                *arg = TraitValue::List(items);
                            } else {
                                let items: Vec<TraitValue> = s
                                    .split(',')
                                    .map(|part| TraitValue::String(part.trim().to_string()))
                                    .collect();
                                *arg = TraitValue::List(items);
                            }
                        }
                    }
                    // Coerce string → map by JSON.parse
                    TraitType::Map(_, _) => {
                        if let TraitValue::String(s) = arg {
                            if let Ok(parsed) = serde_json::from_str::<HashMap<String, serde_json::Value>>(s) {
                                let entries: HashMap<String, TraitValue> = parsed
                                    .into_iter()
                                    .map(|(k, v)| (k, serde_json::from_value::<TraitValue>(v).unwrap_or(TraitValue::Null)))
                                    .collect();
                                *arg = TraitValue::Map(entries);
                            }
                        }
                    }
                    // Coerce string → bool ("true"/"false")
                    TraitType::Bool => {
                        if let TraitValue::String(s) = arg {
                            match s.trim().to_lowercase().as_str() {
                                "true" | "1" | "yes" => *arg = TraitValue::Bool(true),
                                "false" | "0" | "no" => *arg = TraitValue::Bool(false),
                                _ => {}
                            }
                        }
                    }
                    // Coerce string → number
                    TraitType::Int => {
                        if let TraitValue::String(s) = arg {
                            if let Ok(n) = s.trim().parse::<i64>() {
                                *arg = TraitValue::Int(n);
                            }
                        }
                    }
                    TraitType::Float => {
                        if let TraitValue::String(s) = arg {
                            if let Ok(n) = s.trim().parse::<f64>() {
                                *arg = TraitValue::Float(n);
                            }
                        }
                    }
                    // Coerce string → handle (e.g. "hdl:py:abc123" from MCP tools)
                    TraitType::Handle => {
                        if let TraitValue::String(s) = arg {
                            if s.starts_with("hdl:") {
                                let mut m = HashMap::new();
                                m.insert("__handle__".to_string(), TraitValue::String(s.clone()));
                                *arg = TraitValue::Map(m);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // 4. Validate argument types (handles pass as 'any' or 'handle')
        for (i, arg) in args.iter().enumerate() {
            if i < trait_entry.signature.params.len() {
                let param = &trait_entry.signature.params[i];
                // Null is fine for optional params
                if matches!(arg, TraitValue::Null) && param.optional {
                    continue;
                }
                if !arg.matches_type(&param.param_type) {
                    return Err(RouterError::TypeMismatch {
                        name: param.name.clone(),
                        expected: format!("{:?}", param.param_type),
                        got: arg.type_name().to_string(),
                    });
                }
            }
        }

        // 5. Dispatch
        if trait_entry.background {
            // Background traits (e.g. kernel.serve) run indefinitely — no timeout.
            // Auto-stop any existing instance via PID file before starting.
            match stop_existing(path) {
                StopResult::Killed { pid } => {
                    tracing::info!("Auto-stopped previous {} (PID {})", path, pid);
                    // Give OS time to release resources (ports, files, etc.)
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
                StopResult::StaleCleanup { pid } => {
                    tracing::info!("Cleaned up stale PID file for {} (PID {} was dead)", path, pid);
                }
                _ => {}
            }
            write_pid_file(path);
            // Register in the in-memory task registry so sys.ps shows it
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            kernel_logic::platform::register_task(
                path, path, "service", now, "background trait",
            );
            self.execute(path, args, true).await
        } else {
            tokio::time::timeout(
                std::time::Duration::from_secs(self.timeout),
                self.execute(path, args, false),
            )
            .await
            .map_err(|_| RouterError::Timeout(self.timeout))?
        }
    }

    /// Start a streaming call — sends chunks to `stream_tx` as they arrive.
    /// Validates args and dispatches, then returns immediately.
    /// The sender is dropped (closing the receiver) when the stream ends.
    pub async fn call_stream(
        &self,
        path: &str,
        args: Vec<TraitValue>,
        stream_tx: mpsc::Sender<TraitValue>,
        config: &CallConfig,
    ) -> Result<(), RouterError> {
        // Extract inline { load: { ... } } from last arg
        let (args, config) = Self::extract_load_config(args, config);

        // Merge persistent [load] from the trait's TOML definition
        let config = if let Some(entry) = self.registry.get(path) {
            if let Some(ref base_load) = entry.load {
                config.with_base(base_load)
            } else {
                config
            }
        } else {
            config
        };

        // Handle trait-level override for streaming
        if let Some(redirect) = config.trait_overrides.get(path).cloned() {
            let args = self.apply_load_params(path, &redirect, args, &config);
            return Box::pin(self.call_stream(&redirect, args, stream_tx, &config)).await;
        }

        // 0. Resolve imports
        self.resolve_imports(path, &mut HashSet::new()).await?;

        // 1. Resolve trait
        let trait_entry = self
            .registry
            .get(path)
            .ok_or_else(|| RouterError::NotFound(path.to_string()))?;

        // 2. Validate argument count
        let required_count = trait_entry
            .signature
            .params
            .iter()
            .filter(|p| !p.optional)
            .count();
        let max_count = trait_entry.signature.params.len();

        if args.len() < required_count || args.len() > max_count {
            return Err(RouterError::ArgCount {
                expected: required_count,
                got: args.len(),
            });
        }

        // 3. Execute — stream chunks for traits with native streaming support
        if path == "llm.prompt.acp" {
            let args_json: Vec<serde_json::Value> = args.iter().map(|a| a.to_json()).collect();
            let tx = stream_tx.clone();
            tokio::task::spawn_blocking(move || {
                compiled::acp::acp_proxy_dispatch_streaming(&args_json, &mut |chunk: &str| {
                    let _ = tx.blocking_send(TraitValue::String(chunk.to_string()));
                });
            }).await.map_err(|e| RouterError::ExecError(e.to_string()))?;
            return Ok(());
        }

        // Fallback: single-shot dispatch, send full result
        let result = self.execute(path, args, false).await?;
        let _ = stream_tx.send(result).await;
        Ok(())
    }

    /// Route a handle protocol method (__release__, __inspect__, __export__, __handles__, __log__, __stop__)
    /// to the correct worker based on the handle's language prefix.
    async fn call_handle_method(
        &self,
        method: &str,
        args: Vec<TraitValue>,
    ) -> Result<TraitValue, RouterError> {
        // __stop__ with a trait path or PID: stop via PID file (cross-process)
        if method == "__stop__" {
            let arg = match args.first() {
                Some(TraitValue::String(s)) => s.clone(),
                Some(TraitValue::Int(n)) => n.to_string(),
                _ => return Err(RouterError::HandleError(
                    "__stop__ requires a trait path (e.g. \"kernel.serve\") or a PID number".into()
                )),
            };

            // If the argument is numeric, treat it as a PID
            if let Ok(pid) = arg.parse::<u32>() {
                if pid == std::process::id() {
                    return Err(RouterError::HandleError("Cannot stop self".into()));
                }
                // Try to find which trait owns this PID
                if let Some(trait_path) = find_trait_by_pid(pid) {
                    let result = stop_existing(&trait_path);
                    return match result {
                        StopResult::Killed { pid } => Ok(TraitValue::String(
                            format!("Stopped {} (PID {})", trait_path, pid)
                        )),
                        StopResult::StaleCleanup { pid } => Ok(TraitValue::String(
                            format!("Cleaned up stale PID file for {} (PID {} was not running)", trait_path, pid)
                        )),
                        _ => Ok(TraitValue::String(format!("Cleaned up {}", trait_path))),
                    };
                }
                // No PID file found — try killing the PID directly
                if process_alive(pid) {
                    kill_pid(pid);
                    return Ok(TraitValue::String(format!("Killed PID {}", pid)));
                }
                return Err(RouterError::HandleError(
                    format!("No process with PID {} found", pid)
                ));
            }

            // It's a trait path
            let trait_path = arg;
            let result = stop_existing(&trait_path);
            return match result {
                StopResult::Killed { pid } => Ok(TraitValue::String(
                    format!("Stopped {} (PID {})", trait_path, pid)
                )),
                StopResult::StaleCleanup { pid } => Ok(TraitValue::String(
                    format!("Cleaned up stale PID file for {} (PID {} was not running)", trait_path, pid)
                )),
                StopResult::NotFound => Err(RouterError::HandleError(
                    format!("No running instance of {} found", trait_path)
                )),
                StopResult::IsSelf => Err(RouterError::HandleError(
                    "Cannot stop self".into()
                )),
            };
        }

        if method == "__handles__" {
            // List all active handles
            return self.handle_list().await;
        }

        // For __release__, __inspect__, __export__: first arg must be a handle
        let handle = args.first().ok_or_else(|| {
            RouterError::HandleError(format!("{} requires a handle argument", method))
        })?;

        // Verify it's actually a handle
        if !handle.is_handle() {
            return Err(RouterError::HandleError(format!(
                "{} requires a valid handle (got {:?})", method, handle
            )));
        }

        self.handle_method(method, args).await
    }

    /// Recursively resolve and call imports (dependencies) for a trait.
    /// Uses a visited set to avoid cycles and duplicate calls.
    async fn resolve_imports(
        &self,
        path: &str,
        visited: &mut HashSet<String>,
    ) -> Result<(), RouterError> {
        if visited.contains(path) {
            return Ok(()); // Already processed (cycle or diamond dep)
        }
        visited.insert(path.to_string());

        let trait_entry = match self.registry.get(path) {
            Some(e) => e,
            None => return Ok(()), // Not found — will fail later in call()
        };

        if trait_entry.imports.is_empty() {
            return Ok(());
        }

        for import_path in &trait_entry.imports {
            // Recursively resolve transitive imports
            Box::pin(self.resolve_imports(import_path, visited)).await?;

            // Call the import with empty args
            if self.registry.get(import_path).is_none() {
                return Err(RouterError::ExecError(format!(
                    "Import '{}' (required by '{}') not found in registry",
                    import_path, path
                )));
            }

            match self.execute(import_path, vec![], false).await {
                Ok(_) => tracing::info!("Import '{}' resolved for '{}'", import_path, path),
                Err(e) => return Err(RouterError::ExecError(format!(
                    "Import '{}' failed: {}", import_path, e
                ))),
            }
        }

        Ok(())
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Resolve a keyed interface binding for a calling trait.
    /// Resolution: caller bindings[key] → requires[key] → interface auto-discover.
    pub fn resolve_keyed(
        &self,
        key: &str,
        caller_path: &str,
    ) -> Option<String> {
        let entry = self.registry.get(caller_path)?;
        // 1. Check caller's bindings for this key
        if let Some(impl_path) = entry.trait_bindings.get(key) {
            return Some(impl_path.clone());
        }
        // 2. Look up interface from requires[key], then resolve
        if let Some(interface_path) = entry.requires.get(key) {
            return self.registry.resolve_interface(interface_path, &CallConfig::default());
        }
        None
    }

    /// Resolve all keyed interface bindings for a calling trait.
    /// Returns a map of key → resolved implementation trait path.
    pub fn resolve_all_keyed(
        &self,
        caller_path: &str,
    ) -> HashMap<String, String> {
        let entry = match self.registry.get(caller_path) {
            Some(e) => e,
            None => return HashMap::new(),
        };
        let mut result = HashMap::new();
        for (key, interface_path) in &entry.requires {
            if let Some(impl_path) = entry.trait_bindings.get(key) {
                result.insert(key.clone(), impl_path.clone());
            } else if let Some(impl_path) = self.registry.resolve_interface(interface_path, &CallConfig::default()) {
                result.insert(key.clone(), impl_path.clone());
            }
        }
        result
    }

    // ────────────────── trait execution ──────────────────

    /// Execute a trait by path — dispatches to compiled Rust modules.
    async fn execute(
        &self,
        path: &str,
        args: Vec<TraitValue>,
        background: bool,
    ) -> Result<TraitValue, RouterError> {
        // Background traits have async entry points (e.g. kernel.serve → start())
        if background {
            if let Some(result) = compiled::dispatch_async(path, &args).await {
                return result.map_err(|e| RouterError::ExecError(e.to_string()));
            }
        }

        // REST traits: dispatch via sys.call using the [http] config from the TOML
        if let Some(entry) = self.registry.get(path) {
            if let Some(ref http) = entry.http {
                return self.execute_rest(path, &entry, &args, http);
            }
        }

        // Sync dispatch through compiled modules
        compiled::dispatch_trait_value(path, &args)
            .ok_or_else(|| RouterError::ExecError(format!("No implementation for {}", path)))
    }

    /// Execute a REST trait by building sys.call args from the [http] config.
    /// Template substitution: {{param_name}} in url/body is replaced with the arg value.
    fn execute_rest(
        &self,
        path: &str,
        entry: &crate::registry::TraitEntry,
        args: &[TraitValue],
        http: &crate::registry::HttpTraitConfig,
    ) -> Result<TraitValue, RouterError> {
        // Build param name → value map from signature + args
        // Start with defaults, then override with actual args
        let mut param_map: std::collections::HashMap<String, String> = http.defaults.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (i, param) in entry.signature.params.iter().enumerate() {
            if let Some(arg) = args.get(i) {
                let val_str = match arg {
                    TraitValue::String(s) => s.clone(),
                    TraitValue::Int(n) => n.to_string(),
                    TraitValue::Float(f) => f.to_string(),
                    TraitValue::Bool(b) => b.to_string(),
                    TraitValue::Null => continue,
                    other => serde_json::to_string(&other.to_json()).unwrap_or_default(),
                };
                param_map.insert(param.name.clone(), val_str);
            }
        }

        // Template substitution helper
        let substitute = |template: &str| -> String {
            let mut result = template.to_string();
            for (name, val) in &param_map {
                result = result.replace(&format!("{{{{{}}}}}", name), val);
            }
            result
        };

        // Build URL with template substitution
        let url = substitute(&http.url);

        // Build body with template substitution
        let body: serde_json::Value = if let Some(ref body_template) = http.body {
            let body_str = substitute(body_template);
            serde_json::from_str(&body_str).unwrap_or(serde_json::Value::String(body_str))
        } else {
            serde_json::Value::Null
        };

        // Auth secret
        let auth_secret = http.auth_secret.as_deref()
            .map(|s| serde_json::Value::String(s.to_string()))
            .unwrap_or(serde_json::Value::Null);

        // Method
        let method = serde_json::Value::String(http.method.clone());

        // Headers
        let headers: serde_json::Value = if http.headers.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::json!(http.headers)
        };

        // Call sys.call with [url, body, auth_secret, method, headers]
        let call_args = vec![
            serde_json::Value::String(url),
            body,
            auth_secret,
            method,
            headers,
        ];

        tracing::debug!("REST dispatch for '{}': calling sys.call", path);
        let result = compiled::dispatch("sys.call", &call_args)
            .ok_or_else(|| RouterError::ExecError("sys.call trait not available".into()))?;

        // Extract response_path if configured (e.g., "body.choices.0.message.content")
        let result = if let Some(ref response_path) = http.response_path {
            extract_json_path(&result, response_path)
        } else {
            result
        };

        Ok(TraitValue::from_json(&result))
    }

    /// Handle __release__, __inspect__ for a specific handle.
    async fn handle_method(
        &self,
        method: &str,
        args: Vec<TraitValue>,
    ) -> Result<TraitValue, RouterError> {
        let handles = crate::globals::HANDLES.get()
            .ok_or_else(|| RouterError::HandleError("Handles not initialized".into()))?
            .clone();

        let handle_id = args.first()
            .and_then(|v| v.handle_id())
            .map(|s| s.to_string())
            .ok_or_else(|| RouterError::HandleError(format!("{} requires a handle argument", method)))?;

        match method {
            "__release__" => {
                let mut h = handles.lock().await;
                let removed = h.remove(&handle_id).is_some();
                Ok(TraitValue::Bool(removed))
            }
            "__inspect__" => {
                let h = handles.lock().await;
                match h.get(&handle_id) {
                    Some(entry) => {
                        let mut map = HashMap::new();
                        map.insert("id".into(), TraitValue::String(handle_id));
                        map.insert("type".into(), TraitValue::String(entry.type_name.clone()));
                        map.insert("summary".into(), TraitValue::String(entry.summary.clone()));
                        map.insert("created".into(), TraitValue::Float(entry.created));
                        map.insert("age_seconds".into(), TraitValue::Float(crate::globals::now_epoch() - entry.created));
                        Ok(TraitValue::Map(map))
                    }
                    None => Err(RouterError::HandleError(format!("Invalid handle: {}", handle_id))),
                }
            }
            _ => Err(RouterError::HandleError(format!("Unknown handle method: {}", method))),
        }
    }

    /// List all active handles.
    async fn handle_list(&self) -> Result<TraitValue, RouterError> {
        let handles = crate::globals::HANDLES.get()
            .ok_or_else(|| RouterError::HandleError("Handles not initialized".into()))?
            .clone();
        let h = handles.lock().await;
        let list: Vec<TraitValue> = h.iter().map(|(hid, entry)| {
            let mut map = HashMap::new();
            map.insert("id".into(), TraitValue::String(hid.clone()));
            map.insert("type".into(), TraitValue::String(entry.type_name.clone()));
            map.insert("summary".into(), TraitValue::String(entry.summary.clone()));
            map.insert("created".into(), TraitValue::Float(entry.created));
            TraitValue::Map(map)
        }).collect();
        Ok(TraitValue::List(list))
    }

    pub async fn shutdown(&self) {}
}

// ── JSON path extraction for REST traits ──

/// Extract a value from nested JSON using a dot-separated path.
/// Supports array indexing: "choices.0.message.content"
fn extract_json_path(value: &serde_json::Value, path: &str) -> serde_json::Value {
    let mut current = value;
    for segment in path.split('.') {
        current = if let Ok(idx) = segment.parse::<usize>() {
            match current.get(idx) {
                Some(v) => v,
                None => return serde_json::Value::Null,
            }
        } else {
            match current.get(segment) {
                Some(v) => v,
                None => return serde_json::Value::Null,
            }
        };
    }
    current.clone()
}

// ── Trait dispatch entry point ──

/// kernel.dispatcher introspection: returns dispatcher status and compiled trait list.
pub fn dispatcher(args: &[serde_json::Value]) -> serde_json::Value {
    let _ = args;
    // List all compiled trait paths by calling dispatch_compiled with a marker
    // We can't enumerate directly, so return structural info
    let has_dylib = crate::dylib_loader::LOADER.get()
        .map(|l| !l.list().is_empty())
        .unwrap_or(false);
    let dylib_count = crate::dylib_loader::LOADER.get()
        .map(|l| l.list().len())
        .unwrap_or(0);
    let dylib_list = crate::dylib_loader::LOADER.get()
        .map(|l| l.list())
        .unwrap_or_default();
    serde_json::json!({
        "dispatch_layers": ["dylib", "compiled"],
        "dylib_loaded": has_dylib,
        "dylib_count": dylib_count,
        "dylib_traits": dylib_list,
        "features": ["arg_validation", "type_coercion", "streaming", "background_traits", "handle_protocol", "pid_files"]
    })
}
