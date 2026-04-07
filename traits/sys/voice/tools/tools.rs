use serde_json::{json, Map, Value};

/// Single-source-of-truth exclusion list: traits that must never be exposed
/// as voice function-calling tools. Used by both native WebSocket and browser
/// WebRTC voice sessions.
pub const VOICE_TOOL_EXCLUDE: &[&str] = &[
    "sys.voice",
    "sys.voice.config",
    "sys.voice.instruct",
    "sys.voice.memory",
    "sys.voice.status",
    "sys.voice.tools",
    "sys.mcp",
    "sys.serve",
    "sys.cli",
    "sys.cli.native",
    "sys.cli.wasm",
    "sys.dylib_loader",
    "sys.reload",
    "sys.release",
    "sys.secrets",
    "sys.canvas",
    "sys.vfs",
    "llm.agent",
    "kernel.main",
    "kernel.dispatcher",
    "kernel.globals",
    "kernel.registry",
    "kernel.config",
    "kernel.plugin_api",
    "kernel.cli",
    "www.admin",
    "www.admin.deploy",
    "www.admin.fast_deploy",
    "www.admin.scale",
    "www.admin.destroy",
    "www.admin.save_config",
];

/// sys.voice.tools — build OpenAI Realtime API tool definitions from the
/// trait registry.
///
/// Returns a JSON array of tool objects ready to send in a session.update.
/// Both the native WebSocket voice session (voice.rs) and the browser WebRTC
/// session (traits.js) delegate to this trait, so the tool list, exclusions,
/// and synthetic tools stay in sync automatically.
///
/// Args: [page?]  — "canvas" restricts to canvas-focused tools only
pub fn voice_tools(args: &[Value]) -> Value {
    let page = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let is_canvas = page == "canvas";

    let all_traits = kernel_logic::platform::registry_all();
    let mut tools: Vec<Value> = Vec::new();

    if !is_canvas {
        let mut entries: Vec<&Value> = all_traits.iter().collect();
        entries.sort_by(|a, b| {
            let pa = a.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let pb = b.get("path").and_then(|v| v.as_str()).unwrap_or("");
            pa.cmp(pb)
        });

        for entry in entries {
            let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                continue;
            }
            if VOICE_TOOL_EXCLUDE.contains(&path) {
                continue;
            }
            if path.starts_with("www.") {
                continue;
            }
            let kind = entry
                .get("source")
                .or_else(|| entry.get("kind"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if kind == "library" || kind == "interface" {
                continue;
            }

            let tool_name = path.replace('.', "_");
            let description = entry
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let params = entry.get("params").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let parameters = build_parameters(&params);

            tools.push(json!({
                "type": "function",
                "name": tool_name,
                "description": description,
                "parameters": parameters,
            }));
        }
    }

    // Always include sys_voice_quit so the model can end the session
    tools.push(json!({
        "type": "function",
        "name": "sys_voice_quit",
        "description": "End the voice conversation. Call this when the user says goodbye, wants to stop, or asks to quit.",
        "parameters": { "type": "object", "properties": {} }
    }));

    // Synthetic canvas tool — routes to llm.agent which uses sys.vfs to write canvas/app.html
    tools.push(json!({
        "type": "function",
        "name": "canvas",
        "description": "Create, build, or modify anything on the live visual canvas. Invokes a coding agent that writes a complete self-contained HTML+CSS+JS app to canvas/app.html — the canvas page updates automatically. Examples: \"create a breakout clone\", \"draw animated particles\", \"make a Spotify controller\", \"add a reset button\".",
        "parameters": {
            "type": "object",
            "properties": {
                "request": {
                    "type": "string",
                    "description": "What to create or change on the canvas. Use the user's exact words — the agent will expand this into a full implementation."
                }
            },
            "required": ["request"]
        }
    }));

    // On canvas page: also expose sys_echo and sys_audio from the registry
    if is_canvas {
        for entry in &all_traits {
            let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path != "sys.echo" && path != "sys.audio" {
                continue;
            }
            let tool_name = path.replace('.', "_");
            let description = entry
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let params = entry.get("params").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let parameters = build_parameters(&params);
            tools.push(json!({
                "type": "function",
                "name": tool_name,
                "description": description,
                "parameters": parameters,
            }));
        }
    }

    json!({ "ok": true, "tools": tools, "count": tools.len() })
}

/// Build a JSON Schema parameters object from a trait's params array.
fn build_parameters(params: &[Value]) -> Value {
    let mut properties = Map::new();
    let mut required: Vec<Value> = Vec::new();

    for param in params {
        let name = param.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let type_str = param
            .get("type")
            .or_else(|| param.get("param_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("string");
        let description = param
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let optional = param
            .get("optional")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let required_flag = param
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mut prop = type_to_schema(type_str);
        if !description.is_empty() {
            if let Value::Object(ref mut m) = prop {
                m.insert("description".to_string(), json!(description));
            }
        }
        properties.insert(name.to_string(), prop);
        if !optional && required_flag {
            required.push(json!(name));
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

/// Map trait type string → JSON Schema type.
pub fn type_to_schema(type_str: &str) -> Value {
    let t = type_str.to_lowercase().replace(' ', "");
    if t == "int" || t == "integer" {
        return json!({"type": "integer"});
    }
    if t == "float" || t == "number" {
        return json!({"type": "number"});
    }
    if t == "bool" || t == "boolean" {
        return json!({"type": "boolean"});
    }
    if t.starts_with("list<") || t.starts_with("array<") {
        let inner = &t[t.find('<').unwrap_or(0) + 1..t.rfind('>').unwrap_or(t.len())];
        return json!({"type": "array", "items": type_to_schema(inner)});
    }
    if t.starts_with("map<") {
        return json!({"type": "object"});
    }
    json!({"type": "string"})
}
