use crate::dispatcher::CallConfig;
use crate::types::{Language, TraitSignature, TraitType, TraitValue, ParamDef, ReturnDef};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn, error};

// ── Builtin trait definitions (embedded by build.rs) ──
#[derive(Debug, Clone, Copy)]
pub struct BuiltinTraitDef {
    pub path: &'static str,
    pub rel_path: &'static str,
    pub toml: &'static str,
}
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

/// Raw TOML structure for .trait.toml files
#[derive(Debug, Deserialize)]
struct TraitToml {
    #[serde(alias = "strait", rename = "trait")]
    trait_def: TraitDefToml,
    #[serde(default)]
    signature: Option<SignatureToml>,
    #[serde(default)]
    implementation: Option<ImplementationToml>,
    #[serde(default)]
    cli_map: Option<CliMapToml>,
    #[serde(default)]
    load: Option<HashMap<String, toml::Value>>,
    #[serde(default)]
    bindings: Option<HashMap<String, String>>,
    #[serde(default)]
    wasm_bindings: Option<HashMap<String, String>>,
    #[serde(default)]
    requires: Option<HashMap<String, String>>,
    #[serde(default)]
    config: Option<HashMap<String, toml::Value>>,
}

#[derive(Debug, Deserialize)]
struct CliMapToml {
    source: String,
    language: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct HttpTraitConfig {
    #[serde(default = "default_http_method")]
    pub method: String,
    pub url: String,
    #[serde(default = "default_http_response")]
    pub response: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub query: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub auth_secret: Option<String>,
    #[serde(default)]
    pub response_path: Option<String>,
    #[serde(default)]
    pub defaults: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct TraitDefToml {
    #[serde(default)]
    description: String,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    published: Option<String>,
    #[serde(default)]
    imports: Vec<String>,
    #[serde(default)]
    gui: Option<String>,
    #[serde(default)]
    frontend: Option<String>,
    #[serde(default)]
    startup_args: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    background: bool,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    codegen: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    sources: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    http: Option<HttpTraitConfig>,
    #[serde(default)]
    provides: Vec<String>,
    #[serde(default)]
    priority: i32,
}

#[derive(Debug, Deserialize)]
struct SignatureToml {
    #[serde(default)]
    params: Vec<ParamToml>,
    returns: ReturnToml,
}

#[derive(Debug, Deserialize)]
struct ParamToml {
    name: String,
    #[serde(rename = "type")]
    param_type: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    optional: bool,
    /// When `required = false` is specified, treat as optional.
    #[serde(default = "default_required")]
    required: bool,
    #[serde(default)]
    pipe: bool,
}

fn default_required() -> bool { true }

impl ParamToml {
    /// A param is optional if `optional = true` OR `required = false`.
    fn is_optional(&self) -> bool {
        self.optional || !self.required
    }
}

#[derive(Debug, Deserialize)]
struct ReturnToml {
    #[serde(rename = "type")]
    return_type: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
struct ImplementationToml {
    language: String,
    source: String,
    #[serde(default)]
    entry: String,
}

fn default_version() -> String {
    "v260322".into()
}

fn default_http_method() -> String {
    "GET".into()
}

fn default_http_response() -> String {
    "json".into()
}

/// Parse a type string like "int", "list<string>", "map<string, int>" into TraitType
pub fn parse_type(s: &str) -> TraitType {
    let s = s.trim();
    if s.ends_with('?') {
        return TraitType::Optional(Box::new(parse_type(&s[..s.len() - 1])));
    }
    let lower = s.to_lowercase();
    let s_lower = lower.as_str();
    match s_lower {
        "int" | "integer" => TraitType::Int,
        "float" | "double" | "number" => TraitType::Float,
        "string" | "str" => TraitType::String,
        "bool" | "boolean" => TraitType::Bool,
        "bytes" => TraitType::Bytes,
        "null" | "none" | "void" => TraitType::Null,
        "any" => TraitType::Any,
        "handle" => TraitType::Handle,
        s_lower if s_lower.starts_with("list<") && s_lower.ends_with('>') => {
            let inner = &s_lower[5..s_lower.len() - 1];
            TraitType::List(Box::new(parse_type(inner)))
        }
        s_lower if s_lower.starts_with("map<") && s_lower.ends_with('>') => {
            let inner = &s_lower[4..s_lower.len() - 1];
            if let Some(comma_pos) = inner.find(',') {
                let k = &inner[..comma_pos];
                let v = &inner[comma_pos + 1..];
                TraitType::Map(Box::new(parse_type(k)), Box::new(parse_type(v)))
            } else {
                TraitType::Map(Box::new(TraitType::String), Box::new(TraitType::Any))
            }
        }
        _ => TraitType::Any,
    }
}

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

fn toml_value_to_trait_value(v: &toml::Value) -> Option<TraitValue> {
    match v {
        toml::Value::String(s) => Some(TraitValue::String(s.clone())),
        toml::Value::Integer(n) => Some(TraitValue::Int(*n)),
        toml::Value::Float(f) => Some(TraitValue::Float(*f)),
        toml::Value::Boolean(b) => Some(TraitValue::Bool(*b)),
        toml::Value::Array(arr) => {
            let items: Vec<TraitValue> = arr.iter()
                .filter_map(toml_value_to_trait_value)
                .collect();
            Some(TraitValue::List(items))
        }
        toml::Value::Table(tbl) => {
            let entries: HashMap<String, TraitValue> = tbl.iter()
                .filter_map(|(k, v)| toml_value_to_trait_value(v).map(|tv| (k.clone(), tv)))
                .collect();
            Some(TraitValue::Map(entries))
        }
        _ => None,
    }
}

pub fn parse_language(s: &str) -> Option<Language> {
    match s.to_lowercase().as_str() {
        "rust" => Some(Language::Rust),
        "python" => Some(Language::Python),
        "javascript" | "js" => Some(Language::JavaScript),
        "typescript" | "ts" => Some(Language::TypeScript),
        "java" => Some(Language::Java),
        "perl" => Some(Language::Perl),
        "lisp" | "commonlisp" | "common-lisp" | "cl" => Some(Language::Lisp),
        _ => None,
    }
}

fn derive_trait_path(toml_path: &Path) -> Option<String> {
    let path_str = toml_path.to_string_lossy();
    let markers: &[&str] = &["traits/", "traits\\", "impl/", "impl\\"];
    let (idx, marker_len) = markers.iter()
        .filter_map(|m| path_str.find(m).map(|i| (i, m.len())))
        .next()?;
    let rel = &path_str[idx + marker_len..];
    let stem = rel.strip_suffix(".trait.toml")
        .or_else(|| rel.strip_suffix(".strait.toml"))?;
    let mut result = stem.replace('/', ".").replace('\\', ".");
    // Collapse trailing duplicate: sys.checksum.checksum -> sys.checksum
    let parts: Vec<&str> = result.split('.').collect();
    if parts.len() >= 2 && parts[parts.len() - 1] == parts[parts.len() - 2] {
        result = parts[..parts.len() - 1].join(".");
    }
    Some(result)
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
            let toml_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(def.rel_path);
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
        let language = if is_rest {
            Language::Rust
        } else if let Some(imp) = impl_toml {
            parse_language(&imp.language)
                .ok_or_else(|| format!("Unknown language: {}", imp.language))?
        } else if toml.trait_def.command.is_some() {
            if let Some(cm) = cli_map {
                parse_language(&cm.language)
                    .ok_or_else(|| format!("Unknown cli_map language: {}", cm.language))?
            } else {
                Language::Rust
            }
        } else {
            return Err("Missing [implementation] section (required unless command= is set)".into());
        };

        let params: Vec<ParamDef> = if let Some(ref sig) = toml.signature {
            sig.params.iter().map(|p| {
                ParamDef {
                    name: p.name.clone(),
                    param_type: parse_type(&p.param_type),
                    description: p.description.clone(),
                    optional: p.is_optional(),
                    pipe: p.pipe,
                }
            }).collect()
        } else {
            vec![]
        };

        let returns = if let Some(ref sig) = toml.signature {
            ReturnDef {
                return_type: parse_type(&sig.returns.return_type),
                description: sig.returns.description.clone(),
            }
        } else {
            ReturnDef {
                return_type: TraitType::Any,
                description: String::new(),
            }
        };

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
            signature: TraitSignature { params, returns },
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
            config: toml.config.map(|m| {
                m.into_iter().map(|(k, v)| {
                    let s = match &v {
                        toml::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    (k, s)
                }).collect()
            }).unwrap_or_default(),
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
    fn test_parse_type() {
        assert_eq!(parse_type("int"), TraitType::Int);
        assert_eq!(parse_type("float"), TraitType::Float);
        assert_eq!(parse_type("string"), TraitType::String);
        assert_eq!(parse_type("bool"), TraitType::Bool);
        assert_eq!(parse_type("list<int>"), TraitType::List(Box::new(TraitType::Int)));
        assert_eq!(
            parse_type("map<string, int>"),
            TraitType::Map(Box::new(TraitType::String), Box::new(TraitType::Int))
        );
        assert_eq!(
            parse_type("int?"),
            TraitType::Optional(Box::new(TraitType::Int))
        );
    }

    #[test]
    fn test_parse_language() {
        assert_eq!(parse_language("rust"), Some(Language::Rust));
        assert_eq!(parse_language("Python"), Some(Language::Python));
        assert_eq!(parse_language("JS"), Some(Language::JavaScript));
        assert_eq!(parse_language("ts"), Some(Language::TypeScript));
        assert_eq!(parse_language("java"), Some(Language::Java));
        assert_eq!(parse_language("perl"), Some(Language::Perl));
        assert_eq!(parse_language("unknown"), None);
    }

    #[test]
    fn test_registry_basic() {
        let reg = Registry::new();
        assert_eq!(reg.len(), 0);
        assert!(reg.get("math.fibonacci").is_none());
    }
}
