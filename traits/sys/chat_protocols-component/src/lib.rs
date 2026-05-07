//! sys.chat_protocols — thin delegator to the bound chat/protocol-reader.
//!
//! In the dylib version, this dispatches to `sys.chat_protocols.vscode` (the
//! current default binding). The component version preserves that behaviour
//! by going through the kernel-host dispatch import.

#[allow(warnings, clippy::all)]
mod bindings;

use bindings::traits::kernel_host::dispatch as host;

struct Component;

impl bindings::exports::traits::sys_chat_protocols::chat_protocols::Guest for Component {
    fn chat_protocols(
        workspace_id: String,
        method: Option<String>,
        base_dir: Option<String>,
    ) -> Result<String, String> {
        let mut args = vec![serde_json::Value::String(workspace_id)];
        if let Some(m) = method {
            args.push(serde_json::Value::String(m));
        }
        if let Some(b) = base_dir {
            args.push(serde_json::Value::String(b));
        }
        let args_json = serde_json::to_string(&args).unwrap_or_else(|_| "[]".into());
        match host::call("sys.chat_protocols.vscode", &args_json) {
            Ok(s) => Ok(s),
            Err(e) => Err(format!(
                "{{\"ok\":false,\"error\":\"{}\"}}",
                e.replace('"', "\\\"")
            )),
        }
    }
}

bindings::export!(Component with_types_in bindings);
