use serde_json::Value;

/// sys.registry — unified Registry read API.
///
/// Actions: list [ns] | info <path> | tree | namespaces | count | get <path> | search <q> | namespace <ns>
pub fn registry(args: &[Value]) -> Value {
    let reg = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return serde_json::json!({"error": "Registry not initialized"}),
    };

    let action = args.first().and_then(|v| v.as_str()).unwrap_or("tree");
    let arg2 = args.get(1).and_then(|v| v.as_str()).unwrap_or("");

    match action {
        // ── list: all traits or filtered by namespace ──
        "list" => {
            let mut traits = reg.all();
            if !arg2.is_empty() {
                let prefix = format!("{}.", arg2);
                traits.retain(|t| t.path.starts_with(&prefix) || t.path == arg2);
            }
            traits.sort_by(|a, b| a.path.cmp(&b.path));
            Value::Array(traits.iter().map(|t| t.to_summary_json()).collect())
        }

        // ── info: detailed trait info or namespace listing ──
        "info" => {
            if arg2.is_empty() {
                return serde_json::json!({"error": "info requires a trait path"});
            }
            if let Some(t) = reg.get(arg2) {
                return t.to_json();
            }
            let prefix = format!("{}.", arg2);
            let traits_in_ns: Vec<_> = reg.all().into_iter()
                .filter(|t| t.path.starts_with(&prefix) || t.path == arg2)
                .collect();
            if !traits_in_ns.is_empty() {
                return Value::Array(traits_in_ns.iter().map(|e| {
                    serde_json::json!({
                        "path": e.path,
                        "description": e.description,
                        "language": e.language.to_string()
                    })
                }).collect());
            }
            serde_json::json!({"error": format!("Trait not found: {}", arg2)})
        }

        "tree" => {
            let all = reg.all();
            let mut tree = Value::Object(serde_json::Map::new());
            for entry in &all {
                let parts: Vec<&str> = entry.path.split('.').collect();
                let mut current = &mut tree;
                for (i, part) in parts.iter().enumerate() {
                    if i == parts.len() - 1 {
                        if let Value::Object(ref mut map) = current {
                            map.insert(part.to_string(), entry.to_json());
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
            let mut ns_set = std::collections::BTreeSet::new();
            for t in &reg.all() {
                if let Some(ns) = t.path.split('.').next() {
                    ns_set.insert(ns.to_string());
                }
            }
            Value::Array(ns_set.into_iter().map(Value::String).collect())
        }

        "count" => Value::from(reg.len() as i64),

        "get" => {
            if arg2.is_empty() {
                return serde_json::json!({"error": "get requires a trait path"});
            }
            match reg.get(arg2) {
                Some(t) => t.to_json(),
                None => serde_json::json!({"error": format!("not found: {}", arg2)}),
            }
        }

        "search" => {
            if arg2.is_empty() {
                return serde_json::json!({"error": "search requires a query"});
            }
            let q = arg2.to_lowercase();
            let hits: Vec<Value> = reg.all().iter()
                .filter(|t| t.path.to_lowercase().contains(&q)
                          || t.description.to_lowercase().contains(&q))
                .map(|t| serde_json::json!({"path": t.path, "description": t.description}))
                .collect();
            Value::Array(hits)
        }

        "namespace" => {
            if arg2.is_empty() {
                return serde_json::json!({"error": "namespace requires a namespace name"});
            }
            let prefix = format!("{}.", arg2);
            let hits: Vec<Value> = reg.all().iter()
                .filter(|t| t.path.starts_with(&prefix) || t.path == arg2)
                .map(|t| serde_json::json!({"path": t.path, "description": t.description}))
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
