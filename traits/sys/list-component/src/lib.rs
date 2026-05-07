//! sys.list — Component Model implementation.
//! Delegates to sys.registry via the kernel-host dispatch import.

#[allow(warnings, clippy::all)]
mod bindings;

use bindings::traits::kernel_host::dispatch as host;

struct Component;

impl bindings::exports::traits::sys_list::list::Guest for Component {
    fn list(namespace: Option<String>) -> Result<Vec<String>, String> {
        let ns = namespace.unwrap_or_default();
        let args = serde_json::json!(["list", ns]).to_string();
        let raw = host::call("sys.registry", &args)?;
        // sys.registry returns a JSON value; for "list" it should be an array of strings.
        let parsed: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| format!("parse: {}", e))?;
        match parsed {
            serde_json::Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| match v {
                    serde_json::Value::String(s) => Some(s),
                    other => Some(other.to_string()),
                })
                .collect()),
            other => Err(format!("expected array, got: {}", other)),
        }
    }
}

bindings::export!(Component with_types_in bindings);
