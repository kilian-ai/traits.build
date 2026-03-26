use serde_json::Value;

// ── Registry access via platform abstraction ──

fn get_all_entries() -> Vec<Value> {
    kernel_logic::platform::registry_all()
}

fn get_entry_detail(path: &str) -> Option<Value> {
    kernel_logic::platform::registry_detail(path)
}

fn registry_count() -> usize {
    kernel_logic::platform::registry_count()
}

// ── Shared dispatch logic (identical on both targets) ──

/// sys.registry — unified Registry read API.
///
/// Actions: list [ns] | info <path> | tree | namespaces | count | get <path> | search <q> | namespace <ns>
pub fn registry(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("tree");
    let arg2 = args.get(1).and_then(|v| v.as_str()).unwrap_or("");

    match action {
        // ── list: all traits or filtered by namespace ──
        "list" => {
            let entries = get_all_entries();
            if arg2.is_empty() {
                return Value::Array(entries);
            }
            let prefix = format!("{}.", arg2);
            let filtered: Vec<Value> = entries.into_iter()
                .filter(|e| {
                    let p = e.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    p.starts_with(&prefix) || p == arg2
                })
                .collect();
            Value::Array(filtered)
        }

        // ── info: detailed trait info or namespace listing ──
        "info" => {
            if arg2.is_empty() {
                return serde_json::json!({"error": "info requires a trait path"});
            }
            if let Some(detail) = get_entry_detail(arg2) {
                return detail;
            }
            // Try as namespace
            let prefix = format!("{}.", arg2);
            let entries = get_all_entries();
            let in_ns: Vec<Value> = entries.into_iter()
                .filter(|e| {
                    let p = e.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    p.starts_with(&prefix) || p == arg2
                })
                .map(|e| {
                    serde_json::json!({
                        "path": e.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                        "description": e.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                        "language": e.get("language").and_then(|v| v.as_str()).unwrap_or("rust"),
                    })
                })
                .collect();
            if !in_ns.is_empty() {
                return Value::Array(in_ns);
            }
            serde_json::json!({"error": format!("Trait not found: {}", arg2)})
        }

        "tree" => {
            let all = get_all_entries();
            let mut tree = Value::Object(serde_json::Map::new());
            for entry in &all {
                let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let parts: Vec<&str> = path.split('.').collect();
                let mut current = &mut tree;
                for (i, part) in parts.iter().enumerate() {
                    if i == parts.len() - 1 {
                        if let Value::Object(ref mut map) = current {
                            map.insert(part.to_string(), entry.clone());
                        }
                    } else {
                        if let Value::Object(ref mut map) = current {
                            if !map.contains_key(*part) {
                                map.insert(part.to_string(), Value::Object(serde_json::Map::new()));
                            }
                        }
                        if let Value::Object(ref mut map) = current {
                            current = map.get_mut(*part).unwrap();
                        }
                    }
                }
            }
            tree
        }

        "namespaces" => {
            let entries = get_all_entries();
            let mut ns_set = std::collections::BTreeSet::new();
            for e in &entries {
                if let Some(p) = e.get("path").and_then(|v| v.as_str()) {
                    if let Some(ns) = p.split('.').next() {
                        ns_set.insert(ns.to_string());
                    }
                }
            }
            Value::Array(ns_set.into_iter().map(Value::String).collect())
        }

        "count" => Value::from(registry_count() as i64),

        "get" => {
            if arg2.is_empty() {
                return serde_json::json!({"error": "get requires a trait path"});
            }
            match get_entry_detail(arg2) {
                Some(detail) => detail,
                None => serde_json::json!({"error": format!("not found: {}", arg2)}),
            }
        }

        "search" => {
            if arg2.is_empty() {
                return serde_json::json!({"error": "search requires a query"});
            }
            let q = arg2.to_lowercase();
            let hits: Vec<Value> = get_all_entries().into_iter()
                .filter(|e| {
                    let p = e.get("path").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                    let d = e.get("description").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                    p.contains(&q) || d.contains(&q)
                })
                .map(|e| serde_json::json!({
                    "path": e.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                    "description": e.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                }))
                .collect();
            Value::Array(hits)
        }

        "namespace" => {
            if arg2.is_empty() {
                return serde_json::json!({"error": "namespace requires a namespace name"});
            }
            let prefix = format!("{}.", arg2);
            let hits: Vec<Value> = get_all_entries().into_iter()
                .filter(|e| {
                    let p = e.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    p.starts_with(&prefix) || p == arg2
                })
                .map(|e| serde_json::json!({
                    "path": e.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                    "description": e.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                }))
                .collect();
            Value::Array(hits)
        }

        _ => serde_json::json!({"error": format!("Unknown action: {}", action)}),
    }
}

/// sys.list — list all registered traits (delegates to registry "list" action).
pub fn list(args: &[Value]) -> Value {
    let namespace = args.first().and_then(|v| v.as_str()).unwrap_or("");
    registry(&[Value::String("list".to_string()), Value::String(namespace.to_string())])
}

/// sys.info — show detailed info about a specific trait (delegates to registry "info" action).
pub fn info(args: &[Value]) -> Value {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
    registry(&[Value::String("info".to_string()), Value::String(path.to_string())])
}
