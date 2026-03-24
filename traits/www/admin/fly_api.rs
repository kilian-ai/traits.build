/// Shared Fly.io Machines API helpers for admin traits.
/// Uses curl via std::process::Command (curl is installed in the Docker image).

use std::process::Command;

const FLY_API_BASE: &str = "https://api.machines.dev/v1";
fn fly_app() -> String {
    crate::config::trait_config_or("www.admin", "fly_app", "polygrait-api")
}

pub struct FlyApi {
    token: String,
}

impl FlyApi {
    pub fn new() -> Result<Self, String> {
        // Try secrets store first, then fall back to env vars
        let ctx = crate::dispatcher::compiled::secrets::SecretContext::resolve(&["fly_api_token"]);
        let token = if let Some(t) = ctx.get("fly_api_token") {
            t.to_string()
        } else {
            std::env::var("FLY_API_TOKEN")
                .or_else(|_| std::env::var("FLY_ACCESS_TOKEN"))
                .map_err(|_| "FLY_API_TOKEN not set (checked secrets store + env vars)".to_string())?
        };
        if token.is_empty() {
            return Err("FLY_API_TOKEN is empty".to_string());
        }
        Ok(Self { token })
    }

    /// Build the Authorization header value.
    /// If the token already starts with "FlyV1 ", use it as-is.
    /// Otherwise, prefix with "Bearer ".
    fn auth_header(&self) -> String {
        if self.token.starts_with("FlyV1 ") {
            self.token.clone()
        } else {
            format!("Bearer {}", self.token)
        }
    }

    /// Execute a curl request and return (http_status, body).
    fn curl(&self, method: &str, path: &str, body: Option<&str>) -> Result<(u16, String), String> {
        let url = format!("{}/apps/{}{}", FLY_API_BASE, fly_app(), path);
        let auth = self.auth_header();
        let mut cmd = Command::new("curl");
        cmd.args(["-s", "-L", "-X", method, &url,
                  "-H", &format!("Authorization: {}", auth),
                  "-H", "Content-Type: application/json",
                  "-w", "\n__HTTP_STATUS__%{http_code}"]);
        if let Some(b) = body {
            cmd.args(["-d", b]);
        }
        let output = cmd.output().map_err(|e| format!("curl failed: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("curl process error: {}", stderr));
        }
        let raw = String::from_utf8_lossy(&output.stdout).to_string();
        // Split response body from status code suffix
        if let Some(pos) = raw.rfind("__HTTP_STATUS__") {
            let response_body = raw[..pos].trim_end().to_string();
            let status_str = &raw[pos + 15..];
            let status: u16 = status_str.trim().parse().unwrap_or(0);
            Ok((status, response_body))
        } else {
            Ok((0, raw))
        }
    }

    /// Check HTTP status and return body, or Err with diagnostic info.
    fn check_response(&self, method: &str, path: &str, status: u16, body: &str) -> Result<String, String> {
        if status == 0 {
            return Err(format!("{} {} — no HTTP status (network error?)", method, path));
        }
        if status >= 400 {
            let detail = if body.is_empty() { "(empty body)" } else { body };
            return Err(format!("{} {} — HTTP {} — {}", method, path, status, &detail[..detail.len().min(300)]));
        }
        if body.is_empty() {
            return Err(format!("{} {} — HTTP {} but empty body", method, path, status));
        }
        Ok(body.to_string())
    }

    /// GET request to Fly Machines API.
    pub fn get(&self, path: &str) -> Result<String, String> {
        let (status, body) = self.curl("GET", path, None)?;
        self.check_response("GET", path, status, &body)
    }

    /// POST request to Fly Machines API.
    pub fn post(&self, path: &str, body: &str) -> Result<String, String> {
        let (status, resp) = self.curl("POST", path, Some(body))?;
        self.check_response("POST", path, status, &resp)
    }

    /// DELETE request to Fly Machines API.
    pub fn delete(&self, path: &str) -> Result<String, String> {
        let (status, body) = self.curl("DELETE", path, None)?;
        self.check_response("DELETE", path, status, &body)
    }

    /// List all machines for the app.
    pub fn list_machines(&self) -> Result<serde_json::Value, String> {
        let body = self.get("/machines")?;
        serde_json::from_str(&body)
            .map_err(|e| format!("JSON parse error: {} — body: {}", e, &body[..body.len().min(200)]))
    }
}
