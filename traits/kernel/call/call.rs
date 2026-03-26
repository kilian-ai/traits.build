use serde_json::Value;

/// Trait entry point: call(trait_path, args)
///
/// Dispatches to another trait by dot-notation path.
pub fn call(args: &[Value]) -> Value {
    let trait_path = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let call_args = args
        .get(1)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if trait_path.is_empty() {
        return serde_json::json!({ "error": "trait_path is required" });
    }

    let result = kernel_logic::platform::dispatch(trait_path, &call_args);

    match result {
        Some(v) => v,
        None => serde_json::json!({
            "error": format!("Trait '{}' not found", trait_path)
        }),
    }
}
