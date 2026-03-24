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
    /// Populated from [wasm_bindings] first, falling back to [bindings].
    bindings: HashMap<String, String>,
}

impl WasmRegistry {
    pub fn new() -> Self {
        Self { traits: HashMap::new(), bindings: HashMap::new() }
    }

    /// Load from the build.rs-generated BUILTIN_TRAIT_DEFS array.
    /// Each entry is (trait_path, rel_path, toml_content).
    pub fn load_builtins(&mut self, defs: &[(&str, &str, &str)]) {
        for (path, _rel, toml_content) in defs {
            if let Ok(parsed) = toml_content.parse::<toml::Value>() {
                let entry = parse_trait_toml(path, &parsed);
                self.traits.insert(path.to_string(), entry);

                // Extract bindings: [wasm_bindings] overrides [bindings]
                let base = parsed.get("bindings")
                    .and_then(|v| v.as_table());
                let wasm = parsed.get("wasm_bindings")
                    .and_then(|v| v.as_table());

                if let Some(table) = base {
                    for (key, val) in table {
                        if let Some(s) = val.as_str() {
                            self.bindings.insert(format!("{}/{}", path, key), s.to_string());
                        }
                    }
                }
                // wasm_bindings override base bindings for matching keys
                if let Some(table) = wasm {
                    for (key, val) in table {
                        if let Some(s) = val.as_str() {
                            self.bindings.insert(format!("{}/{}", path, key), s.to_string());
                        }
                    }
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

    /// Resolve a keyed binding for a trait (e.g., resolve_binding("kernel.cli", "backend")).
    /// Returns the bound trait path from [wasm_bindings] (preferred) or [bindings].
    pub fn resolve_binding(&self, trait_path: &str, key: &str) -> Option<String> {
        self.bindings.get(&format!("{}/{}", trait_path, key)).cloned()
    }
}

fn parse_trait_toml(path: &str, toml: &toml::Value) -> WasmTraitEntry {
    let trait_def = toml.get("trait").unwrap_or(toml);
    let impl_def = toml.get("implementation");
    let sig = toml.get("signature");
    let http_defaults = trait_def
        .get("http")
        .and_then(|h| h.get("defaults"));

    let description = trait_def.get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let version = trait_def.get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("v0")
        .to_string();

    let author = trait_def.get("author")
        .and_then(|v| v.as_str())
        .unwrap_or("system")
        .to_string();

    let tags: Vec<String> = trait_def.get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let provides: Vec<String> = trait_def.get("provides")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let language = impl_def
        .and_then(|i| i.get("language"))
        .and_then(|v| v.as_str())
        .unwrap_or("rust")
        .to_string();

    let source_type = impl_def
        .and_then(|i| i.get("source"))
        .and_then(|v| v.as_str())
        .unwrap_or("builtin")
        .to_string();

    // Parse params
    let params: Vec<Value> = sig
        .and_then(|s| s.get("params"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().map(|p| {
                let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let required = p.get("required")
                    .and_then(|v| v.as_bool())
                    .unwrap_or_else(|| !p.get("optional").and_then(|v| v.as_bool()).unwrap_or(false));
                let default_val = p.get("default")
                    .and_then(|v| v.as_str())
                    .or_else(|| http_defaults.and_then(|d| d.get(name)).and_then(|v| v.as_str()))
                    .unwrap_or("");
                serde_json::json!({
                    "name": name,
                    "type": p.get("type").and_then(|v| v.as_str()).unwrap_or("any"),
                    "description": p.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                    "required": required,
                    "default": default_val,
                })
            }).collect()
        })
        .unwrap_or_default();

    let returns_type = sig
        .and_then(|s| s.get("returns"))
        .and_then(|r| r.get("type").or(Some(r)))
        .and_then(|v| v.as_str())
        .unwrap_or("any")
        .to_string();

    let returns_description = sig
        .and_then(|s| s.get("returns"))
        .and_then(|r| r.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    WasmTraitEntry {
        path: path.to_string(),
        description,
        version,
        author,
        tags,
        provides,
        language,
        source_type,
        params,
        returns_type,
        returns_description,
        wasm_callable: false,
    }
}
