use serde_json::{json, Value};

#[path = "../context.rs"]
mod context;

/// llm.prompt.openai — OpenAI-compatible prompt→response inference.
///
/// Implements llm/prompt interface with optional context file injection.
/// Uses sys.call internally for HTTP communication.
///
/// Args: [prompt, model?, context?]
pub fn openai(args: &[Value]) -> Value {
    let prompt = match args.first().and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p,
        _ => return json!({ "ok": false, "error": "prompt is required" }),
    };

    let model = args.get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("gpt-4o-mini");

    let context_csv = args.get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    // Read context files and build messages
    let mut messages: Vec<Value> = Vec::new();

    if !context_csv.is_empty() {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".into());
        let files = context::read_context_files(context_csv, &cwd);
        let ctx = context::format_context(&files);
        if !ctx.is_empty() {
            messages.push(json!({"role": "system", "content": ctx}));
        }
    }

    messages.push(json!({"role": "user", "content": prompt}));

    let body = json!({
        "model": model,
        "messages": messages
    });

    let call_args = vec![
        Value::String("https://api.openai.com/v1/chat/completions".into()),
        body,
        Value::String("openai_api_key".into()),
        Value::String("POST".into()),
        Value::Null,
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

    let body = result.get("body").cloned().unwrap_or(Value::Null);
    let content = body.pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    json!(content)
}
