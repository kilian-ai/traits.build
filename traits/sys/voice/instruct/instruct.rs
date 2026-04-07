use serde_json::{json, Value};
use std::sync::Mutex;

/// The compiled-in default instructions.
const DEFAULT_INSTRUCTIONS: &str = include_str!("../realtime_instructions.md");

/// Config namespace for storing custom instructions (native only — uses sys.config).
const CONFIG_NS: &str = "sys.voice";
const CONFIG_KEY: &str = "custom_instructions";

/// In-memory custom instructions (works on both native and WASM).
/// On WASM this is the only store; on native it mirrors sys.config for fast reads.
static CUSTOM_INSTRUCTIONS: Mutex<Option<String>> = Mutex::new(None);

/// sys.voice.instruct — read, replace, or reset the voice agent instructions.
///
/// Actions:
///   get          — return current instructions (custom if set, else default)
///   set          — replace instructions entirely with provided text
///   reset        — remove custom override, revert to compiled-in default
///   append       — add text to the end of current instructions
///   build        — assemble full session instructions: agent + memory + history + instruct
///     arg1: agent (optional, e.g. "traits.build")
///     arg2: session_id (optional — injects recent chat history)
pub fn voice_instruct(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let text = args.get(1).and_then(|v| v.as_str()).unwrap_or("");

    match action {
        "get" => {
            let (instructions, source) = read_instructions();
            json!({ "ok": true, "instructions": instructions, "source": source })
        }
        "set" => {
            if text.is_empty() {
                return json!({ "ok": false, "error": "text is required for set" });
            }
            if text.len() > 10000 {
                return json!({ "ok": false, "error": "Instructions too long (max 10000 chars)" });
            }
            write_instructions(text);
            json!({ "ok": true, "action": "set", "length": text.len(), "source": "custom" })
        }
        "reset" => {
            clear_instructions();
            json!({ "ok": true, "action": "reset", "source": "default", "length": DEFAULT_INSTRUCTIONS.len() })
        }
        "append" => {
            if text.is_empty() {
                return json!({ "ok": false, "error": "text is required for append" });
            }
            let (current, _) = read_instructions();
            let new_text = format!("{}\n\n{}", current, text);
            if new_text.len() > 10000 {
                return json!({ "ok": false, "error": "Combined instructions too long (max 10000 chars)" });
            }
            write_instructions(&new_text);
            json!({ "ok": true, "action": "append", "length": new_text.len(), "source": "custom" })
        }
        "build" => {
            // arg1 = agent name (optional), arg2 = session_id (optional)
            let agent = text;
            let session_id = args.get(2).and_then(|v| v.as_str()).filter(|s| !s.is_empty());
            let instructions = build_instructions(agent, session_id);
            json!({ "ok": true, "instructions": instructions, "length": instructions.len() })
        }
        _ => json!({ "ok": false, "error": format!("Unknown action: '{}'. Use get, set, reset, append, or build.", action) }),
    }
}

/// Assemble full voice session instructions:
///   1. Agent context (if agent name provided)
///   2. Persistent memory notes from sys.voice.memory
///   3. Recent conversation history from sys.chat (if session_id provided)
///   4. Voice-specific instructions (custom if set, else compiled-in default)
///
/// This is the canonical single source of truth used by both:
///   - Native WebSocket voice (voice.rs)
///   - Browser WebRTC voice (traits.js via sdk.call('sys.voice.instruct', ['build', agent, session_id]))
pub fn build_instructions(agent: &str, session_id: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();

    // 1. Agent context
    if !agent.is_empty() {
        parts.push(format!(
            "You are operating as the \"{}\" coding agent on the traits.build platform. \
             The user is a developer who may ask about code, architecture, or technical topics. \
             Maintain awareness of this agent context in your responses.",
            agent
        ));
    }

    // 2. Persistent memory notes
    if let Some(result) = kernel_logic::platform::dispatch("sys.voice.memory", &[json!("list")]) {
        if let Some(notes) = result.get("notes").and_then(|v| v.as_array()) {
            let texts: Vec<&str> = notes
                .iter()
                .filter_map(|n| n.get("text").and_then(|v| v.as_str()))
                .collect();
            if !texts.is_empty() {
                let mut mem = String::from("Your persistent memory (facts you chose to remember):\n");
                for t in &texts {
                    mem.push_str(&format!("- {}\n", t));
                }
                parts.push(mem);
            }
        }
    }

    // 3. Recent conversation history from sys.chat
    if let Some(sid) = session_id {
        if let Some(result) =
            kernel_logic::platform::dispatch("sys.chat", &[json!("get"), json!(sid)])
        {
            if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                if let Some(messages) = result
                    .pointer("/session/messages")
                    .and_then(|v| v.as_array())
                {
                    let recent: Vec<&Value> = messages
                        .iter()
                        .rev()
                        .take(6)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    if !recent.is_empty() {
                        let mut ctx = String::from("Recent conversation context (for continuity):\n");
                        for msg in &recent {
                            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("?");
                            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            let short = if content.len() > 200 {
                                let mut end = 200;
                                while !content.is_char_boundary(end) { end -= 1; }
                                &content[..end]
                            } else {
                                content
                            };
                            ctx.push_str(&format!("  {}: {}\n", role, short));
                        }
                        parts.push(ctx);
                    }
                }
            }
        }
    }

    // 4. Voice-specific instructions (custom if set, else compiled-in default)
    let (instr, _) = read_instructions();
    parts.push(instr);

    parts.join("\n\n")
}

/// Read current instructions. Returns (text, "custom"|"default").
pub fn read_instructions() -> (String, &'static str) {
    // 1. Check in-memory override first (always available, including WASM)
    if let Ok(guard) = CUSTOM_INSTRUCTIONS.lock() {
        if let Some(ref custom) = *guard {
            return (custom.clone(), "custom");
        }
    }
    // 2. Try sys.config (native only — returns None on WASM)
    if let Some(custom) = kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("get"), json!(CONFIG_NS), json!(CONFIG_KEY)],
    ) {
        if let Some(val) = custom.get("value").and_then(|v| v.as_str()) {
            if !val.is_empty() {
                return (val.to_string(), "custom");
            }
        }
    }
    (DEFAULT_INSTRUCTIONS.to_string(), "default")
}

fn write_instructions(text: &str) {
    // Store in memory (works on both native and WASM)
    if let Ok(mut guard) = CUSTOM_INSTRUCTIONS.lock() {
        *guard = Some(text.to_string());
    }
    // Also persist to sys.config (native only — silently no-ops on WASM)
    kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("set"), json!(CONFIG_NS), json!(CONFIG_KEY), json!(text)],
    );
}

fn clear_instructions() {
    // Clear in-memory override
    if let Ok(mut guard) = CUSTOM_INSTRUCTIONS.lock() {
        *guard = None;
    }
    // Also clear from sys.config (native only)
    kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("delete"), json!(CONFIG_NS), json!(CONFIG_KEY)],
    );
}
