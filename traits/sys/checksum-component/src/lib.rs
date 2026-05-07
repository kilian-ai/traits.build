//! sys.checksum — Component Model implementation.
//!
//! This is a Stage 3 proof of concept: the same trait as the cdylib in the
//! sibling `checksum/` directory, but compiled as a Wasm component conforming
//! to the WIT in `wit/checksum.wit`. Currently implements the `hash` action;
//! other actions return a structured error.

#[allow(warnings, clippy::all)]
mod bindings;

use sha2::{Digest, Sha256};

struct Component;

impl bindings::exports::traits::sys_checksum::checksum::Guest for Component {
    fn checksum(action: String, data: String) -> Result<String, String> {
        match action.as_str() {
            "hash" => {
                // Match the existing native trait: SHA-256 over canonical JSON
                // when input parses as JSON, otherwise over the raw bytes.
                let canonical = match serde_json::from_str::<serde_json::Value>(&data) {
                    Ok(v) => serde_json::to_string(&canonicalize(&v)).unwrap_or(data.clone()),
                    Err(_) => data.clone(),
                };
                let mut h = Sha256::new();
                h.update(canonical.as_bytes());
                let hex = h
                    .finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                Ok(serde_json::json!({ "ok": true, "checksum": hex }).to_string())
            }
            other => Err(format!(
                "action '{other}' not implemented in this component (try 'hash')"
            )),
        }
    }
}

fn canonicalize(v: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                sorted.insert(k.clone(), canonicalize(&map[k]));
            }
            Value::Object(sorted)
        }
        Value::Array(a) => Value::Array(a.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

bindings::export!(Component with_types_in bindings);
