use serde_json::{json, Value};

/// sys.voice.status — introspect the current voice session.
///
/// Returns the active configuration and session metadata.
/// Useful for the model to check its own state.
pub fn voice_status(_args: &[Value]) -> Value {
    // Read persisted preferences (same source as sys.voice.config)
    let voice = read_pref("voice").unwrap_or_else(|| "shimmer".into());
    let model = read_pref("model").unwrap_or_else(|| "gpt-realtime-mini-2025-12-15".into());
    let agent = read_pref("agent").unwrap_or_default();

    // Check instruction source via dispatch
    let instruct_source = kernel_logic::platform::dispatch(
        "sys.voice.instruct", &[json!("get")],
    )
    .and_then(|r| r.get("instructions_source").or_else(|| r.get("source")).and_then(|v| v.as_str()).map(|s| s.to_string()))
    .unwrap_or_else(|| "default".into());

    // Count memory notes via dispatch
    let memory_count = kernel_logic::platform::dispatch(
        "sys.voice.memory", &[json!("list")],
    )
    .and_then(|r| r.get("count").and_then(|v| v.as_u64()))
    .unwrap_or(0);

    // Count available tools
    let tool_count = count_voice_tools();

    json!({
        "ok": true,
        "voice": voice,
        "model": model,
        "agent": if agent.is_empty() { "none".to_string() } else { agent },
        "instructions_source": instruct_source,
        "memory_notes": memory_count,
        "tool_count": tool_count,
    })
}

fn read_pref(key: &str) -> Option<String> {
    kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("get"), json!("sys.voice"), json!(key)],
    )
    .and_then(|r| r.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()))
    .filter(|s| !s.is_empty())
}

fn count_voice_tools() -> usize {
    let registry = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return 0,
    };
    // Same exclusion logic as build_tools in voice.rs
    let exclude: &[&str] = &[
        "sys.voice", "sys.mcp", "sys.serve", "sys.cli", "sys.cli.native", "sys.cli.wasm",
        "sys.dylib_loader", "sys.reload", "sys.release", "sys.secrets",
        "kernel.main", "kernel.dispatcher", "kernel.globals", "kernel.registry",
        "kernel.config", "kernel.plugin_api", "kernel.cli",
        "www.admin", "www.admin.deploy", "www.admin.fast_deploy",
        "www.admin.scale", "www.admin.destroy", "www.admin.save_config",
    ];
    registry.all().iter().filter(|entry| {
        !entry.path.starts_with("www.")
            && entry.kind != "library"
            && entry.kind != "interface"
            && !exclude.contains(&entry.path.as_str())
    }).count()
}
