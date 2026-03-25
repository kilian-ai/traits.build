use kernel_logic::registry::TraitToml;
use serde_json::Value;
use std::collections::HashMap;

/// Minimal trait registry entry for WASM.
#[derive(Debug, Clone)]
pub struct WasmTraitEntry {
    pub path: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub tags: Vec<String>,
    pub provides: Vec<String>,
    pub language: String,
    pub source_type: String,
    pub params: Vec<Value>,
    pub returns_type: String,
    pub returns_description: String,
    pub wasm_callable: bool,
}

/// Lightweight trait registry backed by HashMap (no DashMap needed in WASM).
pub struct WasmRegistry {
    traits: HashMap<String, WasmTraitEntry>,
    /// Per-trait bindings: key is "trait_path/binding_key", value is the bound trait path.
    bindings: HashMap<String, String>,
    /// Per-trait requires: key is "trait_path/require_key", value is the interface path.
    requires: HashMap<String, String>,
}

impl WasmRegistry {
    pub fn new() -> Self {
        Self { traits: HashMap::new(), bindings: HashMap::new(), requires: HashMap::new() }
    }

    /// Load from the build.rs-generated BUILTIN_TRAIT_DEFS array.
    /// Each entry is (trait_path, rel_path, toml_content).
    pub fn load_builtins(&mut self, defs: &[(&str, &str, &str)]) {
        for (path, _rel, toml_content) in defs {
            let toml: TraitToml = match toml::from_str(toml_content) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let entry = wasm_entry_from_toml(path, &toml);
            self.traits.insert(path.to_string(), entry);

            // Extract [bindings] section
            if let Some(ref bindings) = toml.bindings {
                for (key, val) in bindings {
                    self.bindings.insert(format!("{}/{}", path, key), val.clone());
                }
            }
            // Extract [requires] section
            if let Some(ref requires) = toml.requires {
                for (key, val) in requires {
                    self.requires.insert(format!("{}/{}", path, key), val.clone());
                }
            }
        }
    }

    pub fn mark_wasm_callable(&mut self, path: &str) {
        if let Some(entry) = self.traits.get_mut(path) {
            entry.wasm_callable = true;
        }
    }

    pub fn get(&self, path: &str) -> Option<&WasmTraitEntry> {
        self.traits.get(path)
    }

    pub fn all(&self) -> Vec<&WasmTraitEntry> {
        let mut entries: Vec<&WasmTraitEntry> = self.traits.values().collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries
    }

    pub fn len(&self) -> usize {
        self.traits.len()
    }

    /// Resolve a keyed binding for a trait.
    /// Resolution: bindings[key] → requires[key] → interface auto-discover.
    pub fn resolve_keyed(&self, caller_path: &str, key: &str) -> Option<String> {
        let compound = format!("{}/{}", caller_path, key);
        // 1. Check bindings for this key
        if let Some(impl_path) = self.bindings.get(&compound) {
            return Some(impl_path.clone());
        }
        // 2. Fallback: resolve interface from requires[key] → find provider
        if let Some(interface_path) = self.requires.get(&compound) {
            // Find the unique trait that provides this interface
            let providers: Vec<&str> = self.traits.values()
                .filter(|e| e.provides.iter().any(|p| p == interface_path))
                .map(|e| e.path.as_str())
                .collect();
            if providers.len() == 1 {
                return Some(providers[0].to_string());
            }
        }
        None
    }
}

/// Convert a parsed TraitToml into a WASM registry entry.
fn wasm_entry_from_toml(path: &str, toml: &TraitToml) -> WasmTraitEntry {
    let http_defaults = toml.trait_def.http.as_ref()
        .map(|h| &h.defaults);

    let params: Vec<Value> = toml.signature.as_ref()
        .map(|sig| {
            sig.params.iter().map(|p| {
                let default_val = p.default.as_ref()
                    .map(|v| match v {
                        toml::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .or_else(|| http_defaults
                        .and_then(|d| d.get(&p.name).cloned()))
                    .unwrap_or_default();
                serde_json::json!({
                    "name": p.name,
                    "type": p.param_type,
                    "description": p.description,
                    "required": !p.is_optional(),
                    "default": default_val,
                })
            }).collect()
        })
        .unwrap_or_default();

    let (returns_type, returns_description) = toml.signature.as_ref()
        .map(|sig| (sig.returns.return_type.clone(), sig.returns.description.clone()))
        .unwrap_or_else(|| ("any".to_string(), String::new()));

    let author = if toml.trait_def.author.is_empty() {
        "system".to_string()
    } else {
        toml.trait_def.author.clone()
    };

    WasmTraitEntry {
        path: path.to_string(),
        description: toml.trait_def.description.clone(),
        version: toml.trait_def.version.clone(),
        author,
        tags: toml.trait_def.tags.clone(),
        provides: toml.trait_def.provides.clone(),
        language: toml.implementation.as_ref()
            .map(|i| i.language.clone())
            .unwrap_or_else(|| "rust".to_string()),
        source_type: toml.implementation.as_ref()
            .map(|i| i.source.clone())
            .unwrap_or_else(|| "builtin".to_string()),
        params,
        returns_type,
        returns_description,
        wasm_callable: false,
    }
}
