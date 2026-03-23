/// Shared Fly.io Machines API helpers for admin traits.
/// Uses curl via std::process::Command (curl is installed in the Docker image).

use std::process::Command;

const FLY_API_BASE: &str = "https://api.machines.dev/v1";
fn fly_app() -> String {
    crate::globals::CONFIG.get()
        .map(|c| c.deploy.fly_app.clone())
        .unwrap_or_else(|| std::env::var("FLY_APP").unwrap_or_else(|_| "your-fly-app".into()))
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

    /// GET request to Fly Machines API.
    pub fn get(&self, path: &str) -> Result<String, String> {
        let url = format!("{}/apps/{}{}", FLY_API_BASE, fly_app(), path);
        let auth = self.auth_header();
        let output = Command::new("curl")
            .args(["-s", "-X", "GET", &url,
                   "-H", &format!("Authorization: {}", auth),
                   "-H", "Content-Type: application/json"])
            .output()
            .map_err(|e| format!("curl failed: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("curl error: {}", stderr));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// POST request to Fly Machines API.
    pub fn post(&self, path: &str, body: &str) -> Result<String, String> {
        let url = format!("{}/apps/{}{}", FLY_API_BASE, fly_app(), path);
        let auth = self.auth_header();
        let output = Command::new("curl")
            .args(["-s", "-X", "POST", &url,
                   "-H", &format!("Authorization: {}", auth),
                   "-H", "Content-Type: application/json",
                   "-d", body])
            .output()
            .map_err(|e| format!("curl failed: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("curl error: {}", stderr));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// DELETE request to Fly Machines API.
    pub fn delete(&self, path: &str) -> Result<String, String> {
        let url = format!("{}/apps/{}{}", FLY_API_BASE, fly_app(), path);
        let auth = self.auth_header();
        let output = Command::new("curl")
            .args(["-s", "-X", "DELETE", &url,
                   "-H", &format!("Authorization: {}", auth),
                   "-H", "Content-Type: application/json"])
            .output()
            .map_err(|e| format!("curl failed: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("curl error: {}", stderr));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// List all machines for the app.
    pub fn list_machines(&self) -> Result<serde_json::Value, String> {
        let body = self.get("/machines")?;
        serde_json::from_str(&body)
            .map_err(|e| format!("JSON parse error: {} — body: {}", e, &body[..body.len().min(200)]))
    }
}
