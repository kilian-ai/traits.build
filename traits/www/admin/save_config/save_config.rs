use serde_json::{json, Value};

/// Persistent config path — Fly.io mounts /data as a persistent volume.
/// Falls back to local traits.toml for non-Fly environments.
fn deploy_config_path() -> &'static str {
    if std::path::Path::new("/data").is_dir() {
        "/data/deploy.toml"
    } else {
        "traits.toml"
    }
}

/// Save deploy config (fly_app, fly_region).
/// On Fly.io: writes to /data/deploy.toml (persistent volume).
/// Locally: updates traits.toml in-place.
pub fn save_config(args: &[Value]) -> Value {
    let fly_app = match args.first().and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return json!({"ok": false, "error": "fly_app is required"}),
    };
    let fly_region = match args.get(1).and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return json!({"ok": false, "error": "fly_region is required"}),
    };

    let config_path = deploy_config_path();

    // If writing to persistent volume, just write the deploy section directly
    if config_path == "/data/deploy.toml" {
        let content = format!(
            "[deploy]\nfly_app = \"{}\"\nfly_region = \"{}\"\n",
            fly_app.replace('\\', "\\\\").replace('"', "\\\""),
            fly_region.replace('\\', "\\\\").replace('"', "\\\""),
        );
        if let Err(e) = std::fs::write(config_path, &content) {
            return json!({"ok": false, "error": format!("Cannot write {}: {}", config_path, e)});
        }
        return json!({
            "ok": true,
            "fly_app": fly_app,
            "fly_region": fly_region,
            "path": config_path,
            "note": "Saved to persistent volume. Changes take effect after server restart."
        });
    }

    // Local mode: update traits.toml in-place
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "error": format!("Cannot read {}: {}", config_path, e)}),
    };

    let new_deploy = format!(
        "[deploy]\nfly_app = \"{}\"\nfly_region = \"{}\"",
        fly_app.replace('\\', "\\\\").replace('"', "\\\""),
        fly_region.replace('\\', "\\\\").replace('"', "\\\""),
    );

    let new_content = if let Some(start) = content.find("\n[deploy]") {
        let deploy_start = start + 1;
        let rest = &content[deploy_start..];
        let section_end = rest[1..]
            .find("\n[")
            .map(|i| deploy_start + 1 + i + 1)
            .unwrap_or(content.len());
        format!("{}{}\n{}", &content[..deploy_start], new_deploy, &content[section_end..])
    } else if content.starts_with("[deploy]") {
        let section_end = content[1..]
            .find("\n[")
            .map(|i| i + 1 + 1)
            .unwrap_or(content.len());
        format!("{}\n{}", new_deploy, &content[section_end..])
    } else {
        format!("{}\n\n{}\n", content.trim_end(), new_deploy)
    };

    if let Err(e) = std::fs::write(config_path, &new_content) {
        return json!({"ok": false, "error": format!("Cannot write {}: {}", config_path, e)});
    }

    json!({
        "ok": true,
        "fly_app": fly_app,
        "fly_region": fly_region,
        "path": config_path,
        "note": "Saved to traits.toml. Changes take effect after server restart."
    })
}
