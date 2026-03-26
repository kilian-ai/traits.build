use serde_json::{json, Value};

/// sys.bindings — Runtime binding management.
///
/// Hot-swap interface → implementation mappings at runtime.
/// Actions: set, get, list, clear
///
/// Args: [action, interface?, impl_path?]
pub fn bindings(args: &[Value]) -> Value {
    let action = match args.first().and_then(|v| v.as_str()) {
        Some(a) if !a.is_empty() => a,
        _ => return json!({
            "ok": false,
            "error": "action is required: set, get, list, clear"
        }),
    };

    let registry = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return json!({"ok": false, "error": "Registry not initialized"}),
    };

    match action {
        "set" => {
            let interface = match args.get(1).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                Some(i) => i,
                None => return json!({"ok": false, "error": "interface path is required for set"}),
            };
            let impl_path = match args.get(2).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                Some(p) => p,
                None => return json!({"ok": false, "error": "impl_path is required for set"}),
            };
            let previous = registry.get_binding(interface);
            registry.set_binding(interface, impl_path);
            json!({
                "ok": true,
                "action": "set",
                "interface": interface,
                "impl": impl_path,
                "previous": previous,
            })
        }
        "get" => {
            let interface = match args.get(1).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                Some(i) => i,
                None => return json!({"ok": false, "error": "interface path is required for get"}),
            };
            let binding = registry.get_binding(interface);
            json!({
                "ok": true,
                "action": "get",
                "interface": interface,
                "impl": binding,
            })
        }
        "list" => {
            let all = registry.all_bindings();
            json!({
                "ok": true,
                "action": "list",
                "bindings": all,
                "count": all.len(),
            })
        }
        "clear" => {
            let interface = match args.get(1).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                Some(i) => i,
                None => return json!({"ok": false, "error": "interface path is required for clear"}),
            };
            let removed = registry.remove_binding(interface);
            json!({
                "ok": true,
                "action": "clear",
                "interface": interface,
                "removed": removed,
            })
        }
        other => json!({
            "ok": false,
            "error": format!("Unknown action: {}. Use: set, get, list, clear", other),
        }),
    }
}
