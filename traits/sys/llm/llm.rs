use serde_json::{json, Value};

/// sys.llm — Unified LLM inference.
///
/// Routes to OpenAI API or a local model server (wgml, ollama, llama.cpp).
/// Uses sys.call internally for all HTTP communication.
///
/// Args: [prompt, provider?, model?, context?, local_url?]
pub fn llm(args: &[Value]) -> Value {
    let prompt = match args.first().and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return json!({ "ok": false, "error": "prompt is required" }),
    };

    let provider = args.get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("openai");

    let model = args.get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let context = args.get(3)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let local_url = args.get(4)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("http://127.0.0.1:8080");

    match provider {
        "openai" => call_openai(&prompt, model, context),
        "local" => call_local(&prompt, model, context, local_url),
        other => json!({ "ok": false, "error": format!("Unknown provider: {}", other) }),
    }
}

/// Dispatch to sys.call via platform abstraction.
fn dispatch_sys_call(args: &[Value]) -> Value {
    kernel_logic::platform::dispatch("sys.call", args)
        .unwrap_or_else(|| json!({"ok": false, "error": "sys.call not found"}))
}

/// Call OpenAI chat completions via sys.call
fn call_openai(prompt: &str, model: Option<&str>, context: Option<&str>) -> Value {
    let model_name = model.unwrap_or("gpt-4.1-nano");

    let mut messages = Vec::new();
    if let Some(ctx) = context {
        messages.push(json!({"role": "system", "content": ctx}));
    }
    messages.push(json!({"role": "user", "content": prompt}));

    let body = json!({
        "model": model_name,
        "messages": messages,
        "max_tokens": 4096
    });

    let call_args = vec![
        Value::String("https://api.openai.com/v1/chat/completions".into()),
        body,
        Value::String("openai_api_key".into()),
        Value::String("POST".into()),
        Value::Null,
    ];

    let result = dispatch_sys_call(&call_args);

    // Extract the response
    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if !ok {
        let error = result.get("body")
            .and_then(|b| b.get("error"))
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("OpenAI API call failed");
        return json!({ "ok": false, "provider": "openai", "model": model_name, "error": error });
    }

    let body = result.get("body").cloned().unwrap_or(Value::Null);

    // Extract chat completion content
    let content = body.pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let actual_model = body.get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(model_name);

    let mut resp = json!({
        "ok": true,
        "provider": "openai",
        "model": actual_model,
        "content": content
    });

    // Include usage if available
    if let Some(usage) = body.get("usage") {
        resp["usage"] = usage.clone();
    }

    resp
}

/// Call a local model server (wgml, ollama, llama.cpp, etc.) via sys.call.
/// Expects OpenAI-compatible /v1/chat/completions endpoint.
fn call_local(prompt: &str, model: Option<&str>, context: Option<&str>, base_url: &str) -> Value {
    let model_name = model.unwrap_or("default");

    let mut messages = Vec::new();
    if let Some(ctx) = context {
        messages.push(json!({"role": "system", "content": ctx}));
    }
    messages.push(json!({"role": "user", "content": prompt}));

    let body = json!({
        "model": model_name,
        "messages": messages,
        "max_tokens": 4096
    });

    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

    let call_args = vec![
        Value::String(url),
        body,
        Value::Null, // no auth secret for local
        Value::String("POST".into()),
        Value::Null,
    ];

    let result = dispatch_sys_call(&call_args);

    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if !ok {
        let status = result.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
        let error = result.get("body")
            .and_then(|b| b.get("error"))
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
            .or_else(|| result.get("error").and_then(|e| e.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| format!("Local server returned HTTP {}", status));
        return json!({ "ok": false, "provider": "local", "model": model_name, "error": error });
    }

    let body = result.get("body").cloned().unwrap_or(Value::Null);

    // Try OpenAI-compatible format first
    let content = body.pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        // Also try plain text response
        .or_else(|| body.get("response").and_then(|v| v.as_str()))
        .or_else(|| body.get("content").and_then(|v| v.as_str()))
        .unwrap_or("");

    let actual_model = body.get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(model_name);

    let mut resp = json!({
        "ok": true,
        "provider": "local",
        "model": actual_model,
        "content": content
    });

    if let Some(usage) = body.get("usage") {
        resp["usage"] = usage.clone();
    }

    resp
}
