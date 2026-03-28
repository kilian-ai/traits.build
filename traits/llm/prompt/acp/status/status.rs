use serde_json::Value;

/// llm.prompt.acp.status — Check if the ACP proxy is running.
///
/// Args: [] (no arguments)
pub fn acp_status(_args: &[Value]) -> Value {
    super::acp::get_proxy_status()
}
