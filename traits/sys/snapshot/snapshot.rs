use serde_json::Value;
use std::fs;

use super::version::{yymmdd_now, hhmmss_now};

pub fn snapshot(args: &[Value]) -> Value {
    let trait_path = match args.first().and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => return serde_json::json!({"ok": false, "error": "trait_path is required"}),
    };

    let registry = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return serde_json::json!({"ok": false, "error": "Registry not initialized"}),
    };

    let entry = match registry.get(trait_path) {
        Some(e) => e,
        None => return serde_json::json!({"ok": false, "error": format!("Trait not found: {}", trait_path)}),
    };

    let toml_path = &entry.toml_path;
    let toml_str = match fs::read_to_string(toml_path) {
        Ok(s) => s,
        Err(e) => return serde_json::json!({"ok": false, "error": format!("Cannot read {}: {}", toml_path.display(), e)}),
    };

    // Extract current version
    let old_version = extract_version(&toml_str).unwrap_or_else(|| "v000000".to_string());

    // Decide new version
    let today = yymmdd_now();
    let new_version = if old_version.starts_with(&today) {
        format!("{}.{}", today, hhmmss_now())
    } else {
        today.clone()
    };

    // Replace version in TOML
    let updated = set_version(&toml_str, &new_version);
    if let Err(e) = fs::write(toml_path, &updated) {
        return serde_json::json!({"ok": false, "error": format!("Cannot write {}: {}", toml_path.display(), e)});
    }

    serde_json::json!({
        "ok": true,
        "trait_path": trait_path,
        "old_version": old_version,
        "new_version": new_version,
        "toml_path": toml_path.to_string_lossy(),
    })
}

fn extract_version(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version") {
            if let Some(val) = trimmed.split('=').nth(1) {
                let v = val.trim().trim_matches('"').trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn set_version(toml: &str, new_version: &str) -> String {
    let mut result = String::with_capacity(toml.len());
    let mut replaced = false;
    for line in toml.lines() {
        if !replaced && line.trim().starts_with("version") && line.contains('=') {
            // Preserve indentation
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            result.push_str(&format!("{}version = \"{}\"\n", indent, new_version));
            replaced = true;
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    // Trim trailing extra newline if original didn't end with one
    if !toml.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    result
}
