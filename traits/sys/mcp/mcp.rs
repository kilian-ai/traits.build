use serde_json::{json, Value, Map};
use std::io::{self, BufRead, Write};

/// sys.mcp — MCP stdio server.
///
/// Implements the Model Context Protocol (MCP) over stdin/stdout using JSON-RPC 2.0.
/// Every registered trait is exposed as an MCP tool. Tool names use underscore
/// notation (e.g. "sys_checksum") mapped from dot notation ("sys.checksum").
///
/// Protocol:  https://modelcontextprotocol.io/specification/2024-11-05
/// Transport: stdio (one JSON-RPC message per line)
///
/// Called by: `traits mcp` (CLI special-case, does NOT go through normal dispatch)
pub fn mcp(_args: &[Value]) -> Value {
    // Should not be called via dispatch — the CLI intercepts "mcp" and calls run_stdio() directly.
    json!({ "error": "sys.mcp is a stdio server — run `traits mcp` from the command line" })
}

// ────────────────── MCP stdio loop ──────────────────

/// Run the MCP stdio server. Blocks until stdin is closed.
/// All logging goes to stderr; only JSON-RPC responses go to stdout.
pub fn run_stdio() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(response) = handle_message(trimmed) {
            write_response(&mut out, &response);
        }
    }
}

/// Process a single JSON-RPC message and return an optional response.
/// Returns None for notifications (no "id" field) and empty lines.
/// This is the shared handler used by both stdio and WebSocket transports.
pub fn handle_message(message: &str) -> Option<Value> {
    let request: Value = match serde_json::from_str(message) {
        Ok(v) => v,
        Err(e) => {
            return Some(json_rpc_error(Value::Null, -32700, &format!("Parse error: {}", e)));
        }
    };

    // Notifications (no "id") — acknowledge silently
    if request.get("id").is_none() {
        return None;
    }

    let id = request["id"].clone();
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

    Some(match method {
        "initialize" => handle_initialize(id, &request),
        "tools/list" => handle_tools_list(id),
        "tools/call" => handle_tools_call(id, &request),
        "ping" => json_rpc_result(id, json!({})),
        _ => json_rpc_error(id, -32601, &format!("Method not found: {}", method)),
    })
}

// ────────────────── MCP method handlers ──────────────────

fn handle_initialize(id: Value, request: &Value) -> Value {
    let _client_info = request.pointer("/params/clientInfo");
    let _protocol = request.pointer("/params/protocolVersion");

    json_rpc_result(id, json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "traits-mcp-server",
            "version": env!("TRAITS_BUILD_VERSION")
        }
    }))
}

fn handle_tools_list(id: Value) -> Value {
    let registry = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return json_rpc_error(id, -32603, "Registry not initialized"),
    };

    let mut tools: Vec<Value> = Vec::new();
    let mut entries = registry.all();
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    for entry in &entries {
        // Skip non-callable / internal traits
        if entry.path == "sys.mcp" || entry.path == "kernel.main" {
            continue;
        }

        let tool_name = entry.path.replace('.', "_");
        let schema = build_input_schema(&entry.signature);

        tools.push(json!({
            "name": tool_name,
            "description": entry.description,
            "inputSchema": schema
        }));
    }

    json_rpc_result(id, json!({ "tools": tools }))
}

fn handle_tools_call(id: Value, request: &Value) -> Value {
    let params = match request.get("params") {
        Some(p) => p,
        None => return json_rpc_error(id, -32602, "Missing params"),
    };

    let tool_name = match params.get("name").and_then(|n| n.as_str()) {
        Some(n) => n,
        None => return json_rpc_error(id, -32602, "Missing tool name"),
    };

    // Convert underscore tool name back to dot path
    let trait_path = tool_name.replace('_', ".");

    // Verify the trait exists
    let registry = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return json_rpc_error(id, -32603, "Registry not initialized"),
    };

    let entry = match registry.get(&trait_path) {
        Some(e) => e,
        None => return json_rpc_error(id, -32602, &format!("Unknown tool: {} (trait: {})", tool_name, trait_path)),
    };

    // Build argument array from the arguments object, ordered by param signature
    let arguments = params.get("arguments").and_then(|a| a.as_object());
    let args = build_args_from_schema(&entry.signature, arguments);

    // Dispatch to compiled trait implementation
    match crate::dispatcher::compiled::dispatch(&trait_path, &args) {
        Some(result) => {
            let text = match &result {
                Value::String(s) => s.clone(),
                other => serde_json::to_string_pretty(other).unwrap_or_default(),
            };
            json_rpc_result(id, json!({
                "content": [{
                    "type": "text",
                    "text": text
                }]
            }))
        }
        None => json_rpc_error(id, -32602, &format!("Dispatch failed for trait: {}", trait_path)),
    }
}

// ────────────────── schema helpers ──────────────────

/// Build a JSON Schema object from a trait's parameter signature.
fn build_input_schema(sig: &crate::types::TraitSignature) -> Value {
    let mut properties = Map::new();
    let mut required: Vec<Value> = Vec::new();

    for param in &sig.params {
        let mut prop = match trait_type_to_json_schema(&param.param_type) {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        if !param.description.is_empty() {
            prop.insert("description".to_string(), json!(param.description));
        }
        properties.insert(param.name.clone(), Value::Object(prop));
        if !param.optional {
            required.push(json!(param.name));
        }
    }

    let mut schema = Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_string(), Value::Array(required));
    }
    Value::Object(schema)
}

/// Map TraitType to a JSON Schema value (not just a type string).
fn trait_type_to_json_schema(tt: &crate::types::TraitType) -> Value {
    match tt {
        crate::types::TraitType::Int => json!({"type": "integer"}),
        crate::types::TraitType::Float => json!({"type": "number"}),
        crate::types::TraitType::String => json!({"type": "string"}),
        crate::types::TraitType::Bool => json!({"type": "boolean"}),
        crate::types::TraitType::Bytes => json!({"type": "string"}),
        crate::types::TraitType::List(inner) => json!({
            "type": "array",
            "items": trait_type_to_json_schema(inner)
        }),
        crate::types::TraitType::Map(_k, v) => json!({
            "type": "object",
            "additionalProperties": trait_type_to_json_schema(v)
        }),
        crate::types::TraitType::Optional(inner) => trait_type_to_json_schema(inner),
        crate::types::TraitType::Any => json!({"type": "string"}),
        crate::types::TraitType::Handle => json!({"type": "string"}),
        crate::types::TraitType::Null => json!({"type": "string"}),
    }
}

/// Build an ordered arg array from the MCP arguments object, matching param signature order.
fn build_args_from_schema(
    sig: &crate::types::TraitSignature,
    arguments: Option<&Map<String, Value>>,
) -> Vec<Value> {
    sig.params.iter().map(|param| {
        arguments
            .and_then(|args| args.get(&param.name))
            .cloned()
            .unwrap_or(Value::Null)
    }).collect()
}

// ────────────────── JSON-RPC helpers ──────────────────

fn json_rpc_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn json_rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn write_response(out: &mut impl Write, response: &Value) {
    let bytes = serde_json::to_vec(response).unwrap_or_default();
    let _ = out.write_all(&bytes);
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}
