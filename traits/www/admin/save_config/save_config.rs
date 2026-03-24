use serde_json::{json, Value};

/// Save deploy config (fly_app, fly_region) via generic trait config system.
/// Written to persistent override file (trait_config.toml).
pub fn save_config(args: &[Value]) -> Value {
    let fly_app = match args.first().and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return json!({"ok": false, "error": "fly_app is required"}),
    };
    let fly_region = match args.get(1).and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return json!({"ok": false, "error": "fly_region is required"}),
    };

    if let Err(e) = crate::config::write_persistent_config("www.admin", "fly_app", fly_app) {
        return json!({"ok": false, "error": e});
    }
    if let Err(e) = crate::config::write_persistent_config("www.admin", "fly_region", fly_region) {
        return json!({"ok": false, "error": e});
    }

    json!({
        "ok": true,
        "fly_app": fly_app,
        "fly_region": fly_region,
        "path": crate::config::persistent_config_path(),
        "note": "Saved. Changes take effect on next config read."
    })
}
