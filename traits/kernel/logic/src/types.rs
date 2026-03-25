use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cross-language type system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TraitType {
    Int,
    Float,
    String,
    Bool,
    Bytes,
    Null,
    #[serde(rename = "list")]
    List(Box<TraitType>),
    #[serde(rename = "map")]
    Map(Box<TraitType>, Box<TraitType>),
    /// Nullable wrapper
    #[serde(rename = "optional")]
    Optional(Box<TraitType>),
    /// Untyped / dynamic
    Any,
    /// Opaque handle to a non-serializable object
    Handle,
}

/// A runtime value that can be passed across language boundaries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TraitValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<TraitValue>),
    Map(HashMap<String, TraitValue>),
    Bytes(Vec<u8>),
}

impl TraitValue {
    /// Convert TraitValue to serde_json::Value
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            TraitValue::Null => serde_json::Value::Null,
            TraitValue::Bool(b) => serde_json::Value::Bool(*b),
            TraitValue::Int(n) => serde_json::Value::from(*n),
            TraitValue::Float(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            TraitValue::String(s) => serde_json::Value::String(s.clone()),
            TraitValue::List(arr) => serde_json::Value::Array(arr.iter().map(|v| v.to_json()).collect()),
            TraitValue::Map(map) => {
                let obj: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json()))
                    .collect();
                serde_json::Value::Object(obj)
            }
            TraitValue::Bytes(b) => {
                serde_json::Value::String(b.iter().map(|byte| format!("{:02x}", byte)).collect())
            }
        }
    }

    /// Convert serde_json::Value to TraitValue
    pub fn from_json(val: &serde_json::Value) -> Self {
        match val {
            serde_json::Value::Null => TraitValue::Null,
            serde_json::Value::Bool(b) => TraitValue::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    TraitValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    TraitValue::Float(f)
                } else {
                    TraitValue::Null
                }
            }
            serde_json::Value::String(s) => TraitValue::String(s.clone()),
            serde_json::Value::Array(arr) => TraitValue::List(arr.iter().map(TraitValue::from_json).collect()),
            serde_json::Value::Object(map) => {
                let hm: HashMap<String, TraitValue> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), TraitValue::from_json(v)))
                    .collect();
                TraitValue::Map(hm)
            }
        }
    }

    /// Check if this value is a handle reference (Map with __handle__ key)
    pub fn is_handle(&self) -> bool {
        matches!(self, TraitValue::Map(m) if m.contains_key("__handle__"))
    }

    /// Extract the handle ID string if this is a handle
    pub fn handle_id(&self) -> Option<&str> {
        match self {
            TraitValue::Map(m) => match m.get("__handle__") {
                Some(TraitValue::String(id)) => Some(id.as_str()),
                _ => None,
            },
            TraitValue::String(s) if s.starts_with("hdl:") => Some(s.as_str()),
            _ => None,
        }
    }

    /// Extract the language prefix from a handle ID (e.g. "py" from "hdl:py:abc123")
    pub fn handle_language(&self) -> Option<&str> {
        self.handle_id().and_then(|id| {
            let parts: Vec<&str> = id.splitn(3, ':').collect();
            if parts.len() == 3 && parts[0] == "hdl" {
                Some(parts[1])
            } else {
                None
            }
        })
    }

    /// Check if a value matches a declared type
    pub fn matches_type(&self, expected: &TraitType) -> bool {
        match (self, expected) {
            (_, TraitType::Any) => true,
            (val, TraitType::Handle) if val.is_handle() => true,
            (val, _) if val.is_handle() => matches!(expected, TraitType::Any | TraitType::Handle),
            (TraitValue::Null, TraitType::Optional(_)) => true,
            (TraitValue::Null, TraitType::Null) => true,
            (val, TraitType::Optional(inner)) => val.matches_type(inner),
            (TraitValue::Bool(_), TraitType::Bool) => true,
            (TraitValue::Int(_), TraitType::Int) => true,
            (TraitValue::Float(_), TraitType::Float) => true,
            (TraitValue::Int(_), TraitType::Float) => true,
            (TraitValue::String(_), TraitType::String) => true,
            (TraitValue::Bytes(_), TraitType::Bytes) => true,
            (TraitValue::String(_), TraitType::List(_)) => true,
            (TraitValue::List(items), TraitType::List(inner)) => {
                items.iter().all(|item| item.matches_type(inner))
            }
            (TraitValue::Map(entries), TraitType::Map(_, v_type)) => {
                entries.values().all(|val| val.matches_type(v_type))
            }
            _ => false,
        }
    }

    /// Get a human-readable type name for error messages
    pub fn type_name(&self) -> &'static str {
        if self.is_handle() {
            return "handle";
        }
        match self {
            TraitValue::Null => "null",
            TraitValue::Bool(_) => "bool",
            TraitValue::Int(_) => "int",
            TraitValue::Float(_) => "float",
            TraitValue::String(_) => "string",
            TraitValue::Bytes(_) => "bytes",
            TraitValue::List(_) => "list",
            TraitValue::Map(_) => "map",
        }
    }
}

/// Parameter definition in a trait signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: TraitType,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub optional: bool,
    /// When true, this parameter accepts piped stdin input if not provided as a CLI arg.
    #[serde(default)]
    pub pipe: bool,
    /// Example value for documentation / OpenAPI spec generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
}

/// Return type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnDef {
    #[serde(rename = "type")]
    pub return_type: TraitType,
    #[serde(default)]
    pub description: String,
}

/// A trait's full signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitSignature {
    pub params: Vec<ParamDef>,
    pub returns: ReturnDef,
}

impl std::fmt::Display for TraitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TraitType::Int => write!(f, "int"),
            TraitType::Float => write!(f, "float"),
            TraitType::String => write!(f, "string"),
            TraitType::Bool => write!(f, "bool"),
            TraitType::Bytes => write!(f, "bytes"),
            TraitType::Null => write!(f, "null"),
            TraitType::Any => write!(f, "any"),
            TraitType::Handle => write!(f, "handle"),
            TraitType::List(inner) => write!(f, "list<{}>", inner),
            TraitType::Map(k, v) => write!(f, "map<{}, {}>", k, v),
            TraitType::Optional(inner) => write!(f, "{}?", inner),
        }
    }
}

/// Supported implementation languages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    Perl,
    Lisp,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "rust"),
            Language::Python => write!(f, "python"),
            Language::JavaScript => write!(f, "javascript"),
            Language::TypeScript => write!(f, "typescript"),
            Language::Java => write!(f, "java"),
            Language::Perl => write!(f, "perl"),
            Language::Lisp => write!(f, "lisp"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_matching() {
        assert!(TraitValue::Int(42).matches_type(&TraitType::Int));
        assert!(TraitValue::Int(42).matches_type(&TraitType::Float));
        assert!(TraitValue::Int(42).matches_type(&TraitType::Any));
        assert!(!TraitValue::Int(42).matches_type(&TraitType::String));
        assert!(TraitValue::String("hello".into()).matches_type(&TraitType::String));
        assert!(!TraitValue::String("hello".into()).matches_type(&TraitType::Int));
        assert!(TraitValue::Null.matches_type(&TraitType::Optional(Box::new(TraitType::Int))));
        assert!(TraitValue::Int(42).matches_type(&TraitType::Optional(Box::new(TraitType::Int))));
        let list = TraitValue::List(vec![TraitValue::Int(1), TraitValue::Int(2)]);
        assert!(list.matches_type(&TraitType::List(Box::new(TraitType::Int))));
        assert!(!list.matches_type(&TraitType::List(Box::new(TraitType::String))));
    }

    #[test]
    fn test_type_name() {
        assert_eq!(TraitValue::Int(0).type_name(), "int");
        assert_eq!(TraitValue::String("".into()).type_name(), "string");
        assert_eq!(TraitValue::Null.type_name(), "null");
        assert_eq!(TraitValue::List(vec![]).type_name(), "list");
    }
}
