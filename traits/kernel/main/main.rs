include!(concat!(env!("OUT_DIR"), "/kernel_modules.rs"));

use config::Config;
use registry::Registry;
use dispatcher::{CallConfig, Dispatcher};
use std::path::Path;
use tracing::info;

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
    dispatcher::compiled::cli::run().await
}
