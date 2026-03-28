use serde_json::{json, Value};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;

pub const ACP_PROXY_PORT: u16 = 9315;
const ACP_PROXY_WS: &str = "ws://localhost:9315/ws";
const PID_FILE: &str = "/tmp/acp_proxy.pid";

/// Agent registry: (name, command, extra_args, api_key_env)
pub const AGENTS: &[(&str, &str, &[&str], &str)] = &[
    ("opencode", "opencode", &["acp"], "OPENAI_API_KEY"),
    ("claude", "claude-code-acp", &[], "ANTHROPIC_API_KEY"),
    ("codex", "codex-acp", &[], "OPENAI_API_KEY"),
    ("copilot", "copilot", &["--acp"], "GITHUB_TOKEN"),
];

// ═══════════════════════════════════════════
// ── Shared helpers (used by sub-traits) ────
// ═══════════════════════════════════════════

/// Check if the ACP proxy port is reachable.
pub fn is_proxy_running() -> bool {
    // Try IPv4 first, then IPv6 — acp-proxy may bind to either
    let addrs: &[std::net::SocketAddr] = &[
        std::net::SocketAddr::from(([127, 0, 0, 1], ACP_PROXY_PORT)),
        std::net::SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], ACP_PROXY_PORT)),
    ];
    addrs.iter().any(|addr| {
        TcpStream::connect_timeout(addr, Duration::from_secs(1)).is_ok()
    })
}

/// Start the ACP proxy for a given agent.
pub fn do_start_proxy(agent: &str) -> Result<String, String> {
    if is_proxy_running() {
        return Ok("ACP proxy already running".into());
    }

    let (_, cmd, extra_args, _) = AGENTS
        .iter()
        .find(|(name, _, _, _)| *name == agent)
        .ok_or_else(|| {
            format!(
                "Unknown agent: {}. Available: opencode, claude, codex, copilot",
                agent
            )
        })?;

    let port_str = ACP_PROXY_PORT.to_string();
    let mut spawn_args: Vec<String> =
        vec!["--no-auth".into(), "--port".into(), port_str, cmd.to_string()];
    for a in *extra_args {
        spawn_args.push(a.to_string());
    }

    let child = Command::new("acp-proxy")
        .args(&spawn_args)
        .env("NO_COLOR", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn acp-proxy: {}. Is it installed?", e))?;

    let pid = child.id();
    let _ = std::fs::write(PID_FILE, pid.to_string());

    // Poll for proxy readiness (up to 15s)
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(500));
        if is_proxy_running() {
            return Ok(format!("ACP proxy started for {} (pid {})", agent, pid));
        }
    }

    // Timeout — kill and report
    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    let _ = std::fs::remove_file(PID_FILE);
    Err("ACP proxy failed to start within 15 seconds".into())
}

/// Stop the ACP proxy.
pub fn do_stop_proxy() -> String {
    if let Ok(pid_str) = std::fs::read_to_string(PID_FILE) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            unsafe { libc::kill(pid, libc::SIGTERM) };
            let _ = std::fs::remove_file(PID_FILE);
            return format!("ACP proxy stopped (pid {})", pid);
        }
    }

    if is_proxy_running() {
        if let Ok(output) = Command::new("lsof")
            .args(["-ti", &format!("tcp:{}", ACP_PROXY_PORT)])
            .output()
        {
            let pids = String::from_utf8_lossy(&output.stdout);
            for line in pids.lines() {
                if let Ok(pid) = line.trim().parse::<i32>() {
                    unsafe { libc::kill(pid, libc::SIGTERM) };
                }
            }
            return "ACP proxy stopped".into();
        }
    }

    "ACP proxy not running".into()
}

/// Get proxy status as JSON.
pub fn get_proxy_status() -> Value {
    let running = is_proxy_running();
    let pid = std::fs::read_to_string(PID_FILE)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok());

    json!({
        "running": running,
        "port": ACP_PROXY_PORT,
        "pid": pid,
    })
}

// ═══════════════════════════════════════════
// ── WebSocket ACP client ───────────────────
// ═══════════════════════════════════════════

/// Send a prompt to the ACP agent via WebSocket and collect the full response.
fn send_prompt(prompt: &str, cwd: &str) -> Result<String, String> {
    use tungstenite::{connect, Message};

    let (mut ws, _) =
        connect(ACP_PROXY_WS).map_err(|e| format!("WebSocket connect failed: {}", e))?;

    // ── 1. Connect to agent ──
    ws.send(Message::Text(json!({"type": "connect"}).to_string()))
        .map_err(|e| format!("Send connect: {}", e))?;

    loop {
        let msg = ws.read().map_err(|e| format!("Read: {}", e))?;
        if let Message::Text(text) = msg {
            let v: Value = serde_json::from_str(&text).unwrap_or_default();
            match v.get("type").and_then(|t| t.as_str()) {
                Some("status") => {
                    if !v
                        .pointer("/payload/connected")
                        .and_then(|c| c.as_bool())
                        .unwrap_or(false)
                    {
                        return Err("Agent not connected".into());
                    }
                    break;
                }
                Some("error") => {
                    let m = v
                        .pointer("/payload/message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown");
                    return Err(format!("Connection error: {}", m));
                }
                _ => continue,
            }
        }
    }

    // ── 2. Create session ──
    ws.send(Message::Text(
        json!({"type": "new_session", "payload": {"cwd": cwd}}).to_string(),
    ))
    .map_err(|e| format!("Send new_session: {}", e))?;

    loop {
        let msg = ws.read().map_err(|e| format!("Read: {}", e))?;
        if let Message::Text(text) = msg {
            let v: Value = serde_json::from_str(&text).unwrap_or_default();
            match v.get("type").and_then(|t| t.as_str()) {
                Some("session_created") => break,
                Some("error") => {
                    let m = v
                        .pointer("/payload/message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown");
                    return Err(format!("Session error: {}", m));
                }
                _ => continue,
            }
        }
    }

    // ── 3. Send prompt ──
    ws.send(Message::Text(
        json!({
            "type": "prompt",
            "payload": {
                "content": [{"type": "text", "text": prompt}]
            }
        })
        .to_string(),
    ))
    .map_err(|e| format!("Send prompt: {}", e))?;

    // ── 4. Collect response chunks ──
    let mut parts: Vec<String> = Vec::new();

    loop {
        let msg = ws.read().map_err(|e| format!("Read: {}", e))?;
        if let Message::Text(text) = msg {
            let v: Value = serde_json::from_str(&text).unwrap_or_default();
            match v.get("type").and_then(|t| t.as_str()) {
                Some("session_update") => {
                    if let Some(update) = v.pointer("/payload/update") {
                        match update.get("sessionUpdate").and_then(|s| s.as_str()) {
                            Some("agent_message_chunk") => {
                                if let Some(t) =
                                    update.pointer("/content/text").and_then(|t| t.as_str())
                                {
                                    parts.push(t.to_string());
                                }
                            }
                            Some("agent_message") => {
                                if let Some(arr) =
                                    update.get("content").and_then(|c| c.as_array())
                                {
                                    for item in arr {
                                        if item.get("type").and_then(|t| t.as_str())
                                            == Some("text")
                                        {
                                            if let Some(t) =
                                                item.get("text").and_then(|t| t.as_str())
                                            {
                                                parts.push(t.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some("prompt_complete") => break,
                Some("error") => {
                    let m = v
                        .pointer("/payload/message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown");
                    return Err(format!("Prompt error: {}", m));
                }
                _ => {}
            }
        }
    }

    let _ = ws.close(None);

    if parts.is_empty() {
        Ok("[No response from agent]".into())
    } else {
        Ok(parts.join(""))
    }
}

// ═══════════════════════════════════════════
// ── Trait entry point ──────────────────────
// ═══════════════════════════════════════════

/// llm.prompt.acp — Route prompts to ACP agents via WebSocket proxy.
///
/// Args: [prompt, agent?, cwd?, auto_approve?]
pub fn acp_proxy_dispatch(args: &[Value]) -> Value {
    let prompt = match args.first().and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p,
        _ => return json!({"ok": false, "error": "prompt is required"}),
    };

    let agent = args
        .get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("opencode");

    let cwd_arg = args
        .get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(".");

    let cwd = std::path::Path::new(cwd_arg)
        .canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".into())
        });

    // Auto-start proxy if not running
    if !is_proxy_running() {
        if let Err(e) = do_start_proxy(agent) {
            return json!({"ok": false, "error": e});
        }
    }

    match send_prompt(prompt, &cwd) {
        Ok(response) => json!(response),
        Err(e) => json!({"ok": false, "error": e}),
    }
}
