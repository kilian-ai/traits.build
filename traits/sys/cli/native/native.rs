use serde_json::Value;

// ── sys.cli.native — Native terminal CLI backend ──
//
// Provides the cli/backend interface with method-based dispatch.
// Methods: call, list_all, get_info, search, all_paths, version,
//          load_param_history, save_param_history, load_examples
//
// Used by the native CLI (sys.cli) through the dispatch system.
// The WASM equivalent (sys.cli.wasm) has its own implementation
// compiled into the WASM module.

/// Dispatch entry: routes method calls to implementations.
pub fn native(args: &[Value]) -> Value {
    let method = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let method_args = if args.len() > 1 { &args[1..] } else { &[] };

    match method {
        "call" => method_call(method_args),
        "list_all" => method_list_all(),
        "get_info" => method_get_info(method_args),
        "search" => method_search(method_args),
        "all_paths" => method_all_paths(),
        "version" => Value::String(env!("CARGO_PKG_VERSION").to_string()),
        "load_param_history" => method_load_history(),
        "save_param_history" => method_save_history(method_args),
        "load_examples" => method_load_examples(method_args),
        // No method → return introspection
        _ => serde_json::json!({
            "provides": "cli/backend",
            "target": "native",
            "methods": [
                "call", "list_all", "get_info", "search", "all_paths",
                "version", "load_param_history", "save_param_history", "load_examples"
            ]
        }),
    }
}

// ── Method implementations ──

fn method_call(args: &[Value]) -> Value {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let call_args = args
        .get(1)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if path.is_empty() {
        return serde_json::json!({"ok": false, "error": "trait path is required"});
    }

    match crate::dispatcher::compiled::dispatch(path, &call_args) {
        Some(result) => serde_json::json!({"ok": true, "result": result}),
        None => serde_json::json!({"ok": false, "error": format!("Trait '{}' not found", path)}),
    }
}

fn method_list_all() -> Value {
    if let Some(reg) = crate::globals::REGISTRY.get() {
        let traits: Vec<Value> = reg
            .all()
            .iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "description": e.description,
                    "version": e.version,
                    "tags": e.tags,
                })
            })
            .collect();
        Value::Array(traits)
    } else {
        Value::Array(vec![])
    }
}

fn method_get_info(args: &[Value]) -> Value {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or("");

    let reg = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return Value::Null,
    };

    match reg.get(path) {
        Some(entry) => serde_json::json!({
            "path": entry.path,
            "description": entry.description,
            "version": entry.version,
            "author": entry.author,
            "tags": entry.tags,
            "params": entry.signature.params.iter().map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "type": format!("{:?}", p.param_type).to_lowercase(),
                    "description": p.description,
                    "required": !p.optional,
                })
            }).collect::<Vec<_>>(),
            "returns": format!("{:?}", entry.signature.returns.return_type).to_lowercase(),
            "returns_description": entry.signature.returns.description,
        }),
        None => Value::Null,
    }
}

fn method_search(args: &[Value]) -> Value {
    let query = args.first().and_then(|v| v.as_str()).unwrap_or("");

    if let Some(reg) = crate::globals::REGISTRY.get() {
        let q = query.to_lowercase();
        let results: Vec<Value> = reg
            .all()
            .iter()
            .filter(|e| {
                e.path.to_lowercase().contains(&q)
                    || e.description.to_lowercase().contains(&q)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "description": e.description,
                })
            })
            .collect();
        Value::Array(results)
    } else {
        Value::Array(vec![])
    }
}

fn method_all_paths() -> Value {
    if let Some(reg) = crate::globals::REGISTRY.get() {
        let paths: Vec<Value> = reg
            .all()
            .iter()
            .map(|e| Value::String(e.path.clone()))
            .collect();
        Value::Array(paths)
    } else {
        Value::Array(vec![])
    }
}

// ── Persistence methods (filesystem-based) ──

fn history_path() -> std::path::PathBuf {
    let traits_dir = crate::globals::TRAITS_DIR
        .get()
        .map(|p| p.as_path())
        .unwrap_or(std::path::Path::new("./traits"));
    traits_dir.join("sys").join("cli").join(".cli_history.json")
}

fn method_load_history() -> Value {
    let path = history_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or(Value::Object(Default::default())),
        Err(_) => Value::Object(Default::default()),
    }
}

fn method_save_history(args: &[Value]) -> Value {
    let history = args.first().unwrap_or(&Value::Null);
    let path = history_path();
    match serde_json::to_string_pretty(history) {
        Ok(json) => {
            let _ = std::fs::write(&path, json);
            serde_json::json!({"ok": true})
        }
        Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}),
    }
}

fn method_load_examples(args: &[Value]) -> Value {
    let trait_path = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let parts: Vec<&str> = trait_path.split('.').collect();
    if parts.len() < 2 {
        return Value::Array(vec![]);
    }

    let traits_dir = crate::globals::TRAITS_DIR
        .get()
        .map(|p| p.as_path())
        .unwrap_or(std::path::Path::new("./traits"));

    // Build path: traits/{ns}/{name}/{name}.features.json
    let mut dir = traits_dir.to_path_buf();
    for part in &parts {
        dir.push(part);
    }
    let feat_file = dir.join(format!("{}.features.json", parts.last().unwrap()));

    let content = match std::fs::read_to_string(&feat_file) {
        Ok(c) => c,
        Err(_) => return Value::Array(vec![]),
    };
    let parsed: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Value::Array(vec![]),
    };

    let mut examples = vec![];
    if let Some(features) = parsed.get("features").and_then(|v| v.as_array()) {
        for feature in features {
            if let Some(exs) = feature.get("examples").and_then(|v| v.as_array()) {
                for ex in exs {
                    if let Some(input) = ex.get("input").and_then(|v| v.as_array()) {
                        let args: Vec<Value> = input
                            .iter()
                            .map(|v| match v {
                                Value::String(s) => Value::String(s.clone()),
                                other => other.clone(),
                            })
                            .collect();
                        examples.push(Value::Array(args));
                    }
                }
            }
        }
    }
    Value::Array(examples)
}
