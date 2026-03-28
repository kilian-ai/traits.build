use serde_json::{json, Value};

/// sys.call — Make outbound HTTP/REST API calls.
///
/// Args: [url, body?, auth_secret?, method?, headers?]
///
/// Native: uses curl subprocess.
/// WASM: uses synchronous XmlHttpRequest.
/// Auth tokens resolved from secrets store (native) or WASM secret store.
pub fn call(args: &[Value]) -> Value {
    let url = match args.first().and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return json!({ "ok": false, "error": "url is required" }),
    };

    // Validate URL scheme
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return json!({ "ok": false, "error": "url must start with https:// or http://" });
    }

    let body = args.get(1).filter(|v| !v.is_null());
    let auth_secret = args.get(2).and_then(|v| v.as_str()).filter(|s| !s.is_empty());
    let method_arg = args.get(3).and_then(|v| v.as_str()).filter(|s| !s.is_empty());
    let headers_arg = args.get(4).filter(|v| !v.is_null() && v.is_object());

    // Determine HTTP method
    let method = match method_arg {
        Some(m) => m.to_uppercase(),
        None => if body.is_some() { "POST".into() } else { "GET".into() },
    };

    // Resolve auth token (platform-specific)
    let auth_token = resolve_auth_token(auth_secret);

    // Build request body string
    let body_str = body.map(|b| {
        if b.is_string() {
            b.as_str().unwrap().to_string()
        } else {
            serde_json::to_string(b).unwrap_or_default()
        }
    });

    // Execute HTTP request (platform-specific)
    execute_request(&url, &method, body_str.as_deref(), auth_token.as_deref(), headers_arg)
}

// ═══════════════════════════════════════════
// ── Native: curl subprocess ────────────────
// ═══════════════════════════════════════════

#[cfg(not(target_arch = "wasm32"))]
fn resolve_auth_token(auth_secret: Option<&str>) -> Option<String> {
    auth_secret.and_then(|secret_id| {
        let ctx = crate::dispatcher::compiled::secrets::SecretContext::resolve(&[secret_id]);
        if let Some(val) = ctx.get(secret_id) {
            return Some(val.to_string());
        }
        let env_key = secret_id.to_uppercase();
        std::env::var(&env_key).ok()
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn execute_request(
    url: &str,
    method: &str,
    body_str: Option<&str>,
    auth_token: Option<&str>,
    headers_arg: Option<&Value>,
) -> Value {
    use std::process::Command;

    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "-w", "\n%{http_code}", "-X", method, url]);
    cmd.args(["-H", "Content-Type: application/json"]);

    if let Some(token) = auth_token {
        let auth_val = format!("Authorization: Bearer {}", token);
        cmd.args(["-H", &auth_val]);
    }

    if let Some(hdrs) = headers_arg {
        if let Some(obj) = hdrs.as_object() {
            for (k, v) in obj {
                let val = v.as_str().unwrap_or(&v.to_string()).to_string();
                let header = format!("{}: {}", k, val);
                cmd.args(["-H", &header]);
            }
        }
    }

    if let Some(body) = body_str {
        cmd.args(["-d", body]);
    }

    cmd.args(["--connect-timeout", "30", "--max-time", "120"]);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => return json!({ "ok": false, "error": format!("curl failed: {}", e) }),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return json!({ "ok": false, "error": format!("curl error: {}", stderr) });
    }

    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    let (response_body, status_code) = match raw.rfind('\n') {
        Some(pos) => {
            let body_part = &raw[..pos];
            let code_part = raw[pos + 1..].trim();
            let code = code_part.parse::<u16>().unwrap_or(0);
            (body_part.to_string(), code)
        }
        None => (raw.clone(), 0),
    };

    let parsed_body = serde_json::from_str::<Value>(&response_body)
        .unwrap_or_else(|_| Value::String(response_body));

    let ok = (200..300).contains(&status_code);

    json!({
        "ok": ok,
        "status": status_code,
        "body": parsed_body
    })
}

// ═══════════════════════════════════════════
// ── WASM: XmlHttpRequest ───────────────────
// ═══════════════════════════════════════════

#[cfg(target_arch = "wasm32")]
fn resolve_auth_token(auth_secret: Option<&str>) -> Option<String> {
    auth_secret.and_then(|secret_id| {
        // Check the WASM in-memory secret store
        crate::wasm_secrets::get_secret(secret_id)
    })
}

#[cfg(target_arch = "wasm32")]
fn execute_request(
    url: &str,
    method: &str,
    body_str: Option<&str>,
    auth_token: Option<&str>,
    headers_arg: Option<&Value>,
) -> Value {
    use web_sys::XmlHttpRequest;

    let xhr = match XmlHttpRequest::new() {
        Ok(x) => x,
        Err(_) => return json!({ "ok": false, "error": "Failed to create XmlHttpRequest" }),
    };

    // Synchronous request
    if xhr.open_with_async(method, url, false).is_err() {
        return json!({ "ok": false, "error": "Failed to open XHR" });
    }

    let _ = xhr.set_request_header("Content-Type", "application/json");

    if let Some(token) = auth_token {
        let _ = xhr.set_request_header("Authorization", &format!("Bearer {}", token));
    }

    if let Some(hdrs) = headers_arg {
        if let Some(obj) = hdrs.as_object() {
            for (k, v) in obj {
                let val = v.as_str().unwrap_or(&v.to_string()).to_string();
                let _ = xhr.set_request_header(k, &val);
            }
        }
    }

    if xhr.send_with_opt_str(body_str).is_err() {
        return json!({ "ok": false, "error": "XHR send failed" });
    }

    let status = xhr.status().unwrap_or(0);
    let response_text = xhr.response_text().ok().flatten().unwrap_or_default();

    let parsed_body = serde_json::from_str::<Value>(&response_text)
        .unwrap_or_else(|_| Value::String(response_text));

    let ok = (200..300).contains(&(status as u16));

    json!({
        "ok": ok,
        "status": status,
        "body": parsed_body
    })
}
