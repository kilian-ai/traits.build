use serde_json::{json, Value};

/// llm.agent — WASM-compatible LLM agent loop with trait-based tool calling.
///
/// Distilled from the claw-code agent architecture (ConversationRuntime pattern):
///   1. Send prompt + tool definitions to OpenAI-compatible API
///   2. If response has `tool_calls`, execute each via platform::dispatch
///   3. Compact message history when token budget is exceeded (claw-code compact.rs)
///   4. Track token usage from API responses (claw-code conversation.rs)
///   5. Repeat until `finish_reason = "stop"` or max_steps
///
/// Modes:
///   "full"  (default) — run until done, return final response
///   "turn"  — run one LLM turn + tool execution, return state for companion/buddy UX
///
/// Works natively and in WASM (via synchronous XHR in sys.call).
///
/// Args: [prompt, system?, tools?, model?, max_steps?, api_secret?, mode?, session?]
///   prompt:     User message (required)
///   system:     System prompt (optional, has default coding agent prompt)
///   tools:      Comma-separated trait paths to expose as tools, or empty for defaults
///   model:      OpenAI model (default: gpt-4o-mini)
///   max_steps:  Max agent loop iterations (default: 10)
///   api_secret: Secret name for OpenAI API key (default: "openai_api_key")
///   mode:       "full" (run to completion) or "turn" (single turn for buddy UX)
///   session:    Previous messages array (for multi-turn buddy sessions)
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
        .min(MAX_STEPS_LIMIT as u64) as usize;

    let api_secret = args.get(5)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("openai_api_key")
        .to_string();

    let mode = args.get(6)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("full")
        .to_string();

    let is_turn_mode = mode == "turn";

    // Build tool definitions and a name→path reverse map
    let (tool_defs, name_to_path) = build_tool_definitions(&tools_arg);

    // Restore session from previous messages (for multi-turn buddy mode)
    let mut messages: Vec<Value> = if let Some(session) = args.get(7).and_then(|v| v.as_array()) {
        let mut msgs = session.clone();
        msgs.push(json!({"role": "user", "content": prompt}));
        msgs
    } else {
        vec![
            json!({"role": "system", "content": system}),
            json!({"role": "user", "content": prompt}),
        ]
    };

    let mut step_count = 0usize;
    let mut final_response = String::new();
    let mut all_tool_calls: Vec<Value> = Vec::new();
    let mut total_usage = UsageTracker::default();
    let mut compacted_count = 0usize;

    // Agent loop (claw-code ConversationRuntime.run_turn pattern)
    for _ in 0..max_steps {
        step_count += 1;

        // Compact if token budget exceeded (claw-code compact.rs pattern)
        if should_compact(&messages) {
            compacted_count += compact_messages(&mut messages);
        }

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
                "usage": total_usage.to_json(),
            });
        }

        let body = result.get("body").cloned().unwrap_or(Value::Null);

        // Track token usage (claw-code conversation.rs UsageSummary pattern)
        if let Some(usage) = body.get("usage") {
            total_usage.add_from_response(usage);
        }

        let choice = match body.pointer("/choices/0/message") {
            Some(c) => c.clone(),
            None => {
                return json!({
                    "ok": false,
                    "error": "No choices in API response",
                    "step_count": step_count,
                    "usage": total_usage.to_json(),
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

        // Turn mode: return after processing one round of tool calls (buddy UX)
        if is_turn_mode {
            return json!({
                "ok": true,
                "done": false,
                "response": final_response,
                "tool_calls": all_tool_calls,
                "step_count": step_count,
                "usage": total_usage.to_json(),
                "compacted_messages": compacted_count,
                "session": messages,
            });
        }

        // Full mode: continue if finish_reason was tool_calls
        if finish_reason != "tool_calls" {
            break;
        }
    }

    json!({
        "ok": true,
        "done": true,
        "response": final_response,
        "tool_calls": all_tool_calls,
        "step_count": step_count,
        "usage": total_usage.to_json(),
        "compacted_messages": compacted_count,
        "session": messages,
    })
}

// ─── Usage Tracking (claw-code conversation.rs UsageSummary) ────────────────

#[derive(Default)]
struct UsageTracker {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
}

impl UsageTracker {
    fn add_from_response(&mut self, usage: &Value) {
        self.input_tokens += usage.get("prompt_tokens")
            .and_then(|v| v.as_u64()).unwrap_or(0);
        self.output_tokens += usage.get("completion_tokens")
            .and_then(|v| v.as_u64()).unwrap_or(0);
        self.total_tokens += usage.get("total_tokens")
            .and_then(|v| v.as_u64()).unwrap_or(0);
    }

    fn to_json(&self) -> Value {
        json!({
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "total_tokens": self.total_tokens,
        })
    }
}

// ─── Session Compaction (claw-code compact.rs) ──────────────────────────────
//
// When the conversation grows beyond COMPACT_MAX_TOKENS, older messages
// (except the most recent COMPACT_PRESERVE_RECENT) are summarized into
// a compact system message with <summary> tags. This prevents context
// window overflow on long agent runs while preserving recent context.

fn estimate_tokens(text: &str) -> usize {
    // claw-code formula: len/4 + 1 (rough approximation of GPT tokenization)
    text.len() / 4 + 1
}

fn estimate_message_tokens(msg: &Value) -> usize {
    let content_tokens = msg.get("content")
        .and_then(|v| v.as_str())
        .map(estimate_tokens)
        .unwrap_or(0);

    let tool_tokens = msg.get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|tcs| tcs.iter().map(|tc| {
            let name_t = tc.pointer("/function/name")
                .and_then(|v| v.as_str()).map(estimate_tokens).unwrap_or(0);
            let args_t = tc.pointer("/function/arguments")
                .and_then(|v| v.as_str()).map(estimate_tokens).unwrap_or(0);
            name_t + args_t
        }).sum::<usize>())
        .unwrap_or(0);

    content_tokens + tool_tokens + 4 // +4 per-message overhead (role, separators)
}

fn should_compact(messages: &[Value]) -> bool {
    let total: usize = messages.iter().map(|m| estimate_message_tokens(m)).sum();
    if total <= COMPACT_MAX_TOKENS {
        return false;
    }
    // Only compact if there are enough messages beyond the preserved window
    messages.len() > COMPACT_PRESERVE_RECENT + 1
}

/// Compact older messages into a summary. Returns the number of messages removed.
fn compact_messages(messages: &mut Vec<Value>) -> usize {
    if messages.len() <= COMPACT_PRESERVE_RECENT + 1 {
        return 0;
    }

    // Keep first message (system prompt) and last N messages
    let system = messages[0].clone();
    let split_point = messages.len().saturating_sub(COMPACT_PRESERVE_RECENT);
    let older = &messages[1..split_point];
    let removed_count = older.len();

    if removed_count == 0 {
        return 0;
    }

    // Build structured summary (claw-code summarize_messages pattern)
    let mut user_requests: Vec<String> = Vec::new();
    let mut tools_used: Vec<String> = Vec::new();
    let mut key_results: Vec<String> = Vec::new();

    for msg in older {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        match role {
            "user" => {
                if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                    let truncated = if content.len() > 200 { &content[..200] } else { content };
                    user_requests.push(truncated.to_string());
                }
            }
            "assistant" => {
                if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                    if !content.is_empty() {
                        let truncated = if content.len() > 150 { &content[..150] } else { content };
                        key_results.push(truncated.to_string());
                    }
                }
                if let Some(tcs) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                    for tc in tcs {
                        if let Some(name) = tc.pointer("/function/name").and_then(|v| v.as_str()) {
                            if !tools_used.contains(&name.to_string()) {
                                tools_used.push(name.to_string());
                            }
                        }
                    }
                }
            }
            "tool" => {
                // Tool results are implied by the tools_used list
            }
            _ => {}
        }
    }

    let summary = format!(
        "<summary>\nCompacted context ({} messages removed):\n\
         User requests: {}\n\
         Tools used: {}\n\
         Key results: {}\n\
         </summary>\n\
         The conversation continues from here. Recent messages are preserved verbatim.",
        removed_count,
        if user_requests.is_empty() { "none".to_string() }
        else { user_requests.join(" | ") },
        if tools_used.is_empty() { "none".to_string() }
        else { tools_used.join(", ") },
        if key_results.is_empty() { "none".to_string() }
        else { key_results.join(" | ") },
    );

    // Rebuild: system + compaction summary + recent messages
    let recent: Vec<Value> = messages[split_point..].to_vec();
    messages.clear();
    messages.push(system);
    messages.push(json!({"role": "system", "content": summary}));
    messages.extend(recent);

    removed_count
}

// ─── Tool name ↔ trait path conversion ──────────────────────────────────────

/// Convert OpenAI tool name back to trait path: sys_checksum → sys.checksum
fn tool_name_to_trait_path(name: &str) -> String {
    name.replacen('_', ".", 1)
}

/// Convert trait path to OpenAI tool name: sys.checksum → sys_checksum
fn trait_path_to_tool_name(path: &str) -> String {
    path.replace('.', "_")
}

// ─── Argument mapping ───────────────────────────────────────────────────────

/// Build positional args from a named JSON object using trait signature metadata.
/// Falls back to wrapping the whole object if no signature found.
fn named_args_to_positional(trait_path: &str, named: &Value) -> Vec<Value> {
    let detail = kernel_logic::platform::registry_detail(trait_path);

    if let Some(detail_val) = detail {
        if let Some(params) = detail_val.pointer("/signature/params").and_then(|v| v.as_array()) {
            let mut positional = Vec::new();
            for param in params {
                let name = param.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let value = named.get(name).cloned().unwrap_or(Value::Null);
                positional.push(value);
            }
            while positional.last() == Some(&Value::Null) {
                positional.pop();
            }
            return positional;
        }
    }

    vec![named.clone()]
}

// ─── Tool definition building ───────────────────────────────────────────────

/// Build OpenAI tool definitions from a comma-separated list of trait paths.
/// Returns (tool_defs, name→path map).
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
        _ => "string",
    }
}

// ─── Constants ──────────────────────────────────────────────────────────────

const DEFAULT_TOOLS: &[&str] = &[
    "sys.call",
    "sys.vfs",
    "sys.list",
    "sys.registry",
    "kernel.call",
];

const DEFAULT_SYSTEM: &str = "\
You are a helpful AI assistant with access to a set of tools (traits). \
When you need to perform an action, call the appropriate tool. \
Think step by step and use tools to accomplish the user's request. \
When you have gathered enough information, provide a clear, concise response.\n\n\
FILE TOOLS: You have a virtual filesystem (sys.vfs) for reading and writing files. \
Use action=\"read\" with path to read a file, action=\"write\" with path and content to write, \
action=\"list\" to list files, action=\"delete\" to remove, action=\"exists\" to check.\n\n\
CANVAS: The file `canvas/app.html` on the VFS is rendered live on the /canvas page in the browser. \
To build visual/interactive content, FIRST try to read `canvas/app.html` with sys.vfs to see what's already there. \
If the file does not exist or the read fails, create it from scratch — never ask the user for permission. \
If it exists, modify the content based on the user's request. \
Then write the updated version back with sys.vfs write. Write complete, self-contained HTML with inline \
CSS and JS — no external dependencies. The canvas page updates automatically when this file changes. \
Prefer dark backgrounds (#0a0a0a) and light text (#e0e0e0) to match the site theme.\n\n\
CANVAS RENDERING RULES (your HTML is injected into a container div, NOT a standalone page):\n\
- Your <script> runs inside a new Function() wrapper with access to document and global scope.\n\
- HTML is placed inside <div id=\"canvas-container\">. Use document.querySelector('#canvas-container canvas') to find your canvas element.\n\
- NEVER use document.getElementById to find your canvas — use querySelector on the container instead.\n\
- Use `let` for variables you reassign in loops, NEVER const. Reassigning a const crashes the script silently.\n\
- For animation loops, store the rAF ID: window.__canvasAnimId = requestAnimationFrame(loop);\n\
- Keep scripts simple: get canvas from container, draw, animate. No DOMContentLoaded listeners.\n\
- Example pattern: const c = document.querySelector('#canvas-container canvas'); if(!c) return; const ctx = c.getContext('2d'); let x=100; function loop(){ ctx.clearRect(0,0,c.width,c.height); x+=2; window.__canvasAnimId=requestAnimationFrame(loop); } loop();\n\n\
The canvas page injects a `window.traits` object your scripts can use: \
traits.call(path, args), traits.list(), traits.canvas(action, content), traits.echo(text), traits.audio(action, ...).";


const MAX_STEPS_LIMIT: usize = 50;

/// Compaction: keep the last N messages verbatim when compacting.
const COMPACT_PRESERVE_RECENT: usize = 4;

/// Compaction: trigger when estimated tokens exceed this threshold.
const COMPACT_MAX_TOKENS: usize = 10_000;
