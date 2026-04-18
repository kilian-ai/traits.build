use serde_json::{json, Value};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;

/// sys.js — execute JavaScript snippets.
///
/// WASM: runs code in the browser via Function constructor, with captured
/// console output.
/// Native: returns an unsupported error (no bundled JS engine).
///
/// Args:
///   [code, input?]
///
/// Returns:
///   { ok, stdout, stderr, result, error? }
pub fn js(args: &[Value]) -> Value {
	let code = args.first().and_then(|v| v.as_str()).unwrap_or("");
	let input = args.get(1).cloned().unwrap_or_else(|| json!({}));

	if code.trim().is_empty() {
		return json!({ "ok": false, "error": "code is required" });
	}

	#[cfg(target_arch = "wasm32")]
	{
		return run_in_browser_js(code, &input);
	}

	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = input;
		json!({
			"ok": false,
			"error": "sys.js is currently available in WASM/browser runtime only"
		})
	}
}

#[cfg(target_arch = "wasm32")]
fn run_in_browser_js(code: &str, input: &Value) -> Value {
	let call_trait = Closure::wrap(Box::new(move |path_js: JsValue, args_json_js: JsValue| -> JsValue {
		let path = match path_js.as_string() {
			Some(p) if !p.trim().is_empty() => p,
			_ => {
				return JsValue::from_str(
					"{\"__traits_error\":\"traits.call: path must be a non-empty string\"}",
				)
			}
		};

		let args_json = args_json_js.as_string().unwrap_or_else(|| "[]".to_string());
		let parsed_args = serde_json::from_str::<Value>(&args_json)
			.ok()
			.and_then(|v| v.as_array().cloned())
			.unwrap_or_default();

		let result = kernel_logic::platform::dispatch(&path, &parsed_args)
			.unwrap_or_else(|| json!({"ok": false, "error": format!("trait not found: {path}")}));
		JsValue::from_str(&result.to_string())
	}) as Box<dyn FnMut(JsValue, JsValue) -> JsValue>);

	let runner = js_sys::Function::new_with_args(
		"code,inputJson,callTrait",
		r#"
const stdout = [];
const stderr = [];
const input = (() => {
  try { return JSON.parse(inputJson || '{}'); }
  catch (_) { return {}; }
})();

const traits = {
  call(path, ...args) {
    const raw = callTrait(String(path), JSON.stringify(args));
    let parsed;
    try { parsed = JSON.parse(String(raw)); }
    catch (e) { throw new Error('traits.call: invalid bridge response: ' + String(e)); }
    if (parsed && parsed.__traits_error) {
      throw new Error(parsed.__traits_error);
    }
    return parsed;
  },
  // Alias for ergonomics in scripts that expect async-style naming.
  callAsync(path, ...args) {
    return Promise.resolve(this.call(path, ...args));
  },
};

const origLog = console.log;
const origErr = console.error;

console.log = (...args) => stdout.push(args.map(a => String(a)).join('\t'));
console.error = (...args) => stderr.push(args.map(a => String(a)).join('\t'));

let ok = true;
let result = null;
let error = null;

try {
	const fn = new Function('input', 'traits', code);
	const value = fn(input, traits);
  result = value === undefined ? null : value;
} catch (e) {
  ok = false;
  error = String((e && e.stack) ? e.stack : e);
}

console.log = origLog;
console.error = origErr;

return JSON.stringify({ ok, stdout, stderr, result, error });
"#,
	);

	let out = match runner.call3(
		&JsValue::NULL,
		&JsValue::from_str(code),
		&JsValue::from_str(&input.to_string()),
		call_trait.as_ref(),
	) {
		Ok(v) => v,
		Err(e) => {
			let err = e
				.as_string()
				.unwrap_or_else(|| "JavaScript runtime call failed".to_string());
			return json!({ "ok": false, "error": err });
		}
	};

	call_trait.forget();

	let out_str = out
		.dyn_ref::<js_sys::JsString>()
		.map(|s| String::from(s))
		.unwrap_or_else(|| {
			"{\"ok\":false,\"error\":\"invalid js runtime response\"}".to_string()
		});

	match serde_json::from_str::<Value>(&out_str) {
		Ok(v) => v,
		Err(e) => json!({ "ok": false, "error": format!("invalid js result json: {e}") }),
	}
}
