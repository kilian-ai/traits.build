// Re-export shared types from kernel-logic
#[cfg_attr(target_arch = "wasm32", allow(unused_imports))]
pub use kernel_logic::types::{Language, ParamDef, ReturnDef, TraitSignature, TraitType};
pub use kernel_logic::types::TraitValue;

use serde::{Deserialize, Serialize};

/// Wire protocol: request from router to worker
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerRequest {
    pub call: String,
    pub args: Vec<TraitValue>,
    pub id: String,
    /// When true, the worker should send __chunk__ messages instead of a single result
    #[serde(default, skip_serializing_if = "is_false")]
    pub stream: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Wire protocol: response from worker to router
///
/// Regular response:   { id, result?, error? }
/// Streaming chunk:    { id, __chunk__: <value> }
/// Stream end:         { id, __stream_end__: true }
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WorkerResponse {
    #[serde(default)]
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TraitValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "__chunk__")]
    pub chunk: Option<TraitValue>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "__stream_end__")]
    pub stream_end: Option<bool>,
}

/// HTTP API: call request
#[derive(Debug, Serialize, Deserialize)]
pub struct CallRequest {
    /// Positional args (array) or named args (object with param names as keys).
    /// Named args accept both underscore and hyphen forms, e.g. "telegram_token" or "telegram-token".
    #[serde(default)]
    pub args: serde_json::Value,
    /// Optional per-call interface overrides: { "llm/prompt": "net.openai" }
    /// Used when code calls `interface:llm/prompt` — picks which implementation runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_overrides: Option<std::collections::HashMap<String, String>>,
    /// Optional per-call trait overrides: { "net.copilot_chat": "net.openai" }
    /// Redirects calls to one trait to a different trait implementation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trait_overrides: Option<std::collections::HashMap<String, String>>,
}

/// HTTP API: call response
#[derive(Debug, Serialize, Deserialize)]
pub struct CallResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ── Trait dispatch entry point ──

/// kernel.types introspection: returns type system info.
pub fn types(args: &[serde_json::Value]) -> serde_json::Value {
    let _ = args;
    serde_json::json!({
        "trait_types": ["int", "float", "string", "bool", "bytes", "null", "list", "map", "optional", "any", "handle"],
        "languages": ["rust", "python", "javascript", "typescript", "java", "perl", "lisp"],
        "wire_protocol": {
            "request": "WorkerRequest { call, args, id, stream }",
            "response": "WorkerResponse { id, result?, error?, __chunk__?, __stream_end__? }"
        },
        "http_api": {
            "request": "CallRequest { args, interface_overrides?, trait_overrides? }",
            "response": "CallResponse { result?, error? }"
        }
    })
}
