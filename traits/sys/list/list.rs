use serde_json::Value;

/// sys.list — delegates to sys.registry "list" action.
pub fn list(args: &[Value]) -> Value {
    let namespace = args.first().and_then(|v| v.as_str()).unwrap_or("");
    kernel_logic::platform::dispatch("sys.registry", &[
        Value::String("list".into()),
        Value::String(namespace.into()),
    ]).unwrap_or_else(|| serde_json::json!({"error": "sys.registry unavailable"}))
}
