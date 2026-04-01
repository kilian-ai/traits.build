use serde_json::{json, Value};

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../context.rs"]
mod context;

/// llm.prompt.openai.ws — Streaming OpenAI-compatible inference (implements llm/prompt).
///
/// In the browser (WASM) this returns a dispatch sentinel that the JS SDK
/// intercepts and resolves via fetch() Server-Sent Events streaming.
/// On native it delegates to the OpenAI REST API synchronously via sys.call
/// (same behaviour as llm.prompt.openai, since streaming has no CLI value).
///
/// Implements the llm/prompt interface and is exchangeable with llm.prompt.webllm.
///
/// Args: [prompt, model?, context?]
pub fn openai_ws(args: &[Value]) -> Value {
    let prompt = match args.first().and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return json!({ "ok": false, "error": "prompt is required" }),
    };

    let model = args.get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("gpt-4o-mini")
        .to_string();

    let context_csv = args.get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();

    // ── WASM browser path ──────────────────────────────────────────────────
    // Return a sentinel: JS SDK handles the actual fetch() SSE stream.
    // Context files are resolved from the embedded VFS before dispatch.
    #[cfg(target_arch = "wasm32")]
    {
        let final_prompt = if !context_csv.is_empty() {
            let ctx = read_context_from_vfs(&context_csv);
            if ctx.is_empty() {
                prompt
            } else {
                format!("{ctx}{prompt}")
            }
        } else {
            prompt
        };

        return json!({
            "dispatch": "openai_stream",
            "prompt": final_prompt,
            "model": model,
        });
    }

    // ── Native path ────────────────────────────────────────────────────────
    // Call OpenAI REST API synchronously (same as llm.prompt.openai).
    // Streams aren't useful for CLI usage, so we just buffer the response.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut messages: Vec<Value> = Vec::new();

        if !context_csv.is_empty() {
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".into());
            let files = context::read_context_files(&context_csv, &cwd);
            let ctx = context::format_context(&files);
            if !ctx.is_empty() {
                messages.push(json!({"role": "system", "content": ctx}));
            }
        }

        messages.push(json!({"role": "user", "content": prompt}));

        let body = json!({
            "model": model,
            "messages": messages,
        });

        let call_args = vec![
            Value::String("https://api.openai.com/v1/chat/completions".into()),
            body,
            Value::String("openai_api_key".into()),
            Value::String("POST".into()),
        ];

        let result = kernel_logic::platform::dispatch("sys.call", &call_args)
            .unwrap_or_else(|| json!({"ok": false, "error": "sys.call not available"}));

        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        if !ok {
            let error = result.get("body")
                .and_then(|b| b.get("error"))
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("OpenAI API call failed");
            return json!({ "ok": false, "error": error });
        }

        let body_val = result.get("body").cloned().unwrap_or(Value::Null);
        let content = body_val.pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        json!(content)
    }
}

/// Read context files from the embedded WASM VFS using simple glob matching.
/// Returns a formatted XML context block, or empty string if nothing matched.
#[cfg(target_arch = "wasm32")]
fn read_context_from_vfs(patterns_csv: &str) -> String {
    let vfs = kernel_logic::platform::make_vfs();
    let all_paths = vfs.list();
    let mut files: Vec<(String, String)> = Vec::new();

    for pattern in patterns_csv.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        // Exact match first
        if let Some(content) = vfs.read(pattern) {
            let name = pattern.rsplit('/').next().unwrap_or(pattern);
            if !files.iter().any(|(n, _)| n == name) {
                files.push((name.to_string(), content));
            }
            continue;
        }

        // Glob-style suffix matching: "docs/*.md" matches any path ending in .md under docs/
        let (prefix, suffix) = if let Some(star) = pattern.find('*') {
            (&pattern[..star], &pattern[star + 1..])
        } else {
            ("", "")
        };

        if !prefix.is_empty() || !suffix.is_empty() {
            for path in &all_paths {
                if (prefix.is_empty() || path.starts_with(prefix))
                    && (suffix.is_empty() || path.ends_with(suffix))
                {
                    if let Some(content) = vfs.read(path) {
                        let name = path.rsplit('/').next().unwrap_or(path.as_str());
                        if !files.iter().any(|(n, _)| n == name) {
                            files.push((name.to_string(), content));
                        }
                    }
                }
            }
        }
    }

    if files.is_empty() {
        return String::new();
    }

    let mut ctx = String::from("<context>\n");
    for (name, content) in &files {
        ctx.push_str(&format!("<file name=\"{name}\">\n{content}\n</file>\n"));
    }
    ctx.push_str("</context>\n\n");
    ctx
}
