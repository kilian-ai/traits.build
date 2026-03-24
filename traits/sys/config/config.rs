use serde_json::Value;

/// Standard dispatch wrapper for build.rs auto-generation
pub fn config_dispatch(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let trait_path = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let key = args.get(2).and_then(|v| v.as_str()).unwrap_or("");
    let value = args.get(3).and_then(|v| v.as_str()).unwrap_or("");
    config_exec(action, trait_path, key, value)
}

/// Trait entry point
pub fn sys_config(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let trait_path = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let key = args.get(2).and_then(|v| v.as_str()).unwrap_or("");
    let value = args.get(3).and_then(|v| v.as_str()).unwrap_or("");
    config_exec(action, trait_path, key, value)
}

/// sys.config — manage persistent trait config values
///
/// Actions:
///   set <trait> <key> <value> — Store a config value in the persistent overlay
///   get <trait> <key>         — Resolve a config value (shows source layer)
///   delete <trait> <key>      — Remove a value from the persistent overlay
///   list [trait]              — List config for a specific trait or all traits with config
fn config_exec(action: &str, trait_path: &str, key: &str, value: &str) -> Value {
    match action {
        "set" => {
            if trait_path.is_empty() {
                return serde_json::json!({ "error": "trait path is required (e.g. www.admin)" });
            }
            if key.is_empty() {
                return serde_json::json!({ "error": "config key is required" });
            }
            if value.is_empty() {
                return serde_json::json!({ "error": "config value is required" });
            }
            // Validate trait path: alphanumeric + dots + underscores
            if !trait_path.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_') {
                return serde_json::json!({ "error": "trait path must be dot-separated (e.g. www.admin)" });
            }
            if !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return serde_json::json!({ "error": "config key must be alphanumeric with underscores" });
            }
            match crate::config::write_persistent_config(trait_path, key, value) {
                Ok(_) => serde_json::json!({
                    "ok": true,
                    "action": "set",
                    "trait": trait_path,
                    "key": key,
                    "value": value,
                }),
                Err(e) => serde_json::json!({ "error": e }),
            }
        }
        "get" => {
            if trait_path.is_empty() {
                return serde_json::json!({ "error": "trait path is required (e.g. www.admin)" });
            }
            if key.is_empty() {
                return serde_json::json!({ "error": "config key is required" });
            }
            // Resolve through full stack, also report where it came from
            let env_key = format!("{}_{}", trait_path.replace('.', "_"), key).to_uppercase();
            let env_val = std::env::var(&env_key).ok().filter(|v| !v.is_empty());
            let persistent_val = crate::config::read_persistent_config(trait_path, key);
            let toml_default = crate::globals::REGISTRY.get()
                .and_then(|reg| reg.get(trait_path))
                .and_then(|entry| entry.config.get(key).cloned());

            let (resolved, source) = if let Some(ref v) = env_val {
                (Some(v.clone()), "env")
            } else if let Some(ref v) = persistent_val {
                (Some(v.clone()), "persistent")
            } else if let Some(ref v) = toml_default {
                if v.starts_with("secret:") {
                    let secret_id = &v["secret:".len()..];
                    let ctx = crate::dispatcher::compiled::secrets::SecretContext::resolve(&[secret_id]);
                    if ctx.get(secret_id).is_some() {
                        (Some("(secret resolved)".to_string()), "secret")
                    } else {
                        (Some(v.clone()), "toml_default")
                    }
                } else {
                    (Some(v.clone()), "toml_default")
                }
            } else {
                (None, "none")
            };

            serde_json::json!({
                "ok": true,
                "action": "get",
                "trait": trait_path,
                "key": key,
                "value": resolved,
                "source": source,
                "env_var": env_key,
            })
        }
        "delete" => {
            if trait_path.is_empty() {
                return serde_json::json!({ "error": "trait path is required (e.g. www.admin)" });
            }
            if key.is_empty() {
                return serde_json::json!({ "error": "config key is required" });
            }
            match crate::config::delete_persistent_config(trait_path, key) {
                Ok(deleted) => serde_json::json!({
                    "ok": true,
                    "action": "delete",
                    "trait": trait_path,
                    "key": key,
                    "deleted": deleted,
                }),
                Err(e) => serde_json::json!({ "error": e }),
            }
        }
        "list" => {
            if !trait_path.is_empty() {
                // List config for a specific trait: merge toml defaults + persistent + env
                let mut entries = Vec::new();

                // Gather keys from .trait.toml [config] defaults
                let toml_keys: Vec<(String, String)> = crate::globals::REGISTRY.get()
                    .and_then(|reg| reg.get(trait_path))
                    .map(|entry| entry.config.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();

                // Gather keys from persistent overlay
                let persistent_keys = crate::config::read_persistent_config_section(trait_path);

                // Collect all unique keys
                let mut all_keys: Vec<String> = toml_keys.iter().map(|(k, _)| k.clone()).collect();
                for (k, _) in &persistent_keys {
                    if !all_keys.contains(k) {
                        all_keys.push(k.clone());
                    }
                }
                all_keys.sort();

                for k in &all_keys {
                    let resolved = crate::config::trait_config(trait_path, k);
                    let env_key = format!("{}_{}", trait_path.replace('.', "_"), k).to_uppercase();
                    let env_val = std::env::var(&env_key).ok().filter(|v| !v.is_empty());
                    let persistent_val = persistent_keys.iter().find(|(pk, _)| pk == k).map(|(_, v)| v.clone());
                    let toml_val = toml_keys.iter().find(|(tk, _)| tk == k).map(|(_, v)| v.clone());

                    let source = if env_val.is_some() {
                        "env"
                    } else if persistent_val.is_some() {
                        "persistent"
                    } else if toml_val.as_ref().map(|v| v.starts_with("secret:")).unwrap_or(false) {
                        "secret"
                    } else if toml_val.is_some() {
                        "toml_default"
                    } else {
                        "none"
                    };

                    // Mask secret values
                    let display_val = if source == "secret" {
                        Some("***".to_string())
                    } else {
                        resolved
                    };

                    entries.push(serde_json::json!({
                        "key": k,
                        "value": display_val,
                        "source": source,
                        "env_var": env_key,
                    }));
                }

                serde_json::json!({
                    "ok": true,
                    "action": "list",
                    "trait": trait_path,
                    "config": entries,
                    "count": entries.len(),
                })
            } else {
                // List all traits that have config (from registry + persistent overlay)
                let mut traits_with_config: Vec<Value> = Vec::new();

                // From registry: traits with [config] sections
                if let Some(reg) = crate::globals::REGISTRY.get() {
                    for entry in reg.all() {
                        if !entry.config.is_empty() {
                            traits_with_config.push(serde_json::json!({
                                "trait": entry.path,
                                "keys": entry.config.keys().collect::<Vec<_>>(),
                                "source": "toml_default",
                            }));
                        }
                    }
                }

                // From persistent overlay: traits with overrides
                let persistent_traits = crate::config::read_persistent_config_all();
                for (path, keys) in &persistent_traits {
                    // Check if already listed from registry
                    let already = traits_with_config.iter().any(|t| {
                        t.get("trait").and_then(|v| v.as_str()) == Some(path.as_str())
                    });
                    if already {
                        // Merge persistent keys into existing entry
                        if let Some(existing) = traits_with_config.iter_mut().find(|t| {
                            t.get("trait").and_then(|v| v.as_str()) == Some(path.as_str())
                        }) {
                            if let Some(obj) = existing.as_object_mut() {
                                obj.insert("persistent_keys".to_string(),
                                    serde_json::json!(keys.keys().collect::<Vec<_>>()));
                            }
                        }
                    } else {
                        traits_with_config.push(serde_json::json!({
                            "trait": path,
                            "keys": keys.keys().collect::<Vec<_>>(),
                            "source": "persistent",
                        }));
                    }
                }

                traits_with_config.sort_by(|a, b| {
                    let pa = a.get("trait").and_then(|v| v.as_str()).unwrap_or("");
                    let pb = b.get("trait").and_then(|v| v.as_str()).unwrap_or("");
                    pa.cmp(pb)
                });

                serde_json::json!({
                    "ok": true,
                    "action": "list",
                    "traits": traits_with_config,
                    "count": traits_with_config.len(),
                })
            }
        }
        _ => {
            serde_json::json!({
                "error": format!("Unknown action: {}. Use set, get, delete, or list", action),
                "actions": ["set", "get", "delete", "list"],
                "usage": {
                    "set": "sys.config set <trait> <key> <value>",
                    "get": "sys.config get <trait> <key>",
                    "delete": "sys.config delete <trait> <key>",
                    "list": "sys.config list [trait]",
                }
            })
        }
    }
}
