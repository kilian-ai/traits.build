use serde_json::{json, Value};
use std::sync::Mutex;

static CANVAS: Mutex<String> = Mutex::new(String::new());

pub fn canvas(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("get");

    match action {
        "set" => {
            let content = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let len = content.len();
            *CANVAS.lock().unwrap() = content.to_string();
            json!({"ok": true, "action": "set", "length": len})
        }
        "append" => {
            let content = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let mut canvas = CANVAS.lock().unwrap();
            canvas.push_str(content);
            let len = canvas.len();
            json!({"ok": true, "action": "append", "length": len})
        }
        "get" => {
            let canvas = CANVAS.lock().unwrap();
            json!({"ok": true, "content": &*canvas, "length": canvas.len()})
        }
        "clear" => {
            *CANVAS.lock().unwrap() = String::new();
            json!({"ok": true, "action": "clear"})
        }
        _ => json!({"ok": false, "error": format!("Unknown action: {}", action)}),
    }
}
