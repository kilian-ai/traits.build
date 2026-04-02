include!(concat!(env!("OUT_DIR"), "/kernel_modules.rs"));

use config::Config;
use registry::Registry;
use dispatcher::{CallConfig, Dispatcher};
use std::path::Path;
use tracing::info;

// ────────────────── native VFS backend ──────────────────

/// Build a `LayeredVfs` seeded from the real filesystem.
///
/// Walks `TRAITS_DIR` and mounts every `.trait.toml` and `.features.json`
/// with a path of the form `traits/{ns}/{name}/{file}` — matching exactly the
/// structure the WASM kernel embeds at compile time so `cat` / `ls` paths are
/// identical across both targets.
///
/// Called via `Platform::make_vfs` each time a `CliSession` is created.
fn make_native_vfs() -> Box<dyn kernel_logic::vfs::Vfs> {
    let mut vfs = kernel_logic::vfs::LayeredVfs::new();
    if let Some(traits_dir) = crate::globals::TRAITS_DIR.get() {
        seed_dir(&mut vfs, traits_dir, traits_dir);
    }
    Box::new(vfs)
}

// ────────────────── native persistent VFS ──────────────────

/// Data directory for user-written VFS files.
fn native_vfs_data_dir() -> std::path::PathBuf {
    if std::path::Path::new("/data").exists() {
        std::path::PathBuf::from("/data/vfs")
    } else {
        std::path::PathBuf::from("data/vfs")
    }
}

/// Read from the persistent VFS: user-written files first, then project root.
fn native_vfs_read(path: &str) -> Option<String> {
    let normalized = path.trim_start_matches('/');
    // User-written files (data/vfs/)
    let user_path = native_vfs_data_dir().join(normalized);
    if let Ok(content) = std::fs::read_to_string(&user_path) {
        return Some(content);
    }
    // Project files (relative to cwd)
    std::fs::read_to_string(normalized).ok()
}

/// Write a file to the persistent VFS data directory.
fn native_vfs_write(path: &str, content: &str) {
    let normalized = path.trim_start_matches('/');
    let full = native_vfs_data_dir().join(normalized);
    if let Some(parent) = full.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&full, content);
}

/// List user-written files in the VFS data directory.
fn native_vfs_list() -> Vec<String> {
    let dir = native_vfs_data_dir();
    let mut files = Vec::new();
    if dir.exists() {
        walk_vfs_dir(&dir, &dir, &mut files);
    }
    files.sort();
    files
}

/// Delete a file from the VFS data directory.
fn native_vfs_delete(path: &str) -> bool {
    let normalized = path.trim_start_matches('/');
    let full = native_vfs_data_dir().join(normalized);
    std::fs::remove_file(&full).is_ok()
}

/// Recursively list files under a directory, producing VFS-relative paths.
fn walk_vfs_dir(dir: &std::path::Path, root: &std::path::Path, files: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_vfs_dir(&path, root, files);
            } else if let Ok(rel) = path.strip_prefix(root) {
                files.push(rel.to_string_lossy().to_string());
            }
        }
    }
}

// ────────────────── native process status ──────────────────

/// Scan `.run/*.pid` files and merge with in-memory task registry.
///
/// Called via `Platform::background_tasks` — provides `sys.ps` with
/// native OS process data (alive check, uptime, RSS memory) plus
/// any in-process tasks registered via `platform::register_task()`.
fn native_background_tasks() -> serde_json::Value {
    let mut processes = Vec::new();

    // ── 1. PID-file scan (background = true traits spawned by dispatcher) ──
    let run_dir = std::path::Path::new(".run");
    if run_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(run_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let fname = match path.file_name().and_then(|f| f.to_str()) {
                    Some(f) if f.ends_with(".pid") => f.to_string(),
                    _ => continue,
                };
                let trait_path = fname.trim_end_matches(".pid").to_string();

                let pid_str = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let pid: u32 = match pid_str.trim().parse() {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let alive = unsafe { libc::kill(pid as i32, 0) == 0 };

                let uptime_secs = path.metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|mtime| std::time::SystemTime::now().duration_since(mtime).ok())
                    .map(|d| d.as_secs_f64());

                let memory_mb = native_rss_mb(pid);

                let mut proc_info = serde_json::json!({
                    "trait": trait_path,
                    "pid": pid,
                    "alive": alive,
                    "source": "pid_file",
                });

                if let Some(up) = uptime_secs {
                    proc_info["uptime"] = serde_json::json!(format_uptime(up));
                    proc_info["uptime_secs"] = serde_json::json!(up.round() as u64);
                }
                if let Some(mb) = memory_mb {
                    proc_info["memory_mb"] = serde_json::json!((mb * 100.0).round() / 100.0);
                }
                proc_info["pid_file"] = serde_json::json!(path.to_string_lossy());

                processes.push(proc_info);
            }
        }
    }

    // ── 2. In-memory task registry (services, workers, tokio tasks) ──
    let registry_tasks = kernel_logic::platform::list_tasks();
    for task in registry_tasks {
        let mut proc_info = task.clone();
        proc_info["source"] = serde_json::json!("registry");
        // Compute uptime from started timestamp (epoch seconds)
        if let Some(started) = task["started"].as_f64() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let elapsed = now - started;
            if elapsed > 0.0 {
                proc_info["uptime"] = serde_json::json!(format_uptime(elapsed));
                proc_info["uptime_secs"] = serde_json::json!(elapsed.round() as u64);
            }
        }
        processes.push(proc_info);
    }

    processes.sort_by(|a, b| {
        let ta = a.get("trait").or_else(|| a.get("name")).and_then(|v| v.as_str()).unwrap_or("");
        let tb = b.get("trait").or_else(|| b.get("name")).and_then(|v| v.as_str()).unwrap_or("");
        ta.cmp(tb)
    });

    serde_json::json!({
        "ok": true,
        "count": processes.len(),
        "processes": processes,
    })
}

fn format_uptime(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

/// Get resident set size in MB for a process.
fn native_rss_mb(pid: u32) -> Option<f64> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let rss_kb: f64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;
        Some(rss_kb / 1024.0)
    }
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
                return Some(kb / 1024.0);
            }
        }
        None
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

fn seed_dir(
    vfs: &mut kernel_logic::vfs::LayeredVfs,
    root: &std::path::Path,
    dir: &std::path::Path,
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            seed_dir(vfs, root, &path);
        } else {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".trait.toml") || name.ends_with(".features.json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(rel) = path.strip_prefix(root) {
                        // Prepend "traits/" so the path matches the WASM VFS convention:
                        // traits/sys/checksum/checksum.trait.toml
                        let key = format!("traits/{}", rel.display());
                        vfs.seed(&key, content);
                    }
                }
            }
        }
    }
}

// ────────────────── system bootstrap ──────────────────

/// Bootstrap the trait runtime: registry + dylibs + workers + router.
/// Shared by both CLI dispatch and HTTP server startup.
///
/// All kernel subsystem implementations are resolved through the interface system:
/// kernel.main [requires] config, registry, dispatcher, globals, dylib_loader.
pub fn bootstrap(config: &Config) -> Result<Dispatcher, Box<dyn std::error::Error>> {
    let registry = Registry::new();
    let traits_dir = Path::new(&config.traits.traits_dir);
    let count = registry.load_from_dir(traits_dir)?;
    info!("Loaded {} trait definitions", count);

    // Resolve all required interfaces through the interface system.
    let cc = CallConfig::default();
    for iface in &["kernel/config", "kernel/registry", "kernel/dispatcher", "kernel/globals", "sys/dylib_loader"] {
        let resolved = registry
            .resolve_interface(iface, &cc)
            .unwrap_or_else(|| format!("{}.{}", iface.split('/').next().unwrap_or(""), iface.split('/').last().unwrap_or("")));
        info!("{} → {}", iface, resolved);
    }

    // Initialize globals so trait implementations can access registry/config
    globals::init(registry.clone(), traits_dir.to_path_buf(), config.clone());

    // Initialize platform abstraction layer (dispatch, registry, config, secrets)
    kernel_logic::platform::init(kernel_logic::platform::Platform {
        dispatch: |path, args| crate::dispatcher::compiled::dispatch(path, args),
        registry_all: || {
            match crate::globals::REGISTRY.get() {
                Some(reg) => {
                    let mut traits = reg.all();
                    traits.sort_by(|a, b| a.path.cmp(&b.path));
                    traits.iter().map(|t| t.to_summary_json()).collect()
                }
                None => vec![],
            }
        },
        registry_count: || crate::globals::REGISTRY.get().map(|r| r.len()).unwrap_or(0),
        registry_detail: |path| crate::globals::REGISTRY.get()?.get(path).map(|t| t.to_json()),
        config_get: crate::config::trait_config_or,
        secret_get: |key| {
            let ctx = crate::dispatcher::compiled::secrets::SecretContext::resolve(&[key]);
            ctx.get(key).map(|v| v.to_string())
        },
        make_vfs: make_native_vfs,
        background_tasks: native_background_tasks,
        vfs_read: native_vfs_read,
        vfs_write: native_vfs_write,
        vfs_list: native_vfs_list,
        vfs_delete: native_vfs_delete,
    });

    // Load trait dylibs from the entire traits directory (recursive)
    let dylib_loader = std::sync::Arc::new(dylib_loader::DylibLoader::new(vec![traits_dir.to_path_buf()]));
    let dylib_count = dylib_loader.load_all();
    dylib_loader::set_global_loader(dylib_loader.clone());
    if dylib_count > 0 {
        info!("Loaded {} trait dylibs: {:?}", dylib_count, dylib_loader.list());
    }

    let dispatcher = Dispatcher::new(registry, config.traits.timeout);
    Ok(dispatcher)
}

/// Check if a trait exists by probing the traits directory for its TOML definition.
/// Lightweight — does not bootstrap the runtime.
pub fn trait_exists(config: &Config, trait_path: &str) -> bool {
    let traits_dir = Path::new(&config.traits.traits_dir);
    // Convert dot path to directory: "kernel.serve" → "kernel/serve/serve.trait.toml"
    let parts: Vec<&str> = trait_path.split('.').collect();
    if parts.len() != 2 {
        return false;
    }
    let toml_name = format!("{}.trait.toml", parts[1]);
    let probe = traits_dir.join(parts[0]).join(parts[1]).join(&toml_name);
    probe.exists()
}

// ── Trait dispatch entry point ──

/// kernel.main introspection: returns binary metadata and compiled kernel module list.
pub fn main_info(args: &[serde_json::Value]) -> serde_json::Value {
    let _ = args;
    let uptime = globals::uptime_secs();
    let cc = dispatcher::CallConfig::default();
    let iface_keys = [
        "kernel/config", "kernel/registry", "kernel/dispatcher",
        "kernel/globals", "sys/dylib_loader",
    ];
    let interfaces: serde_json::Map<String, serde_json::Value> = iface_keys.iter().map(|k| {
        let resolved = globals::REGISTRY
            .get()
            .and_then(|r| r.resolve_interface(k, &cc))
            .unwrap_or_else(|| format!("{}.{}", k.split('/').next().unwrap_or(""), k.split('/').last().unwrap_or("")));
        (k.to_string(), serde_json::Value::String(resolved))
    }).collect();
    serde_json::json!({
        "binary": env!("CARGO_PKG_NAME"),
        "version": env!("TRAITS_BUILD_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "bootstrapped": globals::is_initialized(),
        "uptime_seconds": uptime,
        "uptime_human": globals::format_uptime(uptime),
        "compiled_modules": dispatcher::compiled::list_compiled(),
        "interfaces": interfaces,
    })
}

// ────────────────── entry point ──────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dispatcher::compiled::sys_cli::run().await
}
