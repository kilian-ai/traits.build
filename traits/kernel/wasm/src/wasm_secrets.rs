use std::collections::HashMap;
use std::sync::Mutex;

static SECRETS: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// Store a secret in the WASM in-memory secret store.
pub fn set_secret(key: &str, value: &str) {
    let mut guard = SECRETS.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(key.to_string(), value.to_string());
}

/// Retrieve a secret from the WASM in-memory store.
pub fn get_secret(key: &str) -> Option<String> {
    let guard = SECRETS.lock().unwrap();
    guard.as_ref().and_then(|m| m.get(key).cloned())
}
