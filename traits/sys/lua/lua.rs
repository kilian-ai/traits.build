use serde_json::{json, Value};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};

/// sys.lua — Execute Lua scripts.
///
/// WASM: delegates to Fengari JS bridge (`globalThis.__traitsLuaRun`).
/// Native: delegates to mlua (vendored Lua 5.4).
///
/// Args:
///   [code, input?]
///
/// Returns:
///   { ok, stdout, stderr, result, error? }
pub fn lua(args: &[Value]) -> Value {
    let code = match args.first().and_then(|v| v.as_str()) {
        Some(c) if !c.trim().is_empty() => c,
        _ => return json!({ "ok": false, "error": "code is required" }),
    };

    let input = args.get(1).cloned().unwrap_or_else(|| json!({}));

    #[cfg(target_arch = "wasm32")]
    {
        return run_in_browser_lua(code, &input);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        return run_native_lua(code, &input);
    }
}

// ── WASM: Fengari JS bridge ──

#[cfg(target_arch = "wasm32")]
fn run_in_browser_lua(code: &str, input: &Value) -> Value {
    let global = js_sys::global();

    let runner = match js_sys::Reflect::get(&global, &JsValue::from_str("__traitsLuaRun")) {
        Ok(v) => v,
        Err(_) => return json!({ "ok": false, "error": "Lua runtime bridge not found" }),
    };

    if !runner.is_function() {
        return json!({ "ok": false, "error": "Lua runtime bridge is not callable" });
    }

    let func: js_sys::Function = runner.unchecked_into();
    let input_json = input.to_string();

    let out = match func.call2(
        &JsValue::NULL,
        &JsValue::from_str(code),
        &JsValue::from_str(&input_json),
    ) {
        Ok(v) => v,
        Err(e) => {
            let err = e.as_string().unwrap_or_else(|| "Lua runtime call failed".to_string());
            return json!({ "ok": false, "error": err });
        }
    };

    let out_str = out
        .as_string()
        .unwrap_or_else(|| "{\"ok\":false,\"error\":\"invalid lua runtime response\"}".to_string());

    match serde_json::from_str::<Value>(&out_str) {
        Ok(v) => v,
        Err(e) => json!({ "ok": false, "error": format!("invalid lua result json: {e}") }),
    }
}

// ── Native: mlua (vendored Lua 5.4) ──

#[cfg(not(target_arch = "wasm32"))]
fn run_native_lua(code: &str, input: &Value) -> Value {
    use mlua::prelude::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    let lua = match Lua::new() {
        l => l,
    };

    let stdout_buf: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    // Override print to capture output
    let buf = stdout_buf.clone();
    let print_fn = lua.create_function(move |_, args: mlua::MultiValue| {
        let parts: Vec<String> = args
            .iter()
            .map(|v| match v {
                mlua::Value::String(s) => s.to_str().map(|b| b.to_string()).unwrap_or_default(),
                mlua::Value::Integer(n) => n.to_string(),
                mlua::Value::Number(n) => {
                    if *n == (*n as i64) as f64 {
                        format!("{}", *n as i64)
                    } else {
                        n.to_string()
                    }
                }
                mlua::Value::Boolean(b) => b.to_string(),
                mlua::Value::Nil => "nil".to_string(),
                other => format!("{:?}", other),
            })
            .collect();
        buf.borrow_mut().push(parts.join("\t"));
        Ok(())
    });

    let print_fn = match print_fn {
        Ok(f) => f,
        Err(e) => return json!({ "ok": false, "error": format!("failed to create print: {e}") }),
    };

    if let Err(e) = lua.globals().set("print", print_fn) {
        return json!({ "ok": false, "error": format!("failed to set print: {e}") });
    }

    // Expose input as __traits_input_json string
    let input_json = input.to_string();
    if let Err(e) = lua.globals().set("__traits_input_json", input_json.as_str()) {
        return json!({ "ok": false, "error": format!("failed to set input json: {e}") });
    }

    // Expose input as `input` table
    match json_to_lua(&lua, input) {
        Ok(v) => {
            let _ = lua.globals().set("input", v);
        }
        Err(e) => {
            return json!({ "ok": false, "error": format!("failed to set input: {e}") });
        }
    }

    // Expose top-level primitive keys as globals for convenience
    if let Some(obj) = input.as_object() {
        for (k, v) in obj {
            if !k.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false) {
                continue;
            }
            if !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            let _ = match v {
                Value::String(s) => lua.globals().set(k.as_str(), s.as_str()),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        lua.globals().set(k.as_str(), i)
                    } else if let Some(f) = n.as_f64() {
                        lua.globals().set(k.as_str(), f)
                    } else {
                        continue;
                    }
                }
                Value::Bool(b) => lua.globals().set(k.as_str(), *b),
                Value::Null => lua.globals().set(k.as_str(), mlua::Value::Nil),
                _ => continue,
            };
        }
    }

    // Execute user code
    let exec_err = match lua.load(code).exec() {
        Ok(()) => None,
        Err(e) => Some(format!("{e}")),
    };

    // Read __result convention
    let result = match lua.globals().get::<mlua::Value>("__result") {
        Ok(mlua::Value::String(s)) => {
            let s = s.to_str().map(|b| b.to_string()).unwrap_or_default();
            json!(s)
        }
        Ok(mlua::Value::Integer(n)) => json!(n),
        Ok(mlua::Value::Number(n)) => json!(n),
        Ok(mlua::Value::Boolean(b)) => json!(b),
        Ok(mlua::Value::Nil) | Err(_) => Value::Null,
        Ok(_) => Value::Null,
    };

    let stdout: Vec<String> = stdout_buf.borrow().clone();
    let stderr: Vec<String> = exec_err.iter().cloned().collect();

    if let Some(err) = exec_err {
        json!({ "ok": false, "error": err, "stdout": stdout, "stderr": stderr, "result": result })
    } else {
        json!({ "ok": true, "stdout": stdout, "stderr": stderr, "result": result })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn json_to_lua(lua: &mlua::Lua, value: &Value) -> mlua::Result<mlua::Value> {
    match value {
        Value::Null => Ok(mlua::Value::Nil),
        Value::Bool(b) => Ok(mlua::Value::Boolean(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(mlua::Value::Integer(i))
            } else {
                Ok(mlua::Value::Number(n.as_f64().unwrap_or(0.0)))
            }
        }
        Value::String(s) => Ok(mlua::Value::String(lua.create_string(s.as_str())?)),
        Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, v)?)?;
            }
            Ok(mlua::Value::Table(table))
        }
        Value::Object(map) => {
            let table = lua.create_table()?;
            for (k, v) in map {
                table.set(k.as_str(), json_to_lua(lua, v)?)?;
            }
            Ok(mlua::Value::Table(table))
        }
    }
}
