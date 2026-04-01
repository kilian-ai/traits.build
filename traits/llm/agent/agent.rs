use serde_json::{json, Value};

/// llm.agent — WASM-compatible LLM agent loop with trait-based tool calling.
///
/// Implements the opencode-style agent algorithm purely in Rust:
///   1. Send prompt + tool definitions to OpenAI-compatible API
///   2. If response has `tool_calls`, execute each via platform::dispatch
///   3. Append tool results, repeat until `finish_reason = "stop"` or max_steps
///
/// Works natively and in WASM (via synchronous XHR in sys.call).
///
/// Args: [prompt, system?, tools?, model?, max_steps?, api_secret?]
///   prompt:     User message (required)
///   system:     System prompt (optional, has default coding agent prompt)
///   tools:      Comma-separated trait paths to expose as tools, or empty for defaults
///   model:      OpenAI model (default: gpt-4o-mini)
///   max_steps:  Max agent loop iterations (default: 10)
///   api_secret: Secret name for OpenAI API key (default: "openai_api_key")
pub fn agent(args: &[Value]) -> Value {
    let prompt = match args.first().and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return json!({ "ok": false, "error": "prompt is required" }),
    };

    let system = args.get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_SYSTEM)
        .to_string();

    let tools_arg = args.get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();

    let model = args.get(3)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("gpt-4o-mini")
        .to_string();

    let max_steps = args.get(4)
        .and_then(|v| v.as_u64())
        .unwrap_or(10)
        .min(50) as usize;

    let api_secret = args.get(5)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("openai_api_key")
        .to_string();

    // Build tool definitions and a name→path reverse map
    let (tool_defs, name_to_path) = build_tool_definitions(&tools_arg);

    // Initial messages
    let mut messages: Vec<Value> = vec![
        json!({"role": "system", "content": system}),
        json!({"role": "user", "content": prompt}),
    ];

    let mut step_count = 0usize;
    let mut final_response = String::new();
    let mut all_tool_calls: Vec<Value> = Vec::new();

    // Agent loop
    for _ in 0..max_steps {
        step_count += 1;

        // Build request body
        let mut request_body = json!({
            "model": model,
            "messages": messages,
        });

        if !tool_defs.is_empty() {
            request_body["tools"] = json!(tool_defs);
            request_body["tool_choice"] = json!("auto");
        }

        // Call OpenAI-compatible API
        let call_args = vec![
            json!("https://api.openai.com/v1/chat/completions"),
            request_body,
            json!(api_secret),
            json!("POST"),
        ];

        let result = kernel_logic::platform::dispatch("sys.call", &call_args)
            .unwrap_or_else(|| json!({"ok": false, "error": "sys.call unavailable"}));

        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        if !ok {
            let err = result.pointer("/body/error/message")
                .and_then(|v| v.as_str())
                .or_else(|| result.get("error").and_then(|v| v.as_str()))
                .unwrap_or("API call failed");
            return json!({
                "ok": false,
                "error": err,
                "step_count": step_count,
            });
        }

        let body = result.get("body").cloned().unwrap_or(Value::Null);
        let choice = match body.pointer("/choices/0/message") {
            Some(c) => c.clone(),
            None => {
                return json!({
                    "ok": false,
                    "error": "No choices in API response",
                    "step_count": step_count,
                    "raw": body,
                });
            }
        };

        let finish_reason = body.pointer("/choices/0/finish_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("stop");

        // Extract text content if present
        if let Some(text) = choice.get("content").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                final_response = text.to_string();
            }
        }

        // Check if there are tool calls to process
        let tool_calls = choice.get("tool_calls")
            .and_then(|v| v.as_array())
            .cloned();

        if finish_reason == "stop" || tool_calls.is_none() {
            // No more tool calls — done
            messages.push(choice.clone());
            break;
        }

        let tool_calls = tool_calls.unwrap();

        // Append assistant message with tool_calls to history
        messages.push(choice.clone());

        // Execute each tool call
        for tc in &tool_calls {
            let call_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let fn_name = tc.pointer("/function/name").and_then(|v| v.as_str()).unwrap_or("");
            let fn_args_str = tc.pointer("/function/arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("{}");

            // Parse function arguments JSON
            let fn_args_obj: Value = serde_json::from_str(fn_args_str)
                .unwrap_or_else(|_| json!({}));

            // Map tool name → trait path using pre-built reverse map
            let trait_path = name_to_path.get(fn_name)
                .cloned()
                .unwrap_or_else(|| tool_name_to_trait_path(fn_name));

            // Build positional args from named JSON object using trait signature
            let call_args = named_args_to_positional(&trait_path, &fn_args_obj);

            // Execute via platform dispatch
            let tool_result = kernel_logic::platform::dispatch(&trait_path, &call_args)
                .unwrap_or_else(|| json!({
                    "error": format!("trait '{}' not found or not callable", trait_path)
                }));

            // Record for final output
            all_tool_calls.push(json!({
                "id": call_id,
                "name": fn_name,
                "trait": trait_path,
                "args": fn_args_obj,
                "result": tool_result,
            }));

            // Append tool result message
            messages.push(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": serde_json::to_string(&tool_result).unwrap_or_default(),
            }));
        }

        // If finish_reason was tool_calls, continue the loop
        if finish_reason != "tool_calls" {
            break;
        }
    }

    json!({
        "ok": true,
        "response": final_response,
        "tool_calls": all_tool_calls,
        "step_count": step_count,
        "messages": messages,
    })
}

/// Convert OpenAI tool name back to trait path: sys_checksum → sys.checksum
fn tool_name_to_trait_path(name: &str) -> String {
    // Replace first underscore with dot: sys_checksum → sys.checksum
    // But handle multi-segment paths: sys_chat_protocols_vscode → sys.chat_protocols.vscode
    // Convention: trait paths use dots, tool names use underscores
    // We stored the mapping when building tools, so reverse via simple replacement
    name.replacen('_', ".", 1)
}

/// Convert trait path to OpenAI tool name: sys.checksum → sys_checksum
fn trait_path_to_tool_name(path: &str) -> String {
    path.replace('.', "_")
}

/// Build positional args from a named JSON object using trait signature metadata.
/// Falls back to wrapping the whole object if no signature found.
fn named_args_to_positional(trait_path: &str, named: &Value) -> Vec<Value> {
    // Try to get signature from registry
    let detail = kernel_logic::platform::registry_detail(trait_path);

    if let Some(detail_val) = detail {
        if let Some(params) = detail_val.pointer("/signature/params").and_then(|v| v.as_array()) {
            let mut positional = Vec::new();
            for param in params {
                let name = param.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let value = named.get(name).cloned().unwrap_or(Value::Null);
                positional.push(value);
            }
            // Remove trailing Nulls to match trait's optional args
            while positional.last() == Some(&Value::Null) {
                positional.pop();
            }
            return positional;
        }
    }

    // Fallback: single arg with the whole object
    vec![named.clone()]
}

/// Build OpenAI tool definitions from a comma-separated list of trait paths.
/// Returns (tool_defs, name→path map).
/// Empty string → use DEFAULT_TOOLS list.
fn build_tool_definitions(tools_csv: &str) -> (Vec<Value>, std::collections::HashMap<String, String>) {
    let paths: Vec<&str> = if tools_csv.is_empty() {
        DEFAULT_TOOLS.iter().map(|s| *s).collect()
    } else {
        tools_csv.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
    };

    let mut defs = Vec::new();
    let mut map = std::collections::HashMap::new();

    for path in &paths {
        if let Some(def) = trait_to_tool_def(path) {
            let tool_name = trait_path_to_tool_name(path);
            map.insert(tool_name, path.to_string());
            defs.push(def);
        }
    }

    (defs, map)
}

/// Convert a trait's registry metadata into an OpenAI function tool definition.
fn trait_to_tool_def(trait_path: &str) -> Option<Value> {
    let detail = kernel_logic::platform::registry_detail(trait_path)?;

    let description = detail.pointer("/trait/description")
        .and_then(|v| v.as_str())
        .unwrap_or(trait_path)
        .to_string();

    let tool_name = trait_path_to_tool_name(trait_path);

    // Build JSON Schema properties from trait signature params
    let mut properties = serde_json::Map::new();
    let mut required_params: Vec<Value> = Vec::new();

    if let Some(params) = detail.pointer("/signature/params").and_then(|v| v.as_array()) {
        for param in params {
            let name = param.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }

            let type_str = param.get("type").and_then(|v| v.as_str()).unwrap_or("string");
            let json_type = trait_type_to_json_schema(type_str);
            let param_desc = param.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or(&name)
                .to_string();

            let is_required = param.get("required")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            properties.insert(name.clone(), json!({
                "type": json_type,
                "description": param_desc,
            }));

            if is_required {
                required_params.push(json!(name));
            }
        }
    }

    let parameters = json!({
        "type": "object",
        "properties": properties,
        "required": required_params,
    });

    Some(json!({
        "type": "function",
        "function": {
            "name": tool_name,
            "description": description,
            "parameters": parameters,
        }
    }))
}

/// Map trait type strings to JSON Schema type strings.
fn trait_type_to_json_schema(t: &str) -> &'static str {
    match t {
        "int" => "integer",
        "float" => "number",
        "bool" => "boolean",
        "string" | "bytes" => "string",
        _ => "string", // any, handle, list, map → string fallback
    }
}

// ─── Constants ──────────────────────────────────────────────────────────────

/// Default tools exposed to the agent when no tools_csv is specified.
/// All entries must be WASM-callable (wasm = true in their .trait.toml).
const DEFAULT_TOOLS: &[&str] = &[
    "sys.call",
    "sys.list",
    "sys.registry",
    "kernel.call",
];

const DEFAULT_SYSTEM: &str = "\
You are a helpful AI assistant with access to a set of tools (traits). \
When you need to perform an action, call the appropriate tool. \
Think step by step and use tools to accomplish the user's request. \
When you have gathered enough information, provide a clear, concise response.";
