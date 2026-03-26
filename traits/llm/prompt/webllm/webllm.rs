use serde_json::{json, Value};

/// llm.prompt.webllm — In-browser LLM inference via WebLLM (WebGPU).
///
/// This trait is WASM-only. In the browser it delegates to the WebLLM JS engine
/// loaded in the page. On native builds it returns an error directing the user
/// to use llm.prompt.openai or sys.llm instead.
///
/// Args: [prompt, model?]
///   prompt: User message string (required)
///   model:  WebLLM model ID (optional, default picked by JS runtime)
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

    // In WASM, we return a dispatch sentinel that the browser SDK (traits.js)
    // intercepts and routes to the WebLLM engine loaded in the page.
    // The WASM kernel is synchronous, so actual async WebLLM inference
    // happens on the JS side via _callWebLLM() in the SDK.
    #[cfg(target_arch = "wasm32")]
    {
        json!({
            "dispatch": "webllm",
            "prompt": prompt,
            "model": model,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (prompt, model);
        json!({
            "ok": false,
            "error": "llm.prompt.webllm is only available in the browser (WebGPU). Use llm.prompt.openai or sys.llm instead."
        })
    }
}
