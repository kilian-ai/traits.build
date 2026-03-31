use serde_json::{json, Value};

/// sys.echo — print text to the screen.
///
/// Returns the text as-is. Used by the voice agent to display content
/// that doesn't work well spoken aloud: links, code snippets, file paths, etc.
pub fn echo(args: &[Value]) -> Value {
    let text = args
        .first()
        .and_then(|v| v.as_str())
        .unwrap_or("");

    json!({
        "ok": true,
        "text": text
    })
}
