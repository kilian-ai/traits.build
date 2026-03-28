use serde_json::{json, Value};

/// llm.prompt.acp.start — Start the ACP proxy for a specified agent.
///
/// Args: [agent?]  (default: "opencode")
pub fn acp_start(args: &[Value]) -> Value {
    let agent = args
        .first()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("opencode");

    match super::acp::do_start_proxy(agent) {
        Ok(msg) => json!(msg),
        Err(e) => json!({"ok": false, "error": e}),
    }
}
