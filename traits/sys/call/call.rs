use serde_json::{json, Value};
use std::process::Command;

/// sys.call — Make outbound HTTP/REST API calls.
///
/// Args: [url, body?, auth_secret?, method?, headers?]
///
/// Uses curl subprocess (consistent with fly_api.rs pattern).
/// Auth tokens are resolved from the secrets store first, then env vars.
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

    // Resolve auth token from secrets store, then env var
    let auth_token = auth_secret.and_then(|secret_id| {
        let ctx = crate::dispatcher::compiled::secrets::SecretContext::resolve(&[secret_id]);
        if let Some(val) = ctx.get(secret_id) {
            return Some(val.to_string());
        }
        // Fall back to env var (uppercased, e.g. "openai_api_key" -> "OPENAI_API_KEY")
        let env_key = secret_id.to_uppercase();
        std::env::var(&env_key).ok()
    });

    // Build curl command
    let mut cmd = Command::new("curl");
    cmd.args(["-s", "-w", "\n%{http_code}", "-X", &method, &url]);
    cmd.args(["-H", "Content-Type: application/json"]);

    // Add auth header
    if let Some(ref token) = auth_token {
        let auth_val = format!("Authorization: Bearer {}", token);
        cmd.args(["-H", &auth_val]);
    }

    // Add custom headers
    if let Some(hdrs) = headers_arg {
        if let Some(obj) = hdrs.as_object() {
            for (k, v) in obj {
                let val = v.as_str().unwrap_or(&v.to_string()).to_string();
                let header = format!("{}: {}", k, val);
                cmd.args(["-H", &header]);
            }
        }
    }

    // Add request body
    let body_str;
    if let Some(b) = body {
        body_str = if b.is_string() {
            b.as_str().unwrap().to_string()
        } else {
            serde_json::to_string(b).unwrap_or_default()
        };
        cmd.args(["-d", &body_str]);
    }

    // Set timeout (30s connect, 120s total)
    cmd.args(["--connect-timeout", "30", "--max-time", "120"]);

    // Execute
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => return json!({ "ok": false, "error": format!("curl failed: {}", e) }),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return json!({ "ok": false, "error": format!("curl error: {}", stderr) });
    }

    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    // Last line is HTTP status code (from -w flag)
    let (response_body, status_code) = match raw.rfind('\n') {
        Some(pos) => {
            let body_part = &raw[..pos];
            let code_part = raw[pos + 1..].trim();
            let code = code_part.parse::<u16>().unwrap_or(0);
            (body_part.to_string(), code)
        }
        None => (raw.clone(), 0),
    };

    // Try to parse response as JSON
    let parsed_body = serde_json::from_str::<Value>(&response_body)
        .unwrap_or_else(|_| Value::String(response_body));

    let ok = (200..300).contains(&status_code);

    json!({
        "ok": ok,
        "status": status_code,
        "body": parsed_body
    })
}
