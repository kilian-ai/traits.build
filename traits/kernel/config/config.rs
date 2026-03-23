use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub traits: TraitsConfig,
    #[serde(default)]
    pub deploy: DeployConfig,
    #[serde(default)]
    pub workers: HashMap<String, WorkerConfig>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DeployConfig {
    #[serde(default = "default_fly_app")]
    pub fly_app: String,
    #[serde(default = "default_fly_region")]
    pub fly_region: String,
}

fn default_fly_app() -> String {
    std::env::var("FLY_APP").unwrap_or_else(|_| "your-fly-app".into())
}

fn default_fly_region() -> String {
    std::env::var("FLY_REGION").unwrap_or_else(|_| "iad".into())
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
            // Deploy overrides: persistent volume → env vars
            // Check /data/deploy.toml first (Fly.io persistent volume)
            if let Ok(deploy_content) = std::fs::read_to_string("/data/deploy.toml") {
                if let Ok(overlay) = toml::from_str::<toml::Value>(&deploy_content) {
                    if let Some(deploy) = overlay.get("deploy") {
                        if let Some(app) = deploy.get("fly_app").and_then(|v| v.as_str()) {
                            if !app.is_empty() { config.deploy.fly_app = app.to_string(); }
                        }
                        if let Some(region) = deploy.get("fly_region").and_then(|v| v.as_str()) {
                            if !region.is_empty() { config.deploy.fly_region = region.to_string(); }
                        }
                    }
                }
            }
            // Env vars override everything
            if let Ok(app) = std::env::var("FLY_APP") {
                if !app.is_empty() {
                    config.deploy.fly_app = app;
                }
            }
            if let Ok(region) = std::env::var("FLY_REGION") {
                if !region.is_empty() {
                    config.deploy.fly_region = region;
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
                deploy: DeployConfig::default(),
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
            "deploy": {
                "fly_app": cfg.deploy.fly_app,
                "fly_region": cfg.deploy.fly_region,
            },
            "workers": cfg.workers.keys().collect::<Vec<_>>()
        }),
        None => serde_json::json!({"error": "config not initialized"}),
    }
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
