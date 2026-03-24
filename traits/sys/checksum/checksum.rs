use serde_json::Value;
use sha2::{Sha256, Digest};

// Shared SHA-256 helpers (same code used by build.rs)
include!("checksum.sha256.rs");

/// Recursively sort object keys for deterministic hashing.
pub fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted: serde_json::Map<String, Value> = serde_json::Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                sorted.insert(k.clone(), canonicalize(&map[k]));
            }
            Value::Object(sorted)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

/// SHA-256 hash of a canonicalized JSON value.
pub fn hash_stable(value: &Value) -> String {
    let canonical = canonicalize(value);
    let json = serde_json::to_string(&canonical).unwrap_or_default();
    sha256_hex(&json)
}

/// SHA-256 hash of a raw string.
#[allow(dead_code)]
pub fn hash_hex(input: &str) -> String {
    sha256_hex(input)
}

/// SHA-256 hash of raw bytes.
#[allow(dead_code)]
pub fn hash_bytes(input: &[u8]) -> String {
    sha256_bytes(input)
}

/// Compute checksum for an update object (excludes the "checksum" field itself).
pub fn checksum_for_update(update: &Value) -> String {
    let mut copy = update.clone();
    if let Value::Object(ref mut map) = copy {
        map.remove("checksum");
    }
    hash_stable(&copy)
}

/// Extract I/O pairs from a feature's examples.
pub fn feature_io_pairs(feature: &Value) -> Vec<Value> {
    let examples = feature.get("examples").and_then(|e| e.as_array());
    match examples {
        Some(exs) if !exs.is_empty() => exs
            .iter()
            .map(|ex| {
                serde_json::json!({
                    "input": ex.get("input").cloned().unwrap_or(Value::Null),
                    "output": ex.get("output").cloned().unwrap_or(Value::Null),
                })
            })
            .collect(),
        _ => vec![],
    }
}

/// Compute checksum over feature names, I/O pairs, and vtest assertions.
pub fn io_checksum(features: &[Value]) -> String {
    let mut pairs: Vec<Value> = features
        .iter()
        .filter(|f| {
            f.is_object()
                && f.get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
        })
        .filter_map(|f| {
            let name = f["name"].as_str().unwrap_or("").trim().to_string();
            let io = feature_io_pairs(f);

            let vtests = f.get("vtests").and_then(|v| v.as_array());
            let assertions: Vec<String> = vtests
                .map(|vts| {
                    vts.iter()
                        .filter_map(|v| v.get("assert").and_then(|a| a.as_str()))
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();

            if io.is_empty() && assertions.is_empty() {
                return None;
            }

            let mut entry = serde_json::json!({
                "name": name,
                "io": io,
            });
            if !assertions.is_empty() {
                entry["vtest_asserts"] = Value::Array(
                    assertions.into_iter().map(Value::String).collect(),
                );
            }
            Some(entry)
        })
        .collect();

    pairs.sort_by(|a, b| {
        let na = a["name"].as_str().unwrap_or("");
        let nb = b["name"].as_str().unwrap_or("");
        na.cmp(nb)
    });

    hash_stable(&Value::Array(pairs))
}

/// Compute checksum over signature params extracted from TOML text.
/// Only available on native (requires regex crate).
#[cfg(not(target_arch = "wasm32"))]
pub fn signature_checksum(toml_text: &str) -> String {
    // Split TOML into [[signature.params]] blocks by finding each header
    let name_re = regex::Regex::new(r#"name\s*=\s*"([^"]+)""#).unwrap();
    let type_re = regex::Regex::new(r#"type\s*=\s*"([^"]+)""#).unwrap();
    let opt_re = regex::Regex::new(r"optional\s*=\s*(true|false)").unwrap();

    let mut params = Vec::new();
    // Find all [[signature.params]] blocks by splitting at section headers
    let header = "[[signature.params]]";
    for chunk in toml_text.split(header).skip(1) {
        // Each chunk runs until the next section header (line starting with '[')
        let block: String = chunk
            .lines()
            .take_while(|line| {
                let trimmed = line.trim();
                trimmed.is_empty() || !trimmed.starts_with('[')
            })
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(name_m) = name_re.captures(&block) {
            let name = name_m[1].to_string();
            let typ = type_re
                .captures(&block)
                .map(|m| m[1].to_string())
                .unwrap_or_else(|| "string".to_string());
            let optional = opt_re
                .captures(&block)
                .map(|m| &m[1] == "true")
                .unwrap_or(false);
            params.push(serde_json::json!({
                "name": name,
                "type": typ,
                "optional": optional,
            }));
        }
    }

    let mut returns = serde_json::Map::new();
    let ret_header = "[signature.returns]";
    if let Some(idx) = toml_text.find(ret_header) {
        let after = &toml_text[idx + ret_header.len()..];
        let ret_block: String = after
            .lines()
            .take_while(|line| {
                let trimmed = line.trim();
                trimmed.is_empty() || !trimmed.starts_with('[')
            })
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(rt) = type_re.captures(&ret_block) {
            returns.insert("type".to_string(), Value::String(rt[1].to_string()));
        }
    }

    hash_stable(&serde_json::json!({
        "params": params,
        "returns": returns,
    }))
}

/// Standard dispatch wrapper: checksum_dispatch(args) — uniform interface for auto-generation
pub fn checksum_dispatch(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let data = args.get(1).unwrap_or(&Value::Null);
    checksum(action, data)
}

/// Trait entry point: checksum(action, data)
pub fn checksum(action: &str, data: &Value) -> Value {
    match action {
        "hash" => serde_json::json!({ "ok": true, "checksum": hash_stable(data) }),
        "io" => {
            let features = data.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
            serde_json::json!({ "ok": true, "checksum": io_checksum(features) })
        }
        "signature" => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                match data.as_str() {
                    Some(toml_text) => {
                        serde_json::json!({ "ok": true, "checksum": signature_checksum(toml_text) })
                    }
                    None => serde_json::json!({ "error": "signature action requires TOML text as string" }),
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                serde_json::json!({ "error": "signature action not available in WASM (requires regex)" })
            }
        }
        "update" => {
            if !data.is_object() {
                return serde_json::json!({ "error": "update action requires a release object" });
            }
            serde_json::json!({ "ok": true, "checksum": checksum_for_update(data) })
        }
        _ => serde_json::json!({ "error": format!("Unknown action: {}. Use hash, io, signature, or update", action) }),
    }
}
