use serde_json::{json, Value};

/// sys.info — system status overview or detailed trait info with dispatch location.
///
/// - No args: system status (version, uptime, traits, relay)
/// - Trait path: full trait metadata + where it will be dispatched
pub fn info(args: &[Value]) -> Value {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or("").trim();
    if path.is_empty() {
        return system_status();
    }
    trait_info(path)
}

/// System overview: version, uptime, trait count, relay/gateway status.
fn system_status() -> Value {
    // Version info
    let version_info = kernel_logic::platform::dispatch("sys.version", &[
        Value::String("system".into()),
    ]).unwrap_or(json!({}));

    // Trait count
    let trait_count = kernel_logic::platform::registry_count();

    // Uptime
    let uptime = kernel_logic::platform::dispatch("kernel.globals", &[])
        .unwrap_or(json!({}));

    // Server config (from globals set by sys.serve)
    let bind = crate::globals::SERVER_BIND.get().cloned().unwrap_or_default();
    let port = crate::globals::SERVER_PORT.get().map(|p| p.to_string()).unwrap_or_default();

    // Relay status
    let relay = relay_status();

    // OS info
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    json!({
        "system": {
            "os": os,
            "arch": arch,
            "version": version_info.get("version").cloned().unwrap_or(json!("unknown")),
            "build_version": env!("TRAITS_BUILD_VERSION"),
        },
        "server": {
            "bind": if bind.is_empty() { "not running".into() } else { bind },
            "port": if port.is_empty() { "not running".into() } else { port },
            "uptime": uptime.get("uptime_human").cloned().unwrap_or(json!("n/a")),
            "uptime_seconds": uptime.get("uptime_seconds").cloned().unwrap_or(json!(0)),
        },
        "traits": {
            "total": trait_count,
        },
        "relay": relay,
    })
}

/// Relay connection status.
fn relay_status() -> Value {
    let relay_url = crate::globals::RELAY_URL.get().cloned();
    let relay_code = crate::globals::RELAY_CODE.read()
        .ok()
        .and_then(|guard| guard.clone());
    let relay_connected = crate::globals::RELAY_CONNECTED
        .load(std::sync::atomic::Ordering::Relaxed);

    json!({
        "enabled": relay_url.is_some(),
        "url": relay_url.unwrap_or_default(),
        "code": relay_code.unwrap_or_default(),
        "client_connected": relay_connected,
    })
}

/// Detailed trait info + dispatch location.
fn trait_info(path: &str) -> Value {
    // Get trait metadata from registry
    let detail = kernel_logic::platform::dispatch("sys.registry", &[
        Value::String("info".into()),
        Value::String(path.into()),
    ]).unwrap_or_else(|| json!({"error": "sys.registry unavailable"}));

    // If it's an error or namespace listing (array), return as-is
    if detail.get("error").is_some() || detail.is_array() {
        return detail;
    }

    // Determine dispatch location
    let dispatch = dispatch_location(path, &detail);

    // Merge dispatch info into the detail
    let mut result = detail;
    if let Some(obj) = result.as_object_mut() {
        obj.insert("dispatch".into(), dispatch);
    }
    result
}

/// Determine where a trait will be dispatched from the native server.
fn dispatch_location(path: &str, detail: &Value) -> Value {
    let kind = detail.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let source_str = detail.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let language = detail.get("language").and_then(|v| v.as_str()).unwrap_or("rust");

    // Determine source type from the source path or kind
    let source_type = if source_str.contains("dylib") || source_str.ends_with(".dylib") {
        "dylib"
    } else if kind == "rest" || language == "rest" {
        "rest"
    } else if kind == "library" {
        "library"
    } else {
        "builtin"
    };

    // Check if trait has WASM support by checking the .trait.toml wasm field
    // We can infer this from the registry data
    let wasm_capable = is_wasm_capable(path);

    let location = match source_type {
        "dylib" => "native (dylib, loaded at runtime)",
        "rest" => "remote (REST API call)",
        "library" => "not callable (shared library)",
        _ => "native (compiled into binary)",
    };

    json!({
        "source_type": source_type,
        "location": location,
        "wasm_capable": wasm_capable,
        "browser_dispatch": if wasm_capable { "WASM (local)" } else { "helper → relay → REST" },
    })
}

/// Check if a trait is WASM-capable by looking at its .trait.toml
fn is_wasm_capable(path: &str) -> bool {
    // Read the trait's .trait.toml to check for wasm = true
    let parts: Vec<&str> = path.split('.').collect();
    if parts.len() < 2 { return false; }

    let traits_dir = crate::globals::TRAITS_DIR.get()
        .map(|p| p.clone())
        .unwrap_or_else(|| std::path::PathBuf::from("./traits"));

    // Build the likely .trait.toml path: traits/{ns}/{name}/{name}.trait.toml
    let trait_name = parts.last().unwrap_or(&"");
    let dir_path = parts.iter().fold(traits_dir, |p, seg| p.join(seg));
    let toml_path = dir_path.join(format!("{}.trait.toml", trait_name));

    if let Ok(content) = std::fs::read_to_string(&toml_path) {
        // Simple check — look for "wasm = true" in [implementation] section
        let in_impl = content.find("[implementation]");
        if let Some(pos) = in_impl {
            let section = &content[pos..];
            // Stop at next section header
            let end = section[1..].find('[').map(|p| p + 1).unwrap_or(section.len());
            let impl_section = &section[..end];
            return impl_section.contains("wasm = true");
        }
    }
    false
}
