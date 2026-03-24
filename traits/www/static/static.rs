use serde_json::Value;

pub fn static_page(_args: &[Value]) -> Value {
    Value::String(include_str!("index.html").to_string())
}
