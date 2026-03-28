use serde_json::{json, Value};

/// llm.prompt.acp.list — List available models on the running ACP agent.
///
/// Args: [agent?, cwd?]
pub fn acp_list(args: &[Value]) -> Value {
    let agent = args
        .first()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("opencode");

    let cwd_arg = args
        .get(1)
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

    // Ensure proxy is running for the requested agent
    if let Err(e) = super::acp::ensure_proxy_for(agent) {
        return json!({"ok": false, "error": e});
    }

    match super::acp::list_models(&cwd) {
        Ok(models) => models,
        Err(e) => json!({"ok": false, "error": e}),
    }
}
