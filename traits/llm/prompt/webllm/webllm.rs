use serde_json::{json, Value};

/// llm.prompt.webllm — In-browser LLM inference via WebLLM (WebGPU).
///
/// This trait is WASM-only. In the browser it delegates to the WebLLM JS engine
/// loaded in the page. On native builds it returns an error directing the user
/// to use llm.prompt.openai or sys.llm instead.
///
/// Args: [prompt, model?, context?]
///   prompt:  User message string (required)
///   model:   WebLLM model ID (optional, default picked by JS runtime)
///   context: Comma-separated file paths or globs for context injection (optional)
///            In the browser, files are read from the embedded VFS (docs/*.md etc.)
///
/// Returns: JSON with { ok, content, model, provider } or error string
pub fn webllm(args: &[Value]) -> Value {
    let prompt = match args.first().and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p,
        _ => return json!({ "ok": false, "error": "prompt is required" }),
    };

    let model = args.get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    let context_csv = args.get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    // In WASM, we return a dispatch sentinel that the browser SDK (traits.js)
    // intercepts and routes to the WebLLM engine loaded in the page.
    // The WASM kernel is synchronous, so actual async WebLLM inference
    // happens on the JS side via _callWebLLM() in the SDK.
    //
    // Context files are resolved from the VFS (embedded docs) and prepended
    // to the prompt before dispatch — the JS SDK sees the enriched prompt.
    #[cfg(target_arch = "wasm32")]
    {
        let final_prompt = if !context_csv.is_empty() {
            let ctx = read_context_from_vfs(context_csv);
            if ctx.is_empty() {
                prompt.to_string()
            } else {
                format!("{ctx}{prompt}")
            }
        } else {
            prompt.to_string()
        };

        json!({
            "dispatch": "webllm",
            "prompt": final_prompt,
            "model": model,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (prompt, model, context_csv);
        json!({
            "ok": false,
            "error": "llm.prompt.webllm is only available in the browser (WebGPU). Use llm.prompt.openai or sys.llm instead."
        })
    }
}

/// Read context files from the VFS using simple glob matching.
/// Returns formatted XML context block, or empty string if no files matched.
#[cfg(target_arch = "wasm32")]
fn read_context_from_vfs(patterns_csv: &str) -> String {
    let vfs = kernel_logic::platform::make_vfs();
    let all_paths = vfs.list();
    let mut files: Vec<(String, String)> = Vec::new();

    for pattern in patterns_csv.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        // Try exact match first
        if let Some(content) = vfs.read(pattern) {
            let name = pattern.rsplit('/').next().unwrap_or(pattern);
            files.push((name.to_string(), content));
            continue;
        }
        // Glob match against all VFS paths
        for path in &all_paths {
            if simple_glob_match(pattern, path) {
                if let Some(content) = vfs.read(path) {
                    let name = path.rsplit('/').next().unwrap_or(path);
                    files.push((name.to_string(), content));
                }
            }
        }
    }

    if files.is_empty() {
        return String::new();
    }
    let mut out = String::from("<context>\n");
    for (name, content) in &files {
        out.push_str(&format!("<file name=\"{name}\">\n{content}\n</file>\n"));
    }
    out.push_str("</context>\n\n");
    out
}

/// Minimal glob matcher supporting `*` (any sequence) and `?` (any single char).
#[cfg(target_arch = "wasm32")]
fn simple_glob_match(pattern: &str, text: &str) -> bool {
    let pb = pattern.as_bytes();
    let tb = text.as_bytes();
    glob_match_inner(pb, tb)
}

#[cfg(target_arch = "wasm32")]
fn glob_match_inner(pattern: &[u8], text: &[u8]) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }
    match pattern[0] {
        b'*' => {
            // * matches zero or more characters
            glob_match_inner(&pattern[1..], text)
                || (!text.is_empty() && glob_match_inner(pattern, &text[1..]))
        }
        b'?' => {
            !text.is_empty() && glob_match_inner(&pattern[1..], &text[1..])
        }
        c => {
            !text.is_empty() && c == text[0] && glob_match_inner(&pattern[1..], &text[1..])
        }
    }
}
