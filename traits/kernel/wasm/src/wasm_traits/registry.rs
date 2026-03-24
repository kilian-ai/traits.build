use serde_json::Value;
use std::collections::BTreeSet;

/// Access the WASM registry from the crate root (lib.rs).
fn get_registry() -> &'static crate::registry::WasmRegistry {
    crate::get_registry()
}

/// sys.registry — unified Registry read API for WASM.
///
/// Actions: list [ns] | info <path> | tree | namespaces | count | search <q> | namespace <ns>
pub fn registry_dispatch(args: &[Value]) -> Value {
    let reg = get_registry();
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("tree");
    let arg2 = args.get(1).and_then(|v| v.as_str()).unwrap_or("");

    match action {
        "list" => {
            let entries = reg.all();
            let filtered: Vec<_> = if arg2.is_empty() {
                entries
            } else {
                let prefix = format!("{}.", arg2);
                entries.into_iter()
                    .filter(|t| t.path.starts_with(&prefix) || t.path == arg2)
                    .collect()
            };
            Value::Array(filtered.iter().map(|t| entry_to_summary(t)).collect())
        }

        "info" => {
            if arg2.is_empty() {
                return serde_json::json!({"error": "info requires a trait path"});
            }
            if let Some(t) = reg.get(arg2) {
                return entry_to_detail(t);
            }
            // Try as namespace
            let prefix = format!("{}.", arg2);
            let in_ns: Vec<_> = reg.all().into_iter()
                .filter(|t| t.path.starts_with(&prefix) || t.path == arg2)
                .collect();
            if !in_ns.is_empty() {
                return Value::Array(in_ns.iter().map(|t| {
                    serde_json::json!({
                        "path": t.path,
                        "description": t.description,
                        "language": t.language,
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
                            map.insert(part.to_string(), entry_to_summary(entry));
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
            let mut ns_set = BTreeSet::new();
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
                Some(t) => entry_to_detail(t),
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

/// sys.list — list all traits or filter by namespace.
pub fn list_dispatch(args: &[Value]) -> Value {
    let namespace = args.first().and_then(|v| v.as_str()).unwrap_or("");
    registry_dispatch(&[Value::String("list".to_string()), Value::String(namespace.to_string())])
}

/// sys.info — detailed info for a specific trait.
pub fn info_dispatch(args: &[Value]) -> Value {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
    registry_dispatch(&[Value::String("info".to_string()), Value::String(path.to_string())])
}

// ── Helpers ──

fn entry_to_summary(t: &crate::registry::WasmTraitEntry) -> Value {
    serde_json::json!({
        "path": t.path,
        "description": t.description,
        "version": t.version,
        "tags": t.tags,
        "wasm_callable": t.wasm_callable,
    })
}

fn entry_to_detail(t: &crate::registry::WasmTraitEntry) -> Value {
    serde_json::json!({
        "path": t.path,
        "description": t.description,
        "version": t.version,
        "author": t.author,
        "tags": t.tags,
        "provides": t.provides,
        "language": t.language,
        "source": t.source_type,
        "wasm_callable": t.wasm_callable,
        "params": t.params,
        "returns": t.returns_type,
        "returns_description": t.returns_description,
    })
}
