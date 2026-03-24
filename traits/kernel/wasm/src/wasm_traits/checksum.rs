use serde_json::Value;
use sha2::{Sha256, Digest};

/// SHA-256 of raw bytes → hex string.
fn sha256_bytes(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

/// SHA-256 of a UTF-8 string → hex string.
fn sha256_hex(input: &str) -> String {
    sha256_bytes(input.as_bytes())
}

/// Recursively sort object keys for deterministic hashing.
fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
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

/// SHA-256 of canonicalized JSON.
fn hash_stable(value: &Value) -> String {
    let canonical = canonicalize(value);
    let json = serde_json::to_string(&canonical).unwrap_or_default();
    sha256_hex(&json)
}

/// Extract I/O pairs from a feature's examples.
fn feature_io_pairs(feature: &Value) -> Vec<Value> {
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

/// Checksum over feature names + I/O pairs.
fn io_checksum(features: &[Value]) -> String {
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
            let mut entry = serde_json::json!({ "name": name, "io": io });
            if !assertions.is_empty() {
                entry["vtest_asserts"] =
                    Value::Array(assertions.into_iter().map(Value::String).collect());
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

/// Checksum for an update object (excludes "checksum" field).
fn checksum_for_update(update: &Value) -> String {
    let mut copy = update.clone();
    if let Value::Object(ref mut map) = copy {
        map.remove("checksum");
    }
    hash_stable(&copy)
}

/// Dispatch wrapper: checksum_dispatch(args) → Value
pub fn checksum_dispatch(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let data = args.get(1).unwrap_or(&Value::Null);
    checksum(action, data)
}

/// sys.checksum(action, data) — WASM version (no regex, so `signature` is unavailable).
pub fn checksum(action: &str, data: &Value) -> Value {
    match action {
        "hash" => serde_json::json!({ "ok": true, "checksum": hash_stable(data) }),
        "io" => {
            let features = data.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
            serde_json::json!({ "ok": true, "checksum": io_checksum(features) })
        }
        "update" => {
            if !data.is_object() {
                return serde_json::json!({ "error": "update action requires a release object" });
            }
            serde_json::json!({ "ok": true, "checksum": checksum_for_update(data) })
        }
        "signature" => {
            serde_json::json!({ "error": "signature action not available in WASM (requires regex)" })
        }
        _ => serde_json::json!({
            "error": format!("Unknown action: {}. Use hash, io, or update", action)
        }),
    }
}
