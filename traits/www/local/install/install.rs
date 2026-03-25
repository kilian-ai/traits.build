use serde_json::Value;

pub fn install(_args: &[Value]) -> Value {
    Value::String(include_str!("install.sh").to_string())
}
