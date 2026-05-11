use serde_json::Value;

/// sys.pip — broadcast a pip.open notification to connected browser MCP clients.
pub fn pip(args: &[Value]) -> Value {
    let path = args.get(0).and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return serde_json::json!({ "error": "missing path argument" });
    }

    // Build params and broadcast to all connected /mcp sessions.
    let params = serde_json::json!({ "path": path });
    let sent = crate::dispatcher::compiled::serve::broadcast_mcp_notification("pip.open", params);

    serde_json::json!({ "ok": true, "sent": sent })
}
