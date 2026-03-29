use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};

// ─── Secret primitive: zeroizes on drop, masks on Debug ────────────────────

/// A secret value that clears itself from memory when dropped.
/// Debug output is masked to prevent accidental logging.
struct Secret(String);

impl Drop for Secret {
    fn drop(&mut self) {
        // Overwrite each byte with zero before deallocation
        unsafe {
            let bytes = self.0.as_bytes_mut();
            for b in bytes.iter_mut() {
                std::ptr::write_volatile(b, 0);
            }
        }
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "***")
    }
}

// ─── AES-256-GCM crypto layer ──────────────────────────────────────────────
//
// Uses AES-256-GCM for authenticated encryption:
//   - 32-byte key derived from master secret via SHA-256
//   - 12-byte random nonce per encryption (cryptographically secure)
//   - Authenticated encryption (AEAD) prevents tampering
//
// This provides both confidentiality and integrity, unlike the previous
// XOR-based implementation which only provided confidentiality.

fn derive_key(master: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(master);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new_from_slice(key).expect("valid key size");
    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext).expect("encryption failed");
    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    output
}

fn decrypt(key: &[u8; 32], data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 12 {
        return None;
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ciphertext).ok()
}

// ─── Secret Store: encrypted at rest ────────────────────────────────────────

/// File-based encrypted secret store.
/// Secrets are encrypted with a master key and stored as JSON on disk.
/// The master key is derived from TRAITS_SECRET_KEY env var or a generated key file.
struct SecretStore {
    key: [u8; 32],
    store_path: PathBuf,
}

impl SecretStore {
    fn new() -> Self {
        let key = Self::get_or_create_key();
        let store_path = Self::store_file();
        Self { key, store_path }
    }

    fn store_file() -> PathBuf {
        // Use /data/secrets.enc if /data exists (Fly.io persistent volume)
        // Otherwise use $HOME/.traits/secrets.enc
        let data_dir = PathBuf::from("/data");
        if data_dir.exists() {
            data_dir.join("secrets.enc")
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            let dir = PathBuf::from(home).join(".traits");
            let _ = fs::create_dir_all(&dir);
            dir.join("secrets.enc")
        }
    }

    fn get_or_create_key() -> [u8; 32] {
        // Priority 1: TRAITS_SECRET_KEY env var
        if let Ok(env_key) = std::env::var("TRAITS_SECRET_KEY") {
            return derive_key(env_key.as_bytes());
        }

        // Priority 2: Key file on disk
        let key_path = {
            let data_dir = PathBuf::from("/data");
            if data_dir.exists() {
                data_dir.join(".secret_key")
            } else {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                let dir = PathBuf::from(home).join(".traits");
                let _ = fs::create_dir_all(&dir);
                dir.join(".secret_key")
            }
        };

        if let Ok(existing) = fs::read(&key_path) {
            if existing.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&existing);
                return key;
            }
        }

        // Generate new random key
        let key: [u8; 32] = rand::random();

        // Best effort: restrict permissions before writing key
        let _ = fs::write(&key_path, &key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600));
        }

        key
    }

    fn load(&self) -> HashMap<String, Vec<u8>> {
        let data = match fs::read(&self.store_path) {
            Ok(d) => d,
            Err(_) => return HashMap::new(),
        };
        match decrypt(&self.key, &data) {
            Some(plaintext) => serde_json::from_slice(&plaintext).unwrap_or_default(),
            None => HashMap::new(),
        }
    }

    fn save(&self, secrets: &HashMap<String, Vec<u8>>) -> bool {
        let json = serde_json::to_vec(secrets).unwrap_or_default();
        let encrypted = encrypt(&self.key, &json);
        if fs::write(&self.store_path, &encrypted).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&self.store_path, fs::Permissions::from_mode(0o600));
            }
            true
        } else {
            false
        }
    }

    fn set(&self, id: &str, value: &str) -> bool {
        let mut secrets = self.load();
        // Encrypt the individual value too (double encryption: value + store)
        let encrypted_value = encrypt(&self.key, value.as_bytes());
        secrets.insert(id.to_string(), encrypted_value);
        self.save(&secrets)
    }

    fn get(&self, id: &str) -> Option<Secret> {
        let secrets = self.load();
        secrets.get(id).and_then(|enc| {
            decrypt(&self.key, enc).and_then(|bytes| String::from_utf8(bytes).ok().map(Secret))
        })
    }

    fn delete(&self, id: &str) -> bool {
        let mut secrets = self.load();
        if secrets.remove(id).is_some() {
            self.save(&secrets)
        } else {
            false
        }
    }

    fn list(&self) -> Vec<String> {
        let secrets = self.load();
        let mut keys: Vec<String> = secrets.keys().cloned().collect();
        keys.sort();
        keys
    }
}

// ─── Global store (lazily initialized) ─────────────────────────────────────

fn with_store<F, R>(f: F) -> R
where
    F: FnOnce(&SecretStore) -> R,
{
    // Use a mutex to serialize access, create store on first use
    static STORE: Mutex<Option<SecretStore>> = Mutex::new(None);
    let mut guard = STORE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(SecretStore::new());
    }
    f(guard.as_ref().unwrap())
}

// ─── Secret Context: per-tool isolation ────────────────────────────────────

/// A scoped set of secrets that a specific tool/trait is allowed to access.
/// Secrets are resolved just-in-time and dropped (zeroized) after use.
pub struct SecretContext {
    secrets: HashMap<String, Secret>,
}

impl SecretContext {
    /// Build a context from allowed secret IDs.
    /// Only secrets in `allowed` are resolved; others are rejected.
    pub fn resolve(allowed: &[&str]) -> Self {
        let mut secrets = HashMap::new();
        with_store(|store| {
            for &id in allowed {
                if let Some(secret) = store.get(id) {
                    secrets.insert(id.to_string(), secret);
                }
            }
        });
        Self { secrets }
    }

    /// Get a secret by ID. Panics if not in allowed set.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.secrets.get(key).map(|s| s.0.as_str())
    }

    /// List which secrets were resolved.
    pub fn available(&self) -> Vec<&str> {
        self.secrets.keys().map(|s| s.as_str()).collect()
    }
}

// ─── Trait dispatch ────────────────────────────────────────────────────────

/// Standard dispatch wrapper for build.rs auto-generation
pub fn secrets_dispatch(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let id = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let value = args.get(2).and_then(|v| v.as_str()).unwrap_or("");
    secrets_exec(action, id, value)
}

/// Trait entry point: secrets(args) — accepts &[Value], dispatches by action
pub fn secrets(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let id = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let value = args.get(2).and_then(|v| v.as_str()).unwrap_or("");
    secrets_exec(action, id, value)
}

/// Trait implementation: secrets_exec(action, id?, value?)
///
/// Actions:
///   set <id> <value>  — Store a secret (encrypted at rest)
///   get <id>          — Retrieve a secret (returns masked confirmation, not the value)
///   delete <id>       — Remove a secret from the store
///   list              — List all secret IDs (values never exposed)
///   resolve <ids>     — Build a SecretContext for allowed IDs (internal use)
fn secrets_exec(action: &str, id: &str, value: &str) -> Value {
    match action {
        "set" => {
            if id.is_empty() {
                return serde_json::json!({ "error": "secret id is required" });
            }
            if value.is_empty() {
                return serde_json::json!({ "error": "secret value is required" });
            }
            // Validate ID: alphanumeric + underscores only
            if !id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
            {
                return serde_json::json!({ "error": "secret id must be alphanumeric (a-z, 0-9, _, .)" });
            }
            let ok = with_store(|store| store.set(id, value));
            if ok {
                serde_json::json!({ "ok": true, "action": "set", "id": id })
            } else {
                serde_json::json!({ "error": "failed to write secret store" })
            }
        }
        "get" => {
            if id.is_empty() {
                return serde_json::json!({ "error": "secret id is required" });
            }
            let exists = with_store(|store| store.get(id).is_some());
            serde_json::json!({
                "ok": true,
                "action": "get",
                "id": id,
                "exists": exists,
                // NEVER return the actual secret value — only confirm existence
            })
        }
        "delete" => {
            if id.is_empty() {
                return serde_json::json!({ "error": "secret id is required" });
            }
            let deleted = with_store(|store| store.delete(id));
            serde_json::json!({
                "ok": true,
                "action": "delete",
                "id": id,
                "deleted": deleted,
            })
        }
        "list" => {
            let ids = with_store(|store| store.list());
            serde_json::json!({
                "ok": true,
                "action": "list",
                "secrets": ids,
                "count": ids.len(),
            })
        }
        "resolve" => {
            // Resolve a set of secret IDs into a context (returns available IDs only)
            // The actual secret values are only available via SecretContext in Rust code
            let allowed: Vec<&str> = if id.is_empty() {
                vec![]
            } else {
                id.split(',').map(|s| s.trim()).collect()
            };
            let ctx = SecretContext::resolve(&allowed);
            let available = ctx.available();
            serde_json::json!({
                "ok": true,
                "action": "resolve",
                "requested": allowed,
                "available": available,
                "count": available.len(),
            })
        }
        _ => {
            serde_json::json!({
                "error": format!("Unknown action: {}. Use set, get, delete, list, or resolve", action),
                "actions": ["set", "get", "delete", "list", "resolve"],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_key(b"test-master-key");
        let plaintext = b"hello secret world";
        let encrypted = encrypt(&key, plaintext);
        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_secret_debug_is_masked() {
        let s = Secret("super_secret_value".to_string());
        assert_eq!(format!("{:?}", s), "***");
    }

    #[test]
    fn test_different_nonces() {
        let key = derive_key(b"test-key");
        let e1 = encrypt(&key, b"data");
        let e2 = encrypt(&key, b"data");
        assert_ne!(e1, e2);
        assert_eq!(decrypt(&key, &e1), decrypt(&key, &e2));
    }

    #[test]
    fn test_tamper_detection() {
        let key = derive_key(b"test-key");
        let encrypted = encrypt(&key, b"secret data");
        let mut tampered = encrypted.clone();
        tampered[13] ^= 0x42;
        assert!(decrypt(&key, &tampered).is_none());
    }
}
