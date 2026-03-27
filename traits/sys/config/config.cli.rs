use serde_json::Value;

pub fn format_cli(result: &Value) -> String {
    let obj = match result.as_object() {
        Some(o) => o,
        None => return format!("{}\n", result),
    };

    if let Some(err) = obj.get("error") {
        return format!("Error: {}\n", err.as_str().unwrap_or("unknown"));
    }

    let action = obj.get("action").and_then(|v| v.as_str()).unwrap_or("");

    match action {
        "set" => {
            let t = obj.get("trait").and_then(|v| v.as_str()).unwrap_or("?");
            let k = obj.get("key").and_then(|v| v.as_str()).unwrap_or("?");
            let v = obj.get("value").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Set {}.{} = \"{}\"\n", t, k, v)
        }
        "get" => {
            let t = obj.get("trait").and_then(|v| v.as_str()).unwrap_or("?");
            let k = obj.get("key").and_then(|v| v.as_str()).unwrap_or("?");
            let source = obj.get("source").and_then(|v| v.as_str()).unwrap_or("none");
            let value = obj.get("value").and_then(|v| v.as_str());
            match value {
                Some(v) => format!("{}.{} = \"{}\"  (from {})\n", t, k, v, source),
                None => format!("{}.{} = (not set)\n", t, k),
            }
        }
        "delete" => {
            let t = obj.get("trait").and_then(|v| v.as_str()).unwrap_or("?");
            let k = obj.get("key").and_then(|v| v.as_str()).unwrap_or("?");
            let deleted = obj.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false);
            if deleted {
                format!("Deleted {}.{} from persistent config\n", t, k)
            } else {
                format!("{}.{} not found in persistent config\n", t, k)
            }
        }
        "list" => {
            // Listing a specific trait's config
            if let Some(configs) = obj.get("config").and_then(|v| v.as_array()) {
                let t = obj.get("trait").and_then(|v| v.as_str()).unwrap_or("?");
                if configs.is_empty() {
                    return format!("No config found for {}\n", t);
                }
                let mut out = String::new();
                out.push_str(&format!("Config for {}:\n", t));
                out.push_str(&format!("{}\n", "─".repeat(60)));
                for entry in configs {
                    let k = entry.get("key").and_then(|v| v.as_str()).unwrap_or("?");
                    let v = entry.get("value").and_then(|v| v.as_str());
                    let src = entry.get("source").and_then(|v| v.as_str()).unwrap_or("?");
                    let display = v.unwrap_or("(not set)");
                    out.push_str(&format!("  {:20} = {:30} ({})\n", k, format!("\"{}\"", display), src));
                }
                out
            }
            // Listing all traits with config
            else if let Some(traits) = obj.get("traits").and_then(|v| v.as_array()) {
                if traits.is_empty() {
                    return "No traits with config found.\n".to_string();
                }
                let mut out = String::new();
                out.push_str(&format!("{:<30} {}\n", "TRAIT", "KEYS"));
                out.push_str(&format!("{}\n", "─".repeat(60)));
                for t in traits {
                    let path = t.get("trait").and_then(|v| v.as_str()).unwrap_or("?");
                    let keys = t.get("keys").and_then(|v| v.as_array())
                        .map(|arr| arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", "))
                        .unwrap_or_default();
                    out.push_str(&format!("{:<30} {}\n", path, keys));
                }
                out
            } else {
                format!("{}\n", serde_json::to_string_pretty(result).unwrap_or_default())
            }
        }
        _ => format!("{}\n", serde_json::to_string_pretty(result).unwrap_or_default()),
    }
}
