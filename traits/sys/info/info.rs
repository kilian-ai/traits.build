use serde_json::Value;

/// sys.info — delegates to sys.registry "info" action.
pub fn info(args: &[Value]) -> Value {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
    kernel_logic::platform::dispatch("sys.registry", &[
        Value::String("info".into()),
        Value::String(path.into()),
    ]).unwrap_or_else(|| serde_json::json!({"error": "sys.registry unavailable"}))
}
