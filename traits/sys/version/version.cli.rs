use serde_json::Value;

pub fn format_cli(result: &Value) -> String {
    let obj = match result.as_object() {
        Some(o) => o,
        None => return format!("{}\n", result),
    };
    // System mode: "traits 260320 (11 traits)"
    if obj.contains_key("traits") {
        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("traits");
        let ver = obj.get("version").and_then(|v| v.as_str()).unwrap_or("?");
        let count = obj.get("traits").and_then(|v| v.as_u64()).unwrap_or(0);
        return format!("{} {} ({} traits)\n", name, ver, count);
    }
    // Date/hhmmss mode: just print the version string
    let ver = obj.get("version").and_then(|v| v.as_str()).unwrap_or("?");
    format!("{}\n", ver)
}
