use crate::dispatcher::CallConfig;
use crate::types::{Language, TraitSignature, TraitValue};
use dashmap::DashMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn, error};

// Re-export shared TOML structs and parsing from kernel-logic
pub use kernel_logic::registry::{
    BuiltinTraitDef, TraitToml, TraitDefToml, SignatureToml, ParamToml, ReturnToml,
    ImplementationToml, CliMapToml, HttpTraitConfig,
    parse_type, parse_language, toml_to_json, toml_value_to_trait_value, derive_trait_path,
    build_params, build_returns, build_signature, resolve_language, parse_config_section,
};

// ── Builtin trait definitions (embedded by build.rs) ──
include!(concat!(env!("OUT_DIR"), "/builtin_traits.rs"));

/// A single registered trait
#[derive(Debug, Clone)]
pub struct TraitEntry {
    pub path: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub tags: Vec<String>,
    pub published: Option<String>,
    pub imports: Vec<String>,
    pub gui: Option<String>,
    pub frontend: Option<String>,
    pub startup_args: Option<Vec<serde_json::Value>>,
    pub signature: TraitSignature,
    pub language: Language,
    pub source: PathBuf,
    pub entry: String,
    #[allow(dead_code)]
    pub toml_path: PathBuf,
    pub stream: bool,
    pub background: bool,
    pub kind: String,
    pub command: Option<String>,
    pub codegen: Option<std::collections::HashMap<String, String>>,
    pub sources: Option<std::collections::HashMap<String, String>>,
    pub http: Option<HttpTraitConfig>,
    pub requires: HashMap<String, String>,
    pub provides: Vec<String>,
    pub trait_bindings: HashMap<String, String>,
    pub priority: i32,
    pub load: Option<CallConfig>,
    #[allow(dead_code)]
    pub cli_map_source: Option<PathBuf>,
    /// Per-trait config from [config] section in .trait.toml
    pub config: HashMap<String, String>,
}

impl TraitEntry {
    /// Full JSON serialization of this trait entry.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path,
            "description": self.description,
            "version": self.version,
            "author": self.author,
            "tags": self.tags,
            "published": self.published,
            "imports": self.imports,
            "gui": self.gui,
            "frontend": self.frontend,
            "startup_args": self.startup_args,
            "stream": self.stream,
            "background": self.background,
            "kind": self.kind,
            "command": self.command,
            "codegen": self.codegen,
            "sources": self.sources,
            "http": self.http,
            "requires": self.requires,
            "provides": self.provides,
            "bindings": self.trait_bindings,
            "priority": self.priority,
            "language": self.language.to_string(),
            "entry": self.entry,
            "source": self.source.display().to_string(),
            "signature": {
                "params": self.signature.params.iter().map(|p| {
                    serde_json::json!({
                        "name": p.name,
                        "type": format!("{:?}", p.param_type),
                        "description": p.description,
                        "optional": p.optional
                    })
                }).collect::<Vec<_>>(),
                "returns": format!("{:?}", self.signature.returns.return_type),
                "returns_description": self.signature.returns.description
            }
        })
    }

    /// Compact summary: path, description, language, version, params, returns.
    pub fn to_summary_json(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path,
            "description": self.description,
            "language": self.language.to_string(),
            "version": self.version,
            "params": self.signature.params.iter().map(|p| {
                serde_json::json!({ "name": p.name, "type": format!("{:?}", p.param_type) })
            }).collect::<Vec<_>>(),
            "returns": format!("{:?}", self.signature.returns.return_type)
        })
    }
}

// ── Native-only: load config parsing (depends on CallConfig) ──

fn parse_load_config(raw: &Option<HashMap<String, toml::Value>>) -> Option<CallConfig> {
    let raw = raw.as_ref()?;
    if raw.is_empty() {
        return None;
    }
    let mut cfg = CallConfig::default();
    for (key, val) in raw {
        match val {
            toml::Value::String(target) => {
                if key.contains('/') {
                    cfg.interface_overrides.insert(key.clone(), target.clone());
                } else {
                    cfg.trait_overrides.insert(key.clone(), target.clone());
                }
            }
            toml::Value::Table(tbl) => {
                if let Some(toml::Value::String(target)) = tbl.get("impl") {
                    if key.contains('/') {
                        cfg.interface_overrides.insert(key.clone(), target.clone());
                    } else {
                        cfg.trait_overrides.insert(key.clone(), target.clone());
                    }
                    let params: HashMap<String, TraitValue> = tbl.iter()
                        .filter(|(k, _)| k.as_str() != "impl")
                        .filter_map(|(k, v)| {
                            toml_value_to_trait_value(v).map(|tv| (k.clone(), tv))
                        })
                        .collect();
                    if !params.is_empty() {
                        cfg.load_params.insert(key.clone(), params);
                    }
                }
            }
            _ => {}
        }
    }
    Some(cfg)
}

// ── Registry ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Registry {
    traits: Arc<DashMap<String, TraitEntry>>,
    bindings: Arc<DashMap<String, String>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            traits: Arc::new(DashMap::new()),
            bindings: Arc::new(DashMap::new()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_dir(&self, dir: &Path) -> Result<usize, Box<dyn std::error::Error>> {
        // Clear existing entries so reload doesn't hit duplicate errors
        self.traits.clear();
        self.bindings.clear();

        let mut count = 0;
        if !dir.exists() {
            warn!("Traits directory does not exist: {:?}", dir);
            let builtin_count = self.load_builtin_traits()?;
            return Ok(builtin_count);
        }

        for entry in glob::glob(&format!("{}/**/*.trait.toml", dir.display()))? {
            match entry {
                Ok(path) => {
                    match self.load_trait_file(&path) {
                        Ok(()) => count += 1,
                        Err(e) => warn!("Failed to load {:?}: {}", path, e),
                    }
                }
                Err(e) => warn!("Glob error: {}", e),
            }
        }

        let builtin_count = self.load_builtin_traits()?;
        count += builtin_count;

        info!("Loaded {} traits from {:?}", count, dir);
        Ok(count)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_trait_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let toml: TraitToml = toml::from_str(&content)
            .map_err(|e| format!("Parse error in {:?}: {}", path, e))?;
        self.insert_trait_from_toml(toml, path.to_path_buf())
    }

    fn load_builtin_traits(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let mut count = 0;
        for def in BUILTIN_TRAIT_DEFS {
            if self.traits.contains_key(def.path) {
                continue;
            }
            let toml: TraitToml = toml::from_str(def.toml)
                .map_err(|e| format!("Parse error in builtin {}: {}", def.rel_path, e))?;
            let toml_path = PathBuf::from(def.rel_path);
            self.insert_trait_from_toml(toml, toml_path)?;
            count += 1;
        }
        Ok(count)
    }

    fn insert_trait_from_toml(
        &self,
        toml: TraitToml,
        toml_path: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let impl_toml = toml.implementation.as_ref();
        let cli_map = toml.cli_map.as_ref();
        let is_rest = impl_toml.map(|i| i.source == "rest").unwrap_or(false)
            || toml.trait_def.http.is_some();
        let language = resolve_language(&toml)
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

        let signature = build_signature(toml.signature.as_ref());

        let trait_path = derive_trait_path(&toml_path)
            .ok_or_else(|| format!("Cannot determine trait path from {:?}", toml_path))?;

        if self.traits.contains_key(&trait_path) {
            error!("Duplicate trait path: {} (from {:?})", trait_path, toml_path);
            return Err(format!("Duplicate trait path: {}", trait_path).into());
        }

        let source_path = if is_rest {
            PathBuf::from("rest")
        } else if let Some(imp) = impl_toml {
            toml_path.parent()
                .unwrap_or(Path::new("."))
                .join(&imp.source)
        } else if let Some(cm) = cli_map {
            toml_path.parent()
                .unwrap_or(Path::new("."))
                .join(&cm.source)
        } else {
            PathBuf::new()
        };

        let entry_name = if is_rest {
            "rest".to_string()
        } else if let Some(imp) = impl_toml {
            if imp.entry.is_empty() {
                trait_path.split('.').last().unwrap_or("main").to_string()
            } else {
                imp.entry.clone()
            }
        } else {
            String::new()
        };

        let cli_map_source = cli_map.map(|cm| {
            toml_path.parent()
                .unwrap_or(Path::new("."))
                .join(&cm.source)
        });

        let entry = TraitEntry {
            path: trait_path.clone(),
            description: toml.trait_def.description,
            version: toml.trait_def.version,
            author: toml.trait_def.author,
            tags: toml.trait_def.tags,
            published: toml.trait_def.published,
            imports: toml.trait_def.imports,
            gui: toml.trait_def.gui,
            frontend: toml.trait_def.frontend,
            startup_args: toml.trait_def.startup_args,
            signature,
            language,
            source: source_path,
            entry: entry_name,
            toml_path,
            stream: toml.trait_def.stream,
            background: toml.trait_def.background,
            kind: toml.trait_def.kind,
            command: toml.trait_def.command,
            codegen: toml.trait_def.codegen,
            sources: toml.trait_def.sources,
            http: toml.trait_def.http,
            requires: toml.requires.unwrap_or_default(),
            provides: toml.trait_def.provides,
            trait_bindings: toml.bindings.unwrap_or_default(),
            priority: toml.trait_def.priority,
            load: parse_load_config(&toml.load),
            cli_map_source,
            config: parse_config_section(toml.config),
        };

        self.traits.insert(trait_path, entry);
        Ok(())
    }

    pub fn get(&self, path: &str) -> Option<TraitEntry> {
        self.traits.get(path).map(|e| e.value().clone())
    }

    pub fn all(&self) -> Vec<TraitEntry> {
        self.traits.iter().map(|e| e.value().clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.traits.len()
    }

    pub fn implementations(&self, interface_path: &str) -> Vec<TraitEntry> {
        // Convert interface path from slash to dot notation for prefix matching
        let dot_path = interface_path.replace('/', ".");
        let prefix = format!("{}.", dot_path);
        self.traits
            .iter()
            .filter(|e| {
                // Check if trait explicitly provides this interface
                if e.value().provides.iter().any(|p| p == interface_path) {
                    return true;
                }
                // Fall back to convention: trait path starts with interface path
                if e.key().starts_with(&prefix) {
                    let suffix = &e.key()[prefix.len()..];
                    return !suffix.contains('.');
                }
                false
            })
            .map(|e| e.value().clone())
            .collect()
    }

    pub fn is_interface(&self, path: &str) -> bool {
        let dot_path = path.replace('/', ".");
        let prefix = format!("{}.", dot_path);
        // Check if any trait provides or is nested under this path
        self.traits.iter().any(|e| {
            if e.value().provides.iter().any(|p| p == path) {
                return true;
            }
            if e.key().starts_with(&prefix) {
                let suffix = &e.key()[prefix.len()..];
                return !suffix.contains('.');
            }
            false
        })
    }

    pub fn get_binding(&self, cap: &str) -> Option<String> {
        self.bindings.get(cap).map(|e| e.value().clone())
    }

    pub fn resolve_with_bindings(&self, path: &str, config: &CallConfig) -> Option<TraitEntry> {
        if let Some(override_path) = config.trait_overrides.get(path) {
            if let Some(entry) = self.get(override_path) {
                return Some(entry);
            }
        }
        if let Some(impl_path) = self.get_binding(path) {
            if let Some(entry) = self.get(&impl_path) {
                return Some(entry);
            }
            warn!("Binding {} → {} but implementation not found", path, impl_path);
        }
        let mut impls = self.implementations(path);
        if !impls.is_empty() {
            impls.sort_by(|a, b| b.priority.cmp(&a.priority));
            return Some(impls.into_iter().next().unwrap());
        }
        self.get(path)
    }

    /// Resolve an interface path to its implementation trait path.
    /// Resolution order:
    /// 1. Runtime overrides (CallConfig.interface_overrides)
    /// 2. Global bindings (registry.bindings)
    /// 3. Caller's local bindings (from the trait's [bindings] section)
    /// 4. Auto-discover: find the unique trait that provides this interface
    pub fn resolve_interface(
        &self,
        interface_path: &str,
        config: &CallConfig,
    ) -> Option<String> {
        // 1. Runtime override
        if let Some(target) = config.interface_overrides.get(interface_path) {
            return Some(target.clone());
        }
        // 2. Global bindings
        if let Some(target) = self.get_binding(interface_path) {
            return Some(target);
        }
        // 3. Auto-discover: find providers
        let providers = self.implementations(interface_path);
        if providers.len() == 1 {
            return Some(providers[0].path.clone());
        }
        if providers.len() > 1 {
            // Multiple providers — pick highest priority
            let mut sorted = providers;
            sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
            return Some(sorted[0].path.clone());
        }
        None
    }

    /// Resolve a keyed binding for a trait.
    /// Resolution: bindings[key] → requires[key] → interface auto-discover.
    pub fn resolve_keyed(&self, caller_path: &str, key: &str) -> Option<String> {
        let entry = self.get(caller_path)?;
        // 1. Check caller's bindings for this key
        if let Some(impl_path) = entry.trait_bindings.get(key) {
            return Some(impl_path.clone());
        }
        // 2. Fallback: resolve interface from requires[key]
        if let Some(interface_path) = entry.requires.get(key) {
            return self.resolve_interface(interface_path, &CallConfig::default());
        }
        None
    }

    /// Add a keyed interface requirement to a trait at runtime.
    pub fn add_require(&self, trait_path: &str, key: String, interface: String) -> bool {
        if let Some(mut entry) = self.traits.get_mut(trait_path) {
            entry.requires.insert(key, interface);
            true
        } else {
            false
        }
    }

    /// Add a keyed binding to a trait at runtime.
    pub fn add_binding_for(&self, trait_path: &str, key: String, impl_path: String) -> bool {
        if let Some(mut entry) = self.traits.get_mut(trait_path) {
            entry.trait_bindings.insert(key, impl_path);
            true
        } else {
            false
        }
    }

    /// Set a global binding: interface_path → impl_path.
    /// Used by sys.bindings to hot-swap implementations at runtime.
    pub fn set_binding(&self, interface: &str, impl_path: &str) {
        self.bindings.insert(interface.to_string(), impl_path.to_string());
    }

    /// Remove a global binding.
    pub fn remove_binding(&self, interface: &str) -> bool {
        self.bindings.remove(interface).is_some()
    }

    /// Return all global bindings as a HashMap.
    pub fn all_bindings(&self) -> HashMap<String, String> {
        self.bindings.iter().map(|e| (e.key().clone(), e.value().clone())).collect()
    }

}

// ── Trait dispatch entry point ──

/// kernel.registry introspection: returns registry stats and trait listing.
pub fn registry(args: &[serde_json::Value]) -> serde_json::Value {
    let reg = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return serde_json::json!({"error": "registry not initialized"}),
    };
    // Optional filter: first arg is a glob/prefix pattern
    let filter = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let all = reg.all();
    let filtered: Vec<serde_json::Value> = if filter.is_empty() {
        all.iter().map(|e| e.to_summary_json()).collect()
    } else {
        all.iter()
            .filter(|e| e.path.starts_with(filter) || e.path.contains(filter))
            .map(|e| e.to_summary_json())
            .collect()
    };
    serde_json::json!({
        "trait_count": all.len(),
        "filtered_count": filtered.len(),
        "traits": filtered
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basic() {
        let reg = Registry::new();
        assert_eq!(reg.len(), 0);
        assert!(reg.get("math.fibonacci").is_none());
    }
}
