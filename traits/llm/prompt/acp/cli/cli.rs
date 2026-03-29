use serde_json::{json, Value};

/// llm.prompt.acp.cli — Interactive chat CLI for ACP agents.
///
/// When called directly, returns usage instructions.
/// The actual interactive experience is via `chat [agent] [model]` in the CLI session.
///
/// Args: [agent?, model?]
pub fn acp_cli(args: &[Value]) -> Value {
    let agent = args
        .first()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("opencode");

    let model = args
        .get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    json!({
        "usage": format!("traits chat {} {}", agent, model).trim().to_string(),
        "description": "Interactive chat CLI for ACP agents (Claude, Codex, Copilot, OpenCode)",
        "agent": agent,
        "model": if model.is_empty() { "(agent default)" } else { model },
        "commands": [
            "/agent [name]  — show or switch agent",
            "/model [id]    — show or switch model",
            "/models        — list available models",
            "/status        — show session info",
            "/history       — show conversation history",
            "/clear         — clear terminal",
            "/quit          — exit chat mode",
        ],
        "hint": "Run `traits chat` in the CLI or WASM terminal to start an interactive session"
    })
}
