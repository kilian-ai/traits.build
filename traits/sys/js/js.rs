use serde_json::{json, Value};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};

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
	let runner = js_sys::Function::new_with_args(
		"code,inputJson",
		r#"
const stdout = [];
const stderr = [];
const input = (() => {
  try { return JSON.parse(inputJson || '{}'); }
  catch (_) { return {}; }
})();

const origLog = console.log;
const origErr = console.error;

console.log = (...args) => stdout.push(args.map(a => String(a)).join('\t'));
console.error = (...args) => stderr.push(args.map(a => String(a)).join('\t'));

let ok = true;
let result = null;
let error = null;

try {
  const fn = new Function('input', code);
  const value = fn(input);
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

	let out = match runner.call2(
		&JsValue::NULL,
		&JsValue::from_str(code),
		&JsValue::from_str(&input.to_string()),
	) {
		Ok(v) => v,
		Err(e) => {
			let err = e
				.as_string()
				.unwrap_or_else(|| "JavaScript runtime call failed".to_string());
			return json!({ "ok": false, "error": err });
		}
	};

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
