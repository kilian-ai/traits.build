use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub traits: TraitsConfig,
    #[serde(default)]
    pub workers: HashMap<String, WorkerConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct TraitsConfig {
    #[serde(default = "default_traits_dir")]
    pub traits_dir: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_bindings_file")]
    pub bindings_file: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct WorkerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Number of worker processes to spawn per language (default: 1)
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
}

fn default_pool_size() -> usize {
    1
}

fn default_traits_dir() -> String {
    "./traits".into()
}

fn default_port() -> u16 {
    8080
}

fn default_bind() -> String {
    "127.0.0.1".into()
}

fn default_timeout() -> u64 {
    30
}

fn default_bindings_file() -> String {
    "./state/bindings.cl".into()
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = Path::new(path);
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let mut config: Config = toml::from_str(&content)?;

            // Apply environment variable overrides
            if let Ok(port) = std::env::var("TRAITS_PORT") {
                if let Ok(p) = port.parse() {
                    config.traits.port = p;
                }
            }
            if let Ok(dir) = std::env::var("TRAITS_DIR") {
                config.traits.traits_dir = dir;
            }
            if let Ok(bind) = std::env::var("TRAITS_BIND") {
                if !bind.is_empty() {
                    config.traits.bind = bind;
                }
            }
            if let Ok(timeout) = std::env::var("TRAITS_TIMEOUT") {
                if let Ok(t) = timeout.parse() {
                    config.traits.timeout = t;
                }
            }

            Ok(config)
        } else {
            // Return defaults
            Ok(Config {
                traits: TraitsConfig {
                    traits_dir: default_traits_dir(),
                    port: default_port(),
                    bind: default_bind(),
                    timeout: default_timeout(),
                    bindings_file: default_bindings_file(),
                },
                workers: HashMap::new(),
            })
        }
    }
}

// ── Trait dispatch entry point ──

/// kernel.config introspection: returns current config as JSON.
pub fn config(args: &[serde_json::Value]) -> serde_json::Value {
    // If "schema" arg, return config structure description
    if args.first().and_then(|v| v.as_str()) == Some("schema") {
        return serde_json::json!({
            "fields": {
                "traits.traits_dir": "string — directory containing trait definitions",
                "traits.port": "int — server listen port",
                "traits.bind": "string — bind address (default: 127.0.0.1)",
                "traits.timeout": "int — default call timeout in seconds",
                "traits.bindings_file": "string — path to bindings.cl",
                "workers": "map — per-language worker configs"
            }
        });
    }
    // Return current config from globals
    match crate::globals::CONFIG.get() {
        Some(cfg) => serde_json::json!({
            "traits_dir": cfg.traits.traits_dir,
            "port": cfg.traits.port,
            "bind": cfg.traits.bind,
            "timeout": cfg.traits.timeout,
            "bindings_file": cfg.traits.bindings_file,
            "workers": cfg.workers.keys().collect::<Vec<_>>()
        }),
        None => serde_json::json!({"error": "config not initialized"}),
    }
}

/// Resolve a config value for a trait.
///
/// Resolution order (first non-empty wins):
///   1. Environment variable: `{TRAIT_PATH}_{KEY}` (dots→underscores, uppercased)
///      e.g. `www.admin` key `fly_app` → `WWW_ADMIN_FLY_APP`
///   2. Persistent override file (`/data/trait_config.toml` on Fly, or local fallback)
///   3. sys.secrets store (if the default value starts with `secret:`)
///   4. The `[config]` default from the trait's .trait.toml
///
/// Returns None if no value found at any level.
pub fn trait_config(trait_path: &str, key: &str) -> Option<String> {
    // 1. Env var override: WWW_ADMIN_FLY_APP
    let env_key = format!("{}_{}", trait_path.replace('.', "_"), key).to_uppercase();
    if let Ok(val) = std::env::var(&env_key) {
        if !val.is_empty() {
            return Some(val);
        }
    }

    // 2. Persistent override file (trait_config.toml)
    //    Format: ["www.admin"] fly_app = "polygrait-api"
    if let Some(val) = read_persistent_config(trait_path, key) {
        return Some(val);
    }

    // 3. Look up [config] default from registry
    let default_val = crate::globals::REGISTRY.get()
        .and_then(|reg| reg.get(trait_path))
        .and_then(|entry| entry.config.get(key).cloned());

    match default_val {
        // 4. If default starts with "secret:", resolve from sys.secrets
        Some(ref val) if val.starts_with("secret:") => {
            let secret_id = &val["secret:".len()..];
            let ctx = crate::dispatcher::compiled::secrets::SecretContext::resolve(&[secret_id]);
            ctx.get(secret_id).map(|s| s.to_string()).or(default_val)
        }
        other => other,
    }
}

/// Convenience: resolve a config value with a fallback default.
pub fn trait_config_or(trait_path: &str, key: &str, fallback: &str) -> String {
    trait_config(trait_path, key).unwrap_or_else(|| fallback.to_string())
}

/// Path to the persistent trait config override file.
pub fn persistent_config_path() -> &'static str {
    if std::path::Path::new("/data").is_dir() {
        "/data/trait_config.toml"
    } else {
        "trait_config.toml"
    }
}

/// Read a value from the persistent config overlay.
fn read_persistent_config(trait_path: &str, key: &str) -> Option<String> {
    let content = std::fs::read_to_string(persistent_config_path()).ok()?;
    let table: toml::Value = toml::from_str(&content).ok()?;
    // Trait paths use dots, TOML sections use quoted keys: ["www.admin"]
    table.get(trait_path)?
        .get(key)?
        .as_str()
        .map(|s| s.to_string())
}

/// Write a config value to the persistent overlay file.
/// Creates the file if it doesn't exist; merges into existing content.
pub fn write_persistent_config(trait_path: &str, key: &str, value: &str) -> Result<(), String> {
    let path = persistent_config_path();
    let mut table: toml::value::Table = if let Ok(content) = std::fs::read_to_string(path) {
        toml::from_str(&content).unwrap_or_default()
    } else {
        toml::value::Table::new()
    };

    let section = table.entry(trait_path.to_string())
        .or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    if let Some(t) = section.as_table_mut() {
        t.insert(key.to_string(), toml::Value::String(value.to_string()));
    }

    let content = toml::to_string_pretty(&table)
        .map_err(|e| format!("TOML serialize error: {}", e))?;
    std::fs::write(path, content)
        .map_err(|e| format!("Cannot write {}: {}", path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_defaults() {
        let config = Config::load("nonexistent.toml").unwrap();
        assert_eq!(config.traits.port, 8080);
        assert_eq!(config.traits.timeout, 30);
        assert_eq!(config.traits.traits_dir, "./traits");
    }

    #[test]
    fn test_load_from_file() {
        let mut f = NamedTempFile::new().unwrap();
        write!(
            f,
            r#"
[traits]
port = 9090
timeout = 60
traits_dir = "/custom/dir"

[workers.python]
command = "python3"
args = ["-u"]
"#
        )
        .unwrap();

        let config = Config::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(config.traits.port, 9090);
        assert_eq!(config.traits.timeout, 60);
        assert_eq!(config.traits.traits_dir, "/custom/dir");
        assert!(config.workers.contains_key("python"));
    }
}
