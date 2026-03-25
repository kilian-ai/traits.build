use serde_json::Value;

pub fn helper(_args: &[Value]) -> Value {
    Value::String(include_str!("helper.sh").to_string())
}
