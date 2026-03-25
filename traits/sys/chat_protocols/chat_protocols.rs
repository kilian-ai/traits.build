use serde_json::{json, Value};

pub fn chat_protocols(args: &[Value]) -> Value {
    crate::dispatcher::compiled::dispatch("sys.chat_protocols.vscode", args).unwrap_or_else(|| {
        json!({
            "ok": false,
            "error": "No chat protocol reader is available"
        })
    })
}