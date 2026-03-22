use serde_json::{json, Value};

/// Save deploy config (fly_app, fly_region) to traits.toml.
/// Reads the existing file, updates or inserts the [deploy] section, writes back.
pub fn save_config(args: &[Value]) -> Value {
    let fly_app = match args.first().and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return json!({"ok": false, "error": "fly_app is required"}),
    };
    let fly_region = match args.get(1).and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return json!({"ok": false, "error": "fly_region is required"}),
    };

    let config_path = "traits.toml";
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "error": format!("Cannot read {}: {}", config_path, e)}),
    };

    // Build new [deploy] section
    let new_deploy = format!(
        "[deploy]\nfly_app = \"{}\"\nfly_region = \"{}\"",
        fly_app.replace('\\', "\\\\").replace('"', "\\\""),
        fly_region.replace('\\', "\\\\").replace('"', "\\\""),
    );

    // Replace existing [deploy] section or append it
    let new_content = if let Some(start) = content.find("\n[deploy]") {
        // Find where the [deploy] section ends (next section or EOF)
        let deploy_start = start + 1; // skip the leading newline
        let rest = &content[deploy_start..];
        let section_end = rest[1..] // skip the '[' of [deploy]
            .find("\n[")
            .map(|i| deploy_start + 1 + i + 1) // +1 for the newline
            .unwrap_or(content.len());
        format!("{}{}\n{}", &content[..deploy_start], new_deploy, &content[section_end..])
    } else if content.starts_with("[deploy]") {
        // [deploy] is the very first section
        let section_end = content[1..]
            .find("\n[")
            .map(|i| i + 1 + 1)
            .unwrap_or(content.len());
        format!("{}\n{}", new_deploy, &content[section_end..])
    } else {
        // No [deploy] section exists, append it
        format!("{}\n\n{}\n", content.trim_end(), new_deploy)
    };

    if let Err(e) = std::fs::write(config_path, &new_content) {
        return json!({"ok": false, "error": format!("Cannot write {}: {}", config_path, e)});
    }

    json!({
        "ok": true,
        "fly_app": fly_app,
        "fly_region": fly_region,
        "note": "Saved to traits.toml. Changes take effect after server restart."
    })
}
