use serde_json::{json, Value};

/// llm.prompt.acp.start — Start (or switch) the ACP proxy for an agent.
///
/// Args: [agent?]
pub fn acp_start(args: &[Value]) -> Value {
    let agent = args
        .first()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("opencode");

    match super::acp::ensure_proxy_for(agent) {
        Ok(msg) => json!(msg),
        Err(e) => json!({"ok": false, "error": e}),
    }
}
