use crate::types::{Language, TraitType, TraitValue, ParamDef, ReturnDef, TraitSignature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ── Builtin trait definitions (embedded by build.rs) ──

/// A builtin trait definition embedded at compile time.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinTraitDef {
    pub path: &'static str,
    pub rel_path: &'static str,
    pub toml: &'static str,
}

// ── TOML deserialization structs ──

/// Raw TOML structure for .trait.toml files
#[derive(Debug, Deserialize)]
pub struct TraitToml {
    #[serde(alias = "strait", rename = "trait")]
    pub trait_def: TraitDefToml,
    #[serde(default)]
    pub signature: Option<SignatureToml>,
    #[serde(default)]
    pub implementation: Option<ImplementationToml>,
    #[serde(default)]
    pub cli_map: Option<CliMapToml>,
    #[serde(default)]
    pub load: Option<HashMap<String, toml::Value>>,
    #[serde(default)]
    pub bindings: Option<HashMap<String, String>>,
    #[serde(default)]
    pub requires: Option<HashMap<String, String>>,
    #[serde(default)]
    pub config: Option<HashMap<String, toml::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct CliMapToml {
    pub source: String,
    pub language: String,
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
pub struct TraitDefToml {
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub published: Option<String>,
    #[serde(default)]
    pub imports: Vec<String>,
    #[serde(default)]
    pub gui: Option<String>,
    #[serde(default)]
    pub frontend: Option<String>,
    #[serde(default)]
    pub startup_args: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub background: bool,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub codegen: Option<HashMap<String, String>>,
    #[serde(default)]
    pub sources: Option<HashMap<String, String>>,
    #[serde(default)]
    pub http: Option<HttpTraitConfig>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Deserialize)]
pub struct SignatureToml {
    #[serde(default)]
    pub params: Vec<ParamToml>,
    pub returns: ReturnToml,
}

#[derive(Debug, Deserialize)]
pub struct ParamToml {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub optional: bool,
    /// When `required = false` is specified, treat as optional.
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default)]
    pub pipe: bool,
    #[serde(default)]
    pub example: Option<toml::Value>,
}

fn default_required() -> bool { true }

impl ParamToml {
    /// A param is optional if `optional = true` OR `required = false`.
    pub fn is_optional(&self) -> bool {
        self.optional || !self.required
    }
}

#[derive(Debug, Deserialize)]
pub struct ReturnToml {
    #[serde(rename = "type")]
    pub return_type: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct ImplementationToml {
    pub language: String,
    pub source: String,
    #[serde(default)]
    pub entry: String,
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

// ── Pure parsing functions ──

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

/// Convert a TOML value to TraitValue
pub fn toml_value_to_trait_value(v: &toml::Value) -> Option<TraitValue> {
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

/// Convert a TOML value to a serde_json::Value for OpenAPI examples.
pub fn toml_to_json(v: &toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(n) => serde_json::json!(*n),
        toml::Value::Float(f) => serde_json::json!(*f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Array(arr) => serde_json::Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(tbl) => {
            let map: serde_json::Map<String, serde_json::Value> = tbl.iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        _ => serde_json::Value::Null,
    }
}

/// Parse a language string into a Language enum variant.
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

/// Derive a trait dot-path from its .trait.toml file path.
/// e.g. "traits/sys/checksum/checksum.trait.toml" → "sys.checksum"
pub fn derive_trait_path(toml_path: &Path) -> Option<String> {
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

// ── Signature construction helpers ──

/// Build a Vec<ParamDef> from parsed TOML signature params.
pub fn build_params(sig: &SignatureToml) -> Vec<ParamDef> {
    sig.params.iter().map(|p| {
        ParamDef {
            name: p.name.clone(),
            param_type: parse_type(&p.param_type),
            description: p.description.clone(),
            optional: p.is_optional(),
            pipe: p.pipe,
            example: p.example.as_ref().map(toml_to_json),
        }
    }).collect()
}

/// Build a ReturnDef from parsed TOML signature returns.
pub fn build_returns(sig: &SignatureToml) -> ReturnDef {
    ReturnDef {
        return_type: parse_type(&sig.returns.return_type),
        description: sig.returns.description.clone(),
    }
}

/// Build a full TraitSignature from an optional SignatureToml.
pub fn build_signature(sig: Option<&SignatureToml>) -> TraitSignature {
    match sig {
        Some(s) => TraitSignature {
            params: build_params(s),
            returns: build_returns(s),
        },
        None => TraitSignature {
            params: vec![],
            returns: ReturnDef {
                return_type: TraitType::Any,
                description: String::new(),
            },
        },
    }
}

/// Determine the Language from a TraitToml. Returns Err if language is unknown.
pub fn resolve_language(toml: &TraitToml) -> Result<Language, String> {
    let impl_toml = toml.implementation.as_ref();
    let is_rest = impl_toml.map(|i| i.source == "rest").unwrap_or(false)
        || toml.trait_def.http.is_some();

    if is_rest {
        return Ok(Language::Rust);
    }
    if let Some(imp) = impl_toml {
        return parse_language(&imp.language)
            .ok_or_else(|| format!("Unknown language: {}", imp.language));
    }
    if toml.trait_def.command.is_some() {
        if let Some(cm) = toml.cli_map.as_ref() {
            return parse_language(&cm.language)
                .ok_or_else(|| format!("Unknown cli_map language: {}", cm.language));
        }
        return Ok(Language::Rust);
    }
    Err("Missing [implementation] section (required unless command= is set)".into())
}

/// Parse a [config] section from TOML into a flat HashMap<String, String>.
pub fn parse_config_section(config: Option<HashMap<String, toml::Value>>) -> HashMap<String, String> {
    config.map(|m| {
        m.into_iter().map(|(k, v)| {
            let s = match &v {
                toml::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            (k, s)
        }).collect()
    }).unwrap_or_default()
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
    fn test_derive_trait_path() {
        let p = Path::new("traits/sys/checksum/checksum.trait.toml");
        assert_eq!(derive_trait_path(p), Some("sys.checksum".to_string()));

        let p = Path::new("traits/www/traits/build/build.trait.toml");
        assert_eq!(derive_trait_path(p), Some("www.traits.build".to_string()));

        let p = Path::new("traits/kernel/main/main.trait.toml");
        assert_eq!(derive_trait_path(p), Some("kernel.main".to_string()));
    }

    #[test]
    fn test_toml_to_json() {
        assert_eq!(toml_to_json(&toml::Value::String("hi".into())), serde_json::json!("hi"));
        assert_eq!(toml_to_json(&toml::Value::Integer(42)), serde_json::json!(42));
        assert_eq!(toml_to_json(&toml::Value::Boolean(true)), serde_json::json!(true));
    }
}
