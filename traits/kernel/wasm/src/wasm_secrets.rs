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
/// Falls back to localStorage (traits.secret.KEY / traits.secret.KEY_UPPERCASE) so
/// secrets saved in the Settings UI are always visible even before JS calls set_secret().
pub fn get_secret(key: &str) -> Option<String> {
    // 1. Check in-memory store (fastest — already injected via set_secret / attachWasm)
    {
        let guard = SECRETS.lock().unwrap();
        if let Some(val) = guard.as_ref().and_then(|m| m.get(key).cloned()) {
            return Some(val);
        }
    }

    // 2. Fallback: read directly from localStorage so secrets saved in the Settings UI
    //    are visible without requiring a page reload or explicit set_secret() call.
    let value = (|| -> Option<String> {
        let storage = web_sys::window()?.local_storage().ok()??;
        // Try uppercase variant first (Settings saves as traits.secret.OPENAI_API_KEY)
        let upper = format!("traits.secret.{}", key.to_uppercase());
        if let Ok(Some(v)) = storage.get_item(&upper) {
            let v = v.trim().to_string();
            if !v.is_empty() { return Some(v); }
        }
        // Try lowercase variant (traits.secret.openai_api_key)
        let lower = format!("traits.secret.{}", key.to_lowercase());
        if let Ok(Some(v)) = storage.get_item(&lower) {
            let v = v.trim().to_string();
            if !v.is_empty() { return Some(v); }
        }
        None
    })();

    // Cache the value in-memory for subsequent calls
    if let Some(ref v) = value {
        let mut guard = SECRETS.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(key.to_string(), v.clone());
    }

    value
}
