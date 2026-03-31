use serde_json::{json, Value};

/// SPA session control — navigate pages, click elements, type text, query DOM.
/// Returns action descriptors that the browser JS bridge executes.
pub fn spa(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");

    match action {
        "navigate" => {
            let route = args.get(1).and_then(|v| v.as_str()).unwrap_or("/");
            json!({
                "ok": true,
                "spa_action": "navigate",
                "route": route
            })
        }
        "click" => {
            let selector = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            if selector.is_empty() {
                return json!({"ok": false, "error": "click requires a CSS selector"});
            }
            json!({
                "ok": true,
                "spa_action": "click",
                "selector": selector
            })
        }
        "type" => {
            let selector = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let text = args.get(2).and_then(|v| v.as_str()).unwrap_or("");
            if selector.is_empty() {
                return json!({"ok": false, "error": "type requires a CSS selector"});
            }
            json!({
                "ok": true,
                "spa_action": "type",
                "selector": selector,
                "text": text
            })
        }
        "terminal" => {
            let text = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            json!({
                "ok": true,
                "spa_action": "terminal",
                "text": text
            })
        }
        "query" => {
            let selector = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            if selector.is_empty() {
                return json!({"ok": false, "error": "query requires a CSS selector"});
            }
            json!({
                "ok": true,
                "spa_action": "query",
                "selector": selector
            })
        }
        "eval" => {
            let script = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            if script.is_empty() {
                return json!({"ok": false, "error": "eval requires a script string"});
            }
            json!({
                "ok": true,
                "spa_action": "eval",
                "script": script
            })
        }
        "route" => {
            // Return current route info — resolved by the JS bridge
            json!({
                "ok": true,
                "spa_action": "route"
            })
        }
        _ => json!({
            "ok": false,
            "error": format!("Unknown action: {}. Use: navigate, click, type, terminal, query, eval, route", action)
        }),
    }
}
