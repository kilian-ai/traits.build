use serde_json::Value;

// ── sys.cli.wasm — WASM terminal CLI backend (real implementation) ──
//
// This file is compiled into the WASM module via wasm_traits/mod.rs.
// It provides the cli/backend interface for the browser terminal,
// routing all methods through the WASM dispatch table and registry.
//
// Note: load_param_history / save_param_history are no-ops in WASM
// (no filesystem). History is kept in-memory per CliSession lifetime.

/// Dispatch entry: routes method calls to WASM-specific implementations.
pub fn wasm_dispatch(args: &[Value]) -> Value {
    let method = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let method_args = if args.len() > 1 { &args[1..] } else { &[] };

    match method {
        "call" => method_call(method_args),
        "list_all" => method_list_all(),
        "get_info" => method_get_info(method_args),
        "search" => method_search(method_args),
        "all_paths" => method_all_paths(),
        "version" => Value::String(env!("CARGO_PKG_VERSION").to_string()),
        "load_examples" => method_load_examples(method_args),
        // No-ops for WASM (no filesystem persistence)
        "load_param_history" => Value::Object(Default::default()),
        "save_param_history" => serde_json::json!({"ok": true}),
        // Default: introspection
        _ => serde_json::json!({
            "provides": "cli/backend",
            "target": "wasm",
            "methods": [
                "call", "list_all", "get_info", "search", "all_paths",
                "version", "load_examples", "load_param_history", "save_param_history"
            ]
        }),
    }
}

// ── Method implementations (WASM-specific) ──

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

    // Try WASM-local dispatch (helper-preferred check is inside dispatch())
    match super::dispatch(path, &call_args) {
        Some(result) => {
            // Check for WebLLM dispatch sentinel — pass through with enriched prompt
            if result.get("dispatch").and_then(|d| d.as_str()) == Some("webllm") {
                let prompt = result.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                let model = result.get("model").and_then(|v| v.as_str()).unwrap_or("");
                let sentinel_json = serde_json::json!({"prompt": prompt, "model": model});
                serde_json::json!({
                    "ok": false,
                    "error": format!("WEBLLM:{}", sentinel_json)
                })
            }
            // Other dispatch sentinels — delegate to REST
            else if result.get("dispatch").and_then(|d| d.as_str()).is_some() {
                serde_json::json!({
                    "ok": false,
                    "error": format!("REST:{}", path)
                })
            } else {
                serde_json::json!({"ok": true, "result": result})
            }
        }
        None => {
            // Check if trait exists but needs REST dispatch
            let reg = crate::get_registry();
            if reg.get(path).is_some() {
                serde_json::json!({
                    "ok": false,
                    "error": format!("REST:{}", path)
                })
            } else {
                serde_json::json!({
                    "ok": false,
                    "error": format!("Trait '{}' not found", path)
                })
            }
        }
    }
}

fn method_list_all() -> Value {
    let reg = crate::get_registry();
    let traits: Vec<Value> = reg
        .all()
        .iter()
        .map(|e| {
            serde_json::json!({
                "path": e.path,
                "description": e.description,
                "version": e.version,
                "tags": e.tags,
                "wasm_callable": e.wasm_callable,
            })
        })
        .collect();
    Value::Array(traits)
}

fn method_get_info(args: &[Value]) -> Value {
    let path = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let reg = crate::get_registry();

    match reg.get(path) {
        Some(e) => serde_json::json!({
            "path": e.path,
            "description": e.description,
            "version": e.version,
            "author": e.author,
            "tags": e.tags,
            "wasm_callable": e.wasm_callable,
            "params": e.params,
            "returns": e.returns_type,
            "returns_description": e.returns_description,
        }),
        None => Value::Null,
    }
}

fn method_search(args: &[Value]) -> Value {
    let query = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let reg = crate::get_registry();
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
                "wasm_callable": e.wasm_callable,
            })
        })
        .collect();
    Value::Array(results)
}

fn method_all_paths() -> Value {
    let reg = crate::get_registry();
    let paths: Vec<Value> = reg
        .all()
        .iter()
        .map(|e| Value::String(e.path.clone()))
        .collect();
    Value::Array(paths)
}

fn method_load_examples(args: &[Value]) -> Value {
    let trait_path = args.first().and_then(|v| v.as_str()).unwrap_or("");

    // In WASM, features are embedded at compile time via BUILTIN_FEATURES
    for &(tp, _rel_path, json_str) in crate::BUILTIN_FEATURES {
        if tp != trait_path {
            continue;
        }
        let parsed: Value = match serde_json::from_str(json_str) {
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
                                    other => Value::String(other.to_string()),
                                })
                                .collect();
                            examples.push(Value::Array(args));
                        }
                    }
                }
            }
        }
        return Value::Array(examples);
    }
    Value::Array(vec![])
}
