use serde_json::Value;

/// sys.reload — reload the trait registry from disk.
pub fn reload(_args: &[Value]) -> Value {
    let registry = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return serde_json::json!({"error": "Registry not initialized"}),
    };
    let traits_dir = match crate::globals::TRAITS_DIR.get() {
        Some(p) => p,
        None => return serde_json::json!({"error": "No traits directory configured"}),
    };

    match registry.load_from_dir(traits_dir) {
        Ok(count) => serde_json::json!({ "ok": true, "count": count }),
        Err(e) => serde_json::json!({"error": format!("Reload failed: {}", e)}),
    }
}
