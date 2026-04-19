use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const AGENT_CONFIG_PATH: &str = "config/llm_agent.json";

struct AgentConfig {
    model: String,
    api_secret: String,
    max_steps: usize,
    default_session_id: String,
}

fn default_agent_config() -> AgentConfig {
    AgentConfig {
        model: "gpt-4o-mini".to_string(),
        api_secret: "openai_api_key".to_string(),
        max_steps: 10,
        default_session_id: "default".to_string(),
    }
}

fn load_agent_config() -> AgentConfig {
    let defaults = default_agent_config();
    let raw = kernel_logic::platform::vfs_read(AGENT_CONFIG_PATH);

    let cfg = if let Some(raw_json) = raw {
        if let Ok(v) = serde_json::from_str::<Value>(&raw_json) {
            AgentConfig {
                model: v
                    .get("model")
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&defaults.model)
                    .to_string(),
                api_secret: v
                    .get("api_secret")
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&defaults.api_secret)
                    .to_string(),
                max_steps: v
                    .get("max_steps")
                    .and_then(|x| x.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(defaults.max_steps)
                    .clamp(1, MAX_STEPS_LIMIT),
                default_session_id: v
                    .get("default_session_id")
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&defaults.default_session_id)
                    .to_string(),
            }
        } else {
            defaults
        }
    } else {
        defaults
    };

    // Ensure config is visible/editable in VFS.
    if kernel_logic::platform::vfs_read(AGENT_CONFIG_PATH).is_none() {
        let seed = json!({
            "model": cfg.model,
            "api_secret": cfg.api_secret,
            "max_steps": cfg.max_steps,
            "default_session_id": cfg.default_session_id,
            "notes": "Edit these defaults for llm.agent calls that omit args"
        });
        if let Ok(seed_json) = serde_json::to_string_pretty(&seed) {
            kernel_logic::platform::vfs_write(AGENT_CONFIG_PATH, &seed_json);
        }
    }

    cfg
}

static SESSION_STORE: OnceLock<Mutex<HashMap<String, Vec<Value>>>> = OnceLock::new();

fn session_store() -> &'static Mutex<HashMap<String, Vec<Value>>> {
    SESSION_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_session(session_id: &str) -> Option<Vec<Value>> {
    // 1) Fast in-process cache
    if let Ok(guard) = session_store().lock() {
        if let Some(cached) = guard.get(session_id) {
            return Some(cached.clone());
        }
    }

    // 2) Durable VFS-backed session memory (visible from terminal VFS)
    let path = session_vfs_path(session_id);
    let raw = kernel_logic::platform::vfs_read(&path)?;
    let parsed = serde_json::from_str::<Vec<Value>>(&raw).ok()?;

    if let Ok(mut guard) = session_store().lock() {
        guard.insert(session_id.to_string(), parsed.clone());
    }
    Some(parsed)
}

fn save_session(session_id: &str, mut messages: Vec<Value>) {
    // Keep session payload compact before storing for follow-up turns.
    while should_compact(&messages) {
        if compact_messages(&mut messages) == 0 {
            break;
        }
    }
    // Guard persisted history against invalid assistant/tool sequences so
    // future turns cannot fail with OpenAI role-order validation errors.
    let (sanitized, _) = sanitize_messages_for_api(&messages);
    messages = sanitized;

    if let Ok(mut guard) = session_store().lock() {
        guard.insert(session_id.to_string(), messages);
    }

    let path = session_vfs_path(session_id);
    if let Ok(json) = serde_json::to_string(&session_store().lock().ok().and_then(|g| g.get(session_id).cloned()).unwrap_or_default()) {
        kernel_logic::platform::vfs_write(&path, &json);
    }
}

fn session_vfs_path(session_id: &str) -> String {
    // Stored in VFS so users can inspect chat memory directly from terminal.
    format!("agent_memory/{}.json", sanitize_session_id(session_id))
}

fn sanitize_session_id(session_id: &str) -> String {
    let mut out = String::with_capacity(session_id.len());
    for ch in session_id.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "default".to_string()
    } else {
        out
    }
}

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
///   session:    Previous messages array OR session id string (for multi-turn sessions)
///   session_id: Explicit session id for persisted auto-history (default: "default")
pub fn agent(args: &[Value]) -> Value {
    let cfg = load_agent_config();

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
        .unwrap_or(&cfg.model)
        .to_string();

    let max_steps = args.get(4)
        .and_then(|v| v.as_u64())
        .unwrap_or(cfg.max_steps as u64)
        .min(MAX_STEPS_LIMIT as u64) as usize;

    let api_secret = args.get(5)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&cfg.api_secret)
        .to_string();

    let mode = args.get(6)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("full")
        .to_string();

    let is_turn_mode = mode == "turn";

    // Build tool definitions and a name→path reverse map
    let (tool_defs, name_to_path) = build_tool_definitions(&tools_arg);

    let explicit_session_id = args.get(8)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let inline_session_id = args.get(7)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let session_id = explicit_session_id
        .or(inline_session_id)
        .unwrap_or_else(|| cfg.default_session_id.clone());

    // Restore session from either explicit messages, persisted store, or fresh system+user.
    let mut messages: Vec<Value> = if let Some(session) = args.get(7).and_then(|v| v.as_array()) {
        let mut msgs = session.clone();
        msgs.push(json!({"role": "user", "content": prompt}));
        msgs
    } else if let Some(mut persisted) = load_session(&session_id) {
        if persisted.is_empty() || persisted[0].get("role").and_then(|v| v.as_str()) != Some("system") {
            persisted.insert(0, json!({"role": "system", "content": system}));
        }
        persisted.push(json!({"role": "user", "content": prompt}));
        persisted
    } else {
        vec![
            json!({"role": "system", "content": system}),
            json!({"role": "user", "content": prompt}),
        ]
    };

    let (sanitized_start, _) = sanitize_messages_for_api(&messages);
    messages = sanitized_start;

    let mut step_count = 0usize;
    let mut final_response = String::new();
    let mut all_tool_calls: Vec<Value> = Vec::new();
    let mut total_usage = UsageTracker::default();
    let mut compacted_count = 0usize;
    let mut execution_retry_used = false;
    let mut input_authenticity_retry_used = false;

    // Agent loop (claw-code ConversationRuntime.run_turn pattern)
    for _ in 0..max_steps {
        step_count += 1;

        // Compact if token budget exceeded (claw-code compact.rs pattern)
        if should_compact(&messages) {
            compacted_count += compact_messages(&mut messages);
        }

        // Enforce OpenAI role invariants before every request. This recovers
        // from malformed persisted sessions (e.g. orphaned tool messages).
        let (sanitized, _) = sanitize_messages_for_api(&messages);
        messages = sanitized;

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
                "session_id": session_id,
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
                    "session_id": session_id,
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

            // Input authenticity guard: if the task needs user-provided inputs
            // and those were not provided, prevent fabricated demo values/results.
            if !is_turn_mode
                && !input_authenticity_retry_used
                && should_force_input_authenticity_retry(&prompt, &all_tool_calls)
            {
                input_authenticity_retry_used = true;
                messages.push(json!({
                    "role": "user",
                    "content": "Input authenticity guard: do not invent missing user data, operands, example constants, CLI args, or final results. If required input was not provided, ask a concise clarification by default. Use interactive prompt/stdin only when the user explicitly wants an interactive terminal flow or the runtime is already in an active prompt loop. Do not hardcode placeholder/demo values. Re-run with real user input before finalizing. End with: FINAL: <built artifact> | RAN: <terminal|canvas> | RESULT: <outcome>."
                }));
                continue;
            }

            // Hard guard: if the agent produced build-like file mutations but did not
            // execute code in terminal or render to canvas, force one retry pass.
            if !is_turn_mode
                && !execution_retry_used
                && should_force_execution_retry(&prompt, &all_tool_calls)
            {
                execution_retry_used = true;
                messages.push(json!({
                    "role": "user",
                    "content": "Execution guard: before finalizing, run the built artifact now. For JavaScript files, use sys.js (NOT node/deno/bun — use sys_js tool or sys.shell with 'js <file>'). For interactive or visual JS, render via sys.canvas set instead. Do not only write or cat files. End with: FINAL: <built artifact> | RAN: <terminal|canvas> | RESULT: <outcome>."
                }));
                continue;
            }

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
            save_session(&session_id, messages.clone());
            return json!({
                "ok": true,
                "done": false,
                "response": final_response,
                "tool_calls": all_tool_calls,
                "session_id": session_id,
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

    save_session(&session_id, messages.clone());

    json!({
        "ok": true,
        "done": true,
        "response": final_response,
        "tool_calls": all_tool_calls,
        "session_id": session_id,
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

/// Sanitize message history so OpenAI Chat Completions role-order rules hold:
/// - `tool` messages must immediately follow an assistant with matching `tool_calls`
/// - assistant `tool_calls` blocks must be complete; otherwise drop the block
/// Returns (sanitized_messages, dropped_count).
fn sanitize_messages_for_api(messages: &[Value]) -> (Vec<Value>, usize) {
    let mut out: Vec<Value> = Vec::with_capacity(messages.len());
    let mut dropped = 0usize;

    // Active assistant tool-call block being validated.
    let mut pending_ids: Vec<String> = Vec::new();
    let mut pending_start_idx: Option<usize> = None;

    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");

        match role {
            "assistant" => {
                // If previous assistant tool-call block is incomplete, drop it.
                if !pending_ids.is_empty() {
                    if let Some(start) = pending_start_idx {
                        dropped += out.len().saturating_sub(start);
                        out.truncate(start);
                    }
                }
                pending_ids.clear();
                pending_start_idx = None;

                let ids: Vec<String> = msg
                    .get("tool_calls")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|tc| tc.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                out.push(msg.clone());
                if !ids.is_empty() {
                    pending_start_idx = Some(out.len() - 1);
                    pending_ids = ids;
                }
            }
            "tool" => {
                let call_id = msg.get("tool_call_id").and_then(|v| v.as_str()).unwrap_or("");
                if call_id.is_empty() {
                    dropped += 1;
                    continue;
                }

                if let Some(pos) = pending_ids.iter().position(|id| id == call_id) {
                    out.push(msg.clone());
                    pending_ids.remove(pos);
                    if pending_ids.is_empty() {
                        pending_start_idx = None;
                    }
                } else {
                    // Orphan tool message: no matching pending assistant tool call.
                    dropped += 1;
                }
            }
            _ => {
                // Non-tool role while an assistant tool-call block is unresolved: drop block.
                if !pending_ids.is_empty() {
                    if let Some(start) = pending_start_idx {
                        dropped += out.len().saturating_sub(start);
                        out.truncate(start);
                    }
                    pending_ids.clear();
                    pending_start_idx = None;
                }
                out.push(msg.clone());
            }
        }
    }

    // Drop trailing incomplete assistant tool-call block.
    if !pending_ids.is_empty() {
        if let Some(start) = pending_start_idx {
            dropped += out.len().saturating_sub(start);
            out.truncate(start);
        }
    }

    (out, dropped)
}

// ─── Execution Guard ───────────────────────────────────────────────────────

fn should_force_execution_retry(prompt: &str, tool_calls: &[Value]) -> bool {
    if tool_calls.is_empty() {
        return false;
    }

    let wrote_or_built = tool_calls_have_build_actions(tool_calls);
    let ran_or_rendered = tool_calls_have_run_or_render(tool_calls);
    let build_intent = prompt_suggests_build_or_script(prompt);

    // Only enforce when there is strong evidence this was a build-like turn
    // and no execution/render step happened yet.
    (wrote_or_built || build_intent) && !ran_or_rendered
}

fn should_force_input_authenticity_retry(prompt: &str, tool_calls: &[Value]) -> bool {
    if tool_calls.is_empty() {
        return false;
    }
    if prompt_has_explicit_user_inputs(prompt) {
        return false;
    }

    let uses_interactive = tool_calls_use_interactive_input(tool_calls);
    let hardcoded = tool_calls_hardcode_demo_values(tool_calls);
    let invented_args = tool_calls_invent_runtime_args(prompt, tool_calls);
    let node_only_js = tool_calls_use_node_only_js_apis(tool_calls);

    (hardcoded || invented_args || node_only_js) && !uses_interactive
}

fn prompt_has_explicit_user_inputs(prompt: &str) -> bool {
    let p = prompt.to_ascii_lowercase();
    let has_quotes = p.contains('"') || p.contains('\'');
    let has_digits = p.chars().any(|c| c.is_ascii_digit());
    has_digits || has_quotes
}

fn tool_calls_use_interactive_input(tool_calls: &[Value]) -> bool {
    tool_calls.iter().any(|tc| {
        let name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = tc.get("args").cloned().unwrap_or(Value::Null);

        if name == "sys_shell" {
            let cmd = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            // Only count interactive input on actual execution commands,
            // not on file-writing commands that merely contain those strings.
            if !is_shell_runtime_execution_command(&cmd) {
                return false;
            }

            return cmd.contains("prompt(")
                || cmd.contains("stdin")
                || cmd.contains("io.read");
        }

        if name == "sys_js" {
            return true;
        }

        false
    })
}

fn tool_calls_hardcode_demo_values(tool_calls: &[Value]) -> bool {
    tool_calls.iter().any(|tc| {
        let name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name != "sys_shell" {
            return false;
        }

        let cmd = tc
            .get("args")
            .and_then(|a| a.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        // Heuristic indicators of fabricated/demo constants in generated code.
        (cmd.contains("write ") || cmd.contains("tee "))
            && (cmd.contains("const ") || cmd.contains("let "))
            && (cmd.contains("first number")
                || cmd.contains("second number")
                || cmd.contains("example")
                || cmd.contains("demo")
                || cmd.contains("sample")
                || cmd.contains("const num")
                || cmd.contains("let num")
                || cmd.contains("operation ="))
    })
}

fn tool_calls_invent_runtime_args(prompt: &str, tool_calls: &[Value]) -> bool {
    let p = prompt.to_ascii_lowercase();
    tool_calls.iter().any(|tc| {
        let name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name != "sys_shell" {
            return false;
        }

        let cmd = tc
            .get("args")
            .and_then(|a| a.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();

        if !is_shell_runtime_execution_command(&cmd) {
            return false;
        }

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.len() < 3 {
            return false;
        }

        let runtime_args = &parts[2..];
        let contains_literal = runtime_args.iter().any(|arg| {
            arg.parse::<f64>().is_ok()
                || matches!(*arg, "+" | "-" | "*" | "/")
                || (*arg).len() >= 2
        });
        if !contains_literal {
            return false;
        }

        // Treat as invented if none of runtime args appear in user's prompt.
        !runtime_args.iter().any(|arg| p.contains(&arg.to_ascii_lowercase()))
    })
}

fn tool_calls_use_node_only_js_apis(tool_calls: &[Value]) -> bool {
    tool_calls.iter().any(|tc| {
        let name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name != "sys_shell" {
            return false;
        }

        let cmd = tc
            .get("args")
            .and_then(|a| a.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        // In this environment, `js` runs via sys.js in browser/WASM, so Node's
        // require/readline APIs are unavailable and indicate an invalid approach.
        (cmd.starts_with("write ") || cmd.starts_with("tee "))
            && (cmd.contains("require(") || cmd.contains("require ('") || cmd.contains("readline"))
    })
}

fn is_shell_runtime_execution_command(cmd: &str) -> bool {
    cmd.starts_with("js ")
        || cmd.starts_with("python ")
        || cmd.starts_with("python3 ")
        || cmd.starts_with("lua ")
        || cmd.starts_with("qjs ")
        || cmd.starts_with("bash ")
        || cmd.starts_with("sh ")
        || cmd.starts_with("./")
}

fn prompt_suggests_build_or_script(prompt: &str) -> bool {
    let p = prompt.to_ascii_lowercase();
    [
        "build",
        "create",
        "make",
        "script",
        "program",
        "calculator",
        "app",
        "tool",
        "javascript",
        "js",
        "code",
    ]
    .iter()
    .any(|kw| p.contains(kw))
}

fn tool_calls_have_build_actions(tool_calls: &[Value]) -> bool {
    tool_calls.iter().any(|tc| {
        let name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = tc.get("args").cloned().unwrap_or(Value::Null);

        if name == "sys_vfs" {
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            return action == "write" || action == "append";
        }

        if name == "sys_shell" {
            let cmd = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            return cmd.starts_with("write ")
                || cmd.starts_with("tee ")
                || cmd.starts_with("cat >")
                || cmd.contains(" > ")
                || cmd.contains(" >> ");
        }

        false
    })
}

fn tool_calls_have_run_or_render(tool_calls: &[Value]) -> bool {
    tool_calls.iter().any(|tc| {
        let name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = tc.get("args").cloned().unwrap_or(Value::Null);

        if name == "sys_canvas" {
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            return action == "set" || action == "append" || action == "load";
        }

        if name == "sys_js" {
            return true;
        }

        if name == "sys_shell" {
            let cmd = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();

            // Explicit non-execution inspection commands should not count.
            let is_inspection = cmd.starts_with("cat ")
                || cmd.starts_with("ls")
                || cmd.starts_with("find ")
                || cmd.starts_with("grep ")
                || cmd.starts_with("head ")
                || cmd.starts_with("tail ")
                || cmd.starts_with("wc ");
            if is_inspection {
                return false;
            }

            return cmd.starts_with("node ")
                || cmd.starts_with("deno ")
                || cmd.starts_with("bun ")
                || cmd.starts_with("python ")
                || cmd.starts_with("python3 ")
                || cmd.starts_with("lua ")
                || cmd.starts_with("js ")
                || cmd.starts_with("qjs ")
                || cmd.starts_with("bash ")
                || cmd.starts_with("sh ")
                || cmd.starts_with("./")
                || cmd.starts_with("canvas ")
                || cmd.starts_with("traits canvas ")
                || cmd.starts_with("npm run ")
                || cmd.starts_with("pnpm ")
                || cmd.starts_with("yarn ")
                || cmd.starts_with("cargo run");
        }

        false
    })
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
    "sys.canvas",
    "sys.shell",
    "sys.js",
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
EXECUTION POLICY (STRICT):\n\
1) CANVAS-FIRST for interactive/data requests: if the user request is visual, interactive, data-driven, or benefits from in-browser rendering — build HTML/CSS/JS and render via sys.canvas with action=set in the same turn. Examples: calculators, weather, charts, dashboards, forms, games, simulations, any UI.\n\
2) CLI/TERMINAL ONLY for pure automation: use sys.shell when the task is a pure data transform, file processing, or backend automation with no visual/interactive component.\n\
3) BUILD-THEN-RENDER: after building, always either render to canvas (sys.canvas set) or run in terminal in the same turn. Never stop at file creation.\n\
4) REQUIRED FINAL STATUS LINE: end with exactly one concise line: FINAL: <built artifact> | RAN: <terminal|canvas> | RESULT: <outcome>.\n\n\
CANVAS WORKFLOW: to create a calculator, weather app, form, chart, or any visual/interactive thing:\n\
  a) Write a complete HTML document (<!DOCTYPE html>...) with inline CSS + JS.\n\
  b) Call sys.canvas with action=\"set\" and the full HTML as content.\n\
  Do NOT use sys.shell to write files and run JS for visual tasks — use canvas instead.\n\n\
JS EXECUTION RULE (CRITICAL): node, deno, and bun are NOT installed and will never work. \
To run JS: call the sys.js tool directly, or sys.shell with command \"js scripts/app.js\". \
INPUT AUTHENTICITY RULE: if a task requires user-provided inputs that are missing, ask a concise clarification. Do not invent demo values.\n\n\
CANVAS: The file `canvas/app.html` on the VFS is rendered live on the /canvas page in the browser. \
For visual requests, complete creation and rendering in one turn: after writing `canvas/app.html`, immediately call sys.canvas with action `set` and the same HTML content. Do not stop after file creation unless the user explicitly asks to skip rendering. \
Prefer dark backgrounds (#0a0a0a) and light text (#e0e0e0) to match the site theme.\n\n\
CANVAS RENDERING RULES (your HTML is injected into a container div, NOT a standalone page):\n\
- Your <script> runs inside a new Function() wrapper with access to document and global scope.\n\
- HTML is placed inside <div id=\"canvas-container\">. Use document.querySelector('#canvas-container canvas') to find your canvas element.\n\
- NEVER use document.getElementById to find your canvas — use querySelector on the container instead.\n\
- Use `let` for variables you reassign in loops, NEVER const. Reassigning a const crashes the script silently.\n\
- For animation loops, store the rAF ID: window.__canvasAnimId = requestAnimationFrame(loop);\n\
- Keep scripts simple: get canvas from container, draw, animate. No DOMContentLoaded listeners.\n\n\
The canvas page injects a `window.traits` object your scripts can use: \
traits.call(path, args), traits.list(), traits.canvas(action, content), traits.echo(text), traits.audio(action, ...).";



const MAX_STEPS_LIMIT: usize = 50;

/// Compaction: keep the last N messages verbatim when compacting.
const COMPACT_PRESERVE_RECENT: usize = 4;

/// Compaction: trigger when estimated tokens exceed this threshold.
const COMPACT_MAX_TOKENS: usize = 10_000;

