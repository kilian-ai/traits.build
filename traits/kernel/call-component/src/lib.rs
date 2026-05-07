//! kernel.call — Component Model implementation.
//!
//! Forwards a trait call to the kernel dispatcher via the kernel-host import.
//! Supports both dot and underscore notation (LLMs sometimes use the latter).

#[allow(warnings, clippy::all)]
mod bindings;

use bindings::traits::kernel_host::dispatch as host;

struct Component;

impl bindings::exports::traits::kernel_call::call::Guest for Component {
    fn call(trait_path: String, args: Option<Vec<String>>) -> Result<String, String> {
        let normalized = if trait_path.contains('_') && !trait_path.contains('.') {
            trait_path.replace('_', ".")
        } else {
            trait_path
        };
        if normalized.is_empty() {
            return Err("trait_path is required".into());
        }

        // Args come in as a list of opaque strings; the host expects a JSON
        // array. Try to parse each as JSON, falling back to string.
        let parsed_args: Vec<serde_json::Value> = args
            .unwrap_or_default()
            .into_iter()
            .map(|s| match serde_json::from_str(&s) {
                Ok(v) => v,
                Err(_) => serde_json::Value::String(s),
            })
            .collect();
        let args_json = serde_json::to_string(&parsed_args).unwrap_or_else(|_| "[]".into());
        host::call(&normalized, &args_json)
    }
}

bindings::export!(Component with_types_in bindings);
