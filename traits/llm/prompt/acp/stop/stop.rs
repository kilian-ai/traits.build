use serde_json::{json, Value};

/// llm.prompt.acp.stop — Stop the ACP proxy.
///
/// Args: [] (no arguments)
pub fn acp_stop(_args: &[Value]) -> Value {
    json!(super::acp::do_stop_proxy())
}
