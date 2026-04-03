use serde_json::Value;

// ── Trait dispatch entry point ──

/// sys.serve trait dispatch — sync stub (background traits use start() instead).
pub fn serve(_args: &[Value]) -> Value {
    serde_json::json!({"error": "sys.serve is a background trait — dispatched via start()"})
}

/// Async entry point for background dispatch.
/// Called by the generic background trait mechanism in the dispatcher.
pub async fn start(args: &[crate::types::TraitValue]) -> Result<crate::types::TraitValue, Box<dyn std::error::Error + Send + Sync>> {
    let port = args.first().and_then(|v| match v {
        crate::types::TraitValue::Int(n) => Some(*n as u16),
        _ => None,
    });

    let config = match crate::globals::CONFIG.get() {
        Some(c) => c.clone(),
        None => return Err("No config available".into()),
    };

    let port = port.unwrap_or(config.traits.port);
    start_server(config, port).await.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
        e.to_string().into()
    })?;

    Ok(crate::types::TraitValue::Map({
        let mut m = std::collections::HashMap::new();
        m.insert("ok".into(), crate::types::TraitValue::Bool(true));
        m
    }))
}

// ── Full HTTP server implementation (moved from src/api.rs) ──

use actix_web::{web, App, HttpServer, HttpResponse, HttpRequest};
use actix_cors::Cors;
use actix_ws;
use crate::dispatcher::{CallConfig, Dispatcher};
use crate::types::{CallRequest, CallResponse, TraitValue};
use tracing::info;
use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;

struct RateLimiter {
    relay: DashMap<String, RateLimitState>,
    admin: DashMap<String, RateLimitState>,
    relay_limit: u32,
    admin_limit: u32,
    window_secs: u64,
}

struct RateLimitState {
    requests: Vec<Instant>,
    limit: u32,
}

impl RateLimiter {
    fn new(relay_limit: u32, admin_limit: u32) -> Self {
        Self {
            relay: DashMap::new(),
            admin: DashMap::new(),
            relay_limit,
            admin_limit,
            window_secs: 60,
        }
    }

    fn check(&self, key: &str, is_admin: bool) -> bool {
        let map = if is_admin { &self.admin } else { &self.relay };
        let limit = if is_admin { self.admin_limit } else { self.relay_limit };
        
        let mut state = map.entry(key.to_string()).or_insert_with(|| RateLimitState {
            requests: Vec::new(),
            limit,
        });

        let now = Instant::now();
        state.requests.retain(|t| now.duration_since(*t).as_secs() < self.window_secs);
        
        if state.requests.len() >= limit as usize {
            return false;
        }
        
        state.requests.push(now);
        true
    }
}

struct RateLimitData {
    limiter: Arc<RateLimiter>,
}

impl RateLimitData {
    fn new(relay_limit: u32, admin_limit: u32) -> Self {
        Self {
            limiter: Arc::new(RateLimiter::new(relay_limit, admin_limit)),
        }
    }
}

fn check_rate_limit(req: &HttpRequest, rate_data: &web::Data<RateLimitData>, is_admin: bool) -> Result<(), HttpResponse> {
    let client_ip = req.connection_info()
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    
    if !rate_data.limiter.check(&client_ip, is_admin) {
        return Err(HttpResponse::TooManyRequests()
            .content_type("text/plain")
            .body("Rate limit exceeded. Please try again later."));
    }
    Ok(())
}

fn check_admin_token(req: &HttpRequest) -> Result<(), HttpResponse> {
    let ctx = crate::dispatcher::compiled::secrets::SecretContext::resolve(&["admin_token"]);
    let token = if let Some(t) = ctx.get("admin_token") {
        t.to_string()
    } else {
        match std::env::var("ADMIN_TOKEN") {
            Ok(t) if !t.is_empty() => t,
            _ => return Err(HttpResponse::InternalServerError()
                .content_type("text/plain")
                .body("ADMIN_TOKEN not configured (set via secrets store or env var)")),
        }
    };

    let auth_header = match req.headers().get("Authorization") {
        Some(h) => h,
        None => return Err(HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Bearer realm=\"traits.build admin\""))
            .body("Authorization required")),
    };

    let auth_str = auth_header.to_str().unwrap_or("");
    let provided_token = if auth_str.starts_with("Bearer ") {
        &auth_str[7..]
    } else {
        auth_str
    };

    if provided_token == token {
        Ok(())
    } else {
        Err(HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Bearer realm=\"traits.build admin\""))
            .body("Invalid token"))
    }
}

struct AppState {
    dispatcher: Dispatcher,
    start_time: std::time::Instant,
}

// ══════════════════════════════════════════════════════════════
// Relay: NAT-traversal for remote helpers via pairing codes
// Phone → POST /relay/call → Fly.io relay → Mac (via long-poll)
// ══════════════════════════════════════════════════════════════

struct RelayState {
    sessions: DashMap<String, Arc<RelaySession>>,
}

struct RelaySession {
    request_tx: mpsc::Sender<RelayRequest>,
    request_rx: tokio::sync::Mutex<mpsc::Receiver<RelayRequest>>,
    response_txs: DashMap<String, oneshot::Sender<String>>,
    created: std::time::Instant,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct RelayRequest {
    id: String,
    path: String,
    args: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
struct RelayCallBody {
    code: String,
    path: String,
    args: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
struct RelayCodeQuery {
    code: String,
}

#[derive(Debug, serde::Deserialize)]
struct RelayRegisterBody {
    code: Option<String>,
}

impl RelayState {
    fn new() -> Self {
        Self { sessions: DashMap::new() }
    }

    fn generate_code(&self) -> String {
        loop {
            let id = uuid::Uuid::new_v4().to_string().replace('-', "");
            let code = id[..4].to_uppercase();
            if !self.sessions.contains_key(&code) {
                return code;
            }
        }
    }
}

fn normalize_relay_code(raw: &str) -> Option<String> {
    let code = raw.trim().to_uppercase();
    if code.len() == 4 && code.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(code)
    } else {
        None
    }
}

fn normalize_relay_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    // Migrate the legacy relay endpoint to the current dedicated relay domain.
    if trimmed.eq_ignore_ascii_case("https://traits-build.fly.dev") {
        return Some("https://relay.traits.build".to_string());
    }

    Some(trimmed.to_string())
}

fn ensure_repl_tty() -> bool {
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return true;
    }

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::fd::AsRawFd;

        let tty = match OpenOptions::new().read(true).write(true).open("/dev/tty") {
            Ok(f) => f,
            Err(_) => return false,
        };

        let fd = tty.as_raw_fd();
        let stdin_ok = unsafe { libc::dup2(fd, libc::STDIN_FILENO) } != -1;
        let stdout_ok = unsafe { libc::dup2(fd, libc::STDOUT_FILENO) } != -1;
        let stderr_ok = unsafe { libc::dup2(fd, libc::STDERR_FILENO) } != -1;

        stdin_ok && stdout_ok && stderr_ok && std::io::IsTerminal::is_terminal(&std::io::stdin())
    }

    #[cfg(not(unix))]
    {
        false
    }
}

#[derive(Debug, serde::Deserialize)]
struct CallQuery {
    #[serde(default)]
    stream: Option<String>,
}

/// POST /traits/{path...} — call a trait (supports ?stream=1 for SSE)
async fn call_trait(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<CallRequest>,
    query: web::Query<CallQuery>,
) -> HttpResponse {
    let raw_path = path.into_inner();
    let trait_path = raw_path.replace('/', ".");

    // Convert JSON args to TraitValues — supports both array and object (kwargs) form
    let args: Vec<TraitValue> = match &body.args {
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|v| serde_json::from_value::<TraitValue>(v.clone()).unwrap_or(TraitValue::Null))
            .collect(),
        serde_json::Value::Object(map) => {
            if let Some(entry) = state.dispatcher.registry().get(&trait_path) {
                entry.signature.params.iter().map(|p| {
                    let key_hy = p.name.replace('_', "-");
                    map.get(&p.name)
                        .or_else(|| map.get(&key_hy))
                        .map(|v| serde_json::from_value::<TraitValue>(v.clone()).unwrap_or(TraitValue::Null))
                        .unwrap_or(TraitValue::Null)
                }).collect()
            } else {
                vec![]
            }
        }
        _ => vec![],
    };

    let config = CallConfig::new(
        body.interface_overrides.clone().unwrap_or_default(),
        body.trait_overrides.clone().unwrap_or_default(),
    );

    let stream_mode = query.stream.as_deref()
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    if stream_mode {
        return call_trait_sse(state, &trait_path, args, &config).await;
    }

    match state.dispatcher.call(&trait_path, args, &config).await {
        Ok(result) => {
            let json_result = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
            HttpResponse::Ok().json(CallResponse { result: Some(json_result), error: None })
        }
        Err(crate::dispatcher::RouterError::NotFound(msg)) => {
            HttpResponse::NotFound().json(CallResponse { result: None, error: Some(msg) })
        }
        Err(crate::dispatcher::RouterError::ArgCount { expected, got }) => {
            HttpResponse::BadRequest().json(CallResponse {
                result: None,
                error: Some(format!("Expected {} arguments, got {}", expected, got)),
            })
        }
        Err(crate::dispatcher::RouterError::TypeMismatch { name, expected, got }) => {
            HttpResponse::BadRequest().json(CallResponse {
                result: None,
                error: Some(format!("Type mismatch for parameter '{}': expected {}, got {}", name, expected, got)),
            })
        }
        Err(crate::dispatcher::RouterError::Timeout(secs)) => {
            HttpResponse::GatewayTimeout().json(CallResponse {
                result: None,
                error: Some(format!("Trait call exceeded {}s timeout", secs)),
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(CallResponse {
            result: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn call_trait_sse(
    state: web::Data<AppState>,
    trait_path: &str,
    args: Vec<TraitValue>,
    config: &CallConfig,
) -> HttpResponse {
    let (tx, rx) = mpsc::channel::<TraitValue>(64);

    match state.dispatcher.call_stream(trait_path, args, tx, config).await {
        Ok(()) => {
            let stream = ReceiverStream::new(rx).map(|value| {
                let json = serde_json::to_string(&value).unwrap_or_else(|_| "null".into());
                Ok::<_, actix_web::Error>(
                    actix_web::web::Bytes::from(format!("data: {}\n\n", json))
                )
            });
            HttpResponse::Ok()
                .content_type("text/event-stream")
                .insert_header(("Cache-Control", "no-cache"))
                .insert_header(("Connection", "keep-alive"))
                .insert_header(("X-Accel-Buffering", "no"))
                .streaming(stream)
        }
        Err(crate::dispatcher::RouterError::NotFound(msg)) => {
            HttpResponse::NotFound().json(CallResponse { result: None, error: Some(msg) })
        }
        Err(e) => HttpResponse::InternalServerError().json(CallResponse {
            result: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn health_check(state: web::Data<AppState>) -> HttpResponse {
    let uptime_secs = state.start_time.elapsed().as_secs();
    let uptime_human = if uptime_secs >= 3600 {
        format!("{}h {}m {}s", uptime_secs / 3600, (uptime_secs % 3600) / 60, uptime_secs % 60)
    } else if uptime_secs >= 60 {
        format!("{}m {}s", uptime_secs / 60, uptime_secs % 60)
    } else {
        format!("{}s", uptime_secs)
    };

    let trait_count = match state.dispatcher.call(
        "sys.registry",
        vec![TraitValue::String("count".into())],
        &CallConfig::default(),
    ).await {
        Ok(TraitValue::Int(n)) => n as u64,
        _ => 0,
    };

    let namespace_count = match state.dispatcher.call(
        "sys.registry",
        vec![TraitValue::String("tree".into())],
        &CallConfig::default(),
    ).await {
        Ok(TraitValue::Map(m)) => m.len() as u64,
        _ => 0,
    };

    let relay_code = crate::globals::RELAY_CODE.read().ok().and_then(|g| g.clone());
    let relay_connected = crate::globals::RELAY_CONNECTED.load(std::sync::atomic::Ordering::Relaxed);
    let relay_url = crate::globals::RELAY_URL.get().cloned();

    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "version": env!("TRAITS_BUILD_VERSION"),
        "trait_count": trait_count,
        "namespace_count": namespace_count,
        "uptime_human": uptime_human,
        "uptime_seconds": uptime_secs,
        "relay": {
            "code": relay_code,
            "connected": relay_connected,
            "url": relay_url,
        }
    }))
}

async fn metrics(state: web::Data<AppState>) -> HttpResponse {
    match state.dispatcher.call(
        "sys.registry",
        vec![TraitValue::String("count".into())],
        &CallConfig::default(),
    ).await {
        Ok(TraitValue::Int(n)) => {
            HttpResponse::Ok()
                .content_type("text/plain")
                .body(format!(
                    "# HELP traits_total Total number of registered traits\n\
                     # TYPE traits_total gauge\n\
                     traits_total {}\n",
                    n
                ))
        }
        Ok(_) | Err(_) => {
            HttpResponse::InternalServerError()
                .content_type("text/plain")
                .body("# error reading trait count\n")
        }
    }
}

async fn list_traits(state: web::Data<AppState>) -> HttpResponse {
    match state.dispatcher.call(
        "sys.registry",
        vec![TraitValue::String("tree".into())],
        &CallConfig::default(),
    ).await {
        Ok(result) => {
            let json = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
            HttpResponse::Ok().json(json)
        }
        Err(e) => HttpResponse::InternalServerError().json(CallResponse {
            result: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn get_trait_info(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let trait_path = path.into_inner().replace('/', ".");

    if trait_path.is_empty() {
        return list_traits(state).await;
    }

    match state.dispatcher.call(
        "sys.registry",
        vec![TraitValue::String("info".into()), TraitValue::String(trait_path.clone())],
        &CallConfig::default(),
    ).await {
        Ok(result) => {
            if let TraitValue::Map(ref m) = result {
                if m.contains_key("error") {
                    let json = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
                    return HttpResponse::NotFound().json(json);
                }
            }
            let json = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
            HttpResponse::Ok().json(json)
        }
        Err(e) => HttpResponse::InternalServerError().json(CallResponse {
            result: None,
            error: Some(e.to_string()),
        }),
    }
}

/// Check HTTP Basic Auth against secrets store or ADMIN_PASSWORD env var.
/// Returns Ok(()) if auth is valid, Err(HttpResponse) with 401 if not.
fn check_basic_auth(req: &HttpRequest) -> Result<(), HttpResponse> {
    // Try secrets store first, then fall back to env var
    let ctx = crate::dispatcher::compiled::secrets::SecretContext::resolve(&["admin_password"]);
    let password = if let Some(p) = ctx.get("admin_password") {
        p.to_string()
    } else {
        match std::env::var("ADMIN_PASSWORD") {
            Ok(p) if !p.is_empty() => p,
            _ => return Err(HttpResponse::InternalServerError()
                .content_type("text/plain")
                .body("ADMIN_PASSWORD not configured (set via secrets store or env var)")),
        }
    };

    let auth_header = match req.headers().get("Authorization") {
        Some(h) => h,
        None => return Err(HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Basic realm=\"traits.build admin\""))
            .body("Authentication required")),
    };

    let auth_str = auth_header.to_str().unwrap_or("");
    if !auth_str.starts_with("Basic ") {
        return Err(HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Basic realm=\"traits.build admin\""))
            .body("Authentication required"));
    }

    let decoded = match base64_decode(&auth_str[6..]) {
        Some(d) => d,
        None => return Err(HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Basic realm=\"traits.build admin\""))
            .body("Invalid credentials")),
    };

    // Expected format: "admin:<password>"
    if decoded == format!("admin:{}", password) {
        Ok(())
    } else {
        Err(HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Basic realm=\"traits.build admin\""))
            .body("Invalid credentials"))
    }
}

/// Simple base64 decode helper.
fn base64_decode(input: &str) -> Option<String> {
    // Manual base64 decode — avoids extra dependency
    let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let input = input.trim_end_matches('=');
    let mut bytes = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &c in input.as_bytes() {
        let val = table.iter().position(|&t| t == c)? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    String::from_utf8(bytes).ok()
}

/// Serve embedded static assets (.css, .js) discovered at build time from trait directories.
async fn serve_static(req: HttpRequest) -> HttpResponse {
    let path = req.match_info().get("path").unwrap_or("");
    match crate::dispatcher::static_assets::get_static_asset(path) {
        Some((content, content_type)) => HttpResponse::Ok()
            .content_type(content_type)
            .insert_header(("Cache-Control", "public, max-age=3600"))
            .body(content),
        None => HttpResponse::NotFound()
            .content_type("text/plain")
            .body("Static asset not found"),
    }
}

/// Serve WASM binary assets (wasm-pack output: .wasm, .js glue code).
async fn serve_wasm_asset(req: HttpRequest) -> HttpResponse {
    let path = req.match_info().get("path").unwrap_or("");
    match crate::dispatcher::wasm_static_assets::get_wasm_asset(path) {
        Some((content, content_type)) => {
            // ETag from content length + first 16 bytes for cache validation
            let etag = format!("\"{:x}-{}\"", content.len(),
                content.iter().take(16).fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64)));
            HttpResponse::Ok()
                .content_type(content_type)
                .insert_header(("Cache-Control", "no-cache"))
                .insert_header(("ETag", etag))
                .body(content.to_vec())
        }
        None => HttpResponse::NotFound()
            .content_type("text/plain")
            .body("WASM asset not found"),
    }
}

/// Serve the running binary for download.
/// Returns the current executable as application/octet-stream with platform headers.
async fn serve_binary() -> HttpResponse {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return HttpResponse::InternalServerError()
            .body("Cannot determine executable path"),
    };
    let binary = match std::fs::read(&exe) {
        Ok(b) => b,
        Err(_) => return HttpResponse::InternalServerError()
            .body("Cannot read executable"),
    };
    HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header(("X-Traits-OS", std::env::consts::OS))
        .insert_header(("X-Traits-Arch", std::env::consts::ARCH))
        .insert_header(("Content-Disposition", "attachment; filename=\"traits\""))
        .body(binary)
}

/// POST /admin/update — Self-update: download latest binary from GitHub releases,
/// replace /data/traits, and exit so Fly.io auto-restarts with the new binary.
/// Requires admin Bearer token auth.
async fn admin_update(
    req: HttpRequest,
    rate: web::Data<RateLimitData>,
) -> HttpResponse {
    if let Err(resp) = check_rate_limit(&req, &rate, true) {
        return resp;
    }
    
    if let Err(resp) = check_admin_token(&req) {
        return resp;
    }

    let repo = "kilian-ai/traits.build";
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // 1. Get latest tag from GitHub
    let tag_url = format!("https://api.github.com/repos/{}/tags?per_page=1", repo);
    let tag_output = match tokio::process::Command::new("curl")
        .args(["-fsSL", "--connect-timeout", "10", &tag_url])
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": "Failed to fetch tags from GitHub"})),
    };

    let latest = tag_output.lines()
        .find(|l| l.contains("\"name\""))
        .and_then(|l| {
            let start = l.find('"')? + 1;
            let rest = &l[start..];
            let start2 = rest.find('"')? + 1;
            let rest2 = &rest[start2..];
            let end = rest2.find('"')?;
            Some(rest2[..end].to_string())
        });

    let latest = match latest {
        Some(v) => v,
        None => return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": "No tags found on GitHub"})),
    };

    // 2. Check if we're already running this version
    let current = env!("TRAITS_BUILD_VERSION");
    if current == latest {
        return HttpResponse::Ok()
            .json(serde_json::json!({"status": "up-to-date", "version": current}));
    }

    // 3. Download the binary from GitHub release assets
    let binary_name = format!("traits-{}-{}", os, arch);
    let binary_url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        repo, latest, binary_name
    );

    let tmp_path = "/data/traits.update";
    let final_path = "/data/traits";

    let dl_result = tokio::process::Command::new("curl")
        .args(["-fsSL", "--connect-timeout", "30", "--max-time", "120",
               "-o", tmp_path, &binary_url])
        .output()
        .await;

    match dl_result {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({
                    "error": "Download failed",
                    "url": binary_url,
                    "detail": stderr.to_string()
                }));
        }
        Err(e) => return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": format!("curl failed: {}", e)})),
    };

    // 4. Verify downloaded file is non-empty and looks like an ELF binary
    match std::fs::metadata(tmp_path) {
        Ok(m) if m.len() > 1_000_000 => {} // binary should be >1MB
        Ok(m) => {
            let _ = std::fs::remove_file(tmp_path);
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({
                    "error": "Downloaded file too small",
                    "size": m.len(),
                    "url": binary_url
                }));
        }
        Err(e) => return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": format!("Cannot stat download: {}", e)})),
    }

    // Check ELF magic bytes
    if let Ok(bytes) = std::fs::read(tmp_path) {
        if bytes.len() < 4 || &bytes[..4] != b"\x7fELF" {
            let _ = std::fs::remove_file(tmp_path);
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "Downloaded file is not a valid ELF binary"}));
        }
    }

    // 5. Atomic replace: rename tmp → final
    if let Err(e) = std::fs::rename(tmp_path, final_path) {
        let _ = std::fs::remove_file(tmp_path);
        return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": format!("Failed to install binary: {}", e)}));
    }

    // Set executable permission
    let _ = std::process::Command::new("chmod").args(["+x", final_path]).status();

    // 6. Respond before exiting
    let response = serde_json::json!({
        "status": "updated",
        "from": current,
        "to": latest,
        "path": final_path,
        "restarting": true
    });

    // Schedule exit after a brief delay so the response gets sent
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        info!("Self-update complete: exiting for restart");
        std::process::exit(0);
    });

    HttpResponse::Ok().json(response)
}

/// Serve pages by resolving keyed interface bindings from sys.serve's [requires]/[bindings].
/// Each key is a URL path (e.g. "/", "/admin"), resolved to a page trait.
async fn serve_page(
    state: web::Data<AppState>,
    req: HttpRequest,
    rate: web::Data<RateLimitData>,
) -> HttpResponse {
    let url_path = req.path();

    // Rate limiting for /admin, /settings, and /llm-test paths
    if url_path.starts_with("/admin") || url_path.starts_with("/settings") || url_path.starts_with("/llm-test") {
        if let Err(resp) = check_rate_limit(&req, &rate, true) {
            return resp;
        }
        
        // Protect /admin, /settings, and /llm-test paths with HTTP Basic Auth
        if let Err(resp) = check_basic_auth(&req) {
            return resp;
        }
    }

    let trait_path = match state.dispatcher.resolve_keyed(url_path, "sys.serve") {
        Some(tp) => tp,
        None => return HttpResponse::NotFound()
            .content_type("text/html; charset=utf-8")
            .body("<h1>404</h1><p>No page trait bound for this path.</p>"),
    };

    match state.dispatcher.call(&trait_path, vec![], &CallConfig::default()).await {
        Ok(TraitValue::String(html)) => {
            // Detect shell scripts (shebang) and serve as text/plain
            let content_type = if html.starts_with("#!/") {
                "text/plain; charset=utf-8"
            } else {
                "text/html; charset=utf-8"
            };
            HttpResponse::Ok()
                .content_type(content_type)
                .insert_header(("Cache-Control", "no-cache"))
                .body(html)
        }
        Ok(other) => {
            let body = serde_json::to_string_pretty(&other.to_json()).unwrap_or_default();
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(body)
        }
        Err(e) => HttpResponse::InternalServerError()
            .content_type("text/plain")
            .body(format!("Page trait error: {}", e)),
    }
}

// ── Relay handlers ──

/// POST /relay/register — Reserve a pairing code
async fn relay_register(
    req: HttpRequest,
    relay: web::Data<Arc<RelayState>>,
    rate: web::Data<RateLimitData>,
    body: web::Bytes,
) -> HttpResponse {
    if let Err(resp) = check_rate_limit(&req, &rate, false) {
        return resp;
    }
    
    let requested_code = if body.is_empty() {
        None
    } else {
        serde_json::from_slice::<RelayRegisterBody>(&body)
            .ok()
            .and_then(|payload| payload.code)
            .and_then(|code| normalize_relay_code(&code))
    };
    let code = requested_code.unwrap_or_else(|| relay.generate_code());
    let (tx, rx) = mpsc::channel::<RelayRequest>(32);
    let session = Arc::new(RelaySession {
        request_tx: tx,
        request_rx: tokio::sync::Mutex::new(rx),
        response_txs: DashMap::new(),
        created: std::time::Instant::now(),
    });
    relay.sessions.insert(code.clone(), session);
    info!("Relay session created: {}", code);
    HttpResponse::Ok().json(serde_json::json!({ "code": code }))
}

/// GET /relay/poll?code=XXXX — Mac long-polls for next request
async fn relay_poll(
    req: HttpRequest,
    relay: web::Data<Arc<RelayState>>,
    rate: web::Data<RateLimitData>,
    query: web::Query<RelayCodeQuery>,
) -> HttpResponse {
    if let Err(resp) = check_rate_limit(&req, &rate, false) {
        return resp;
    }
    
    let session = match relay.sessions.get(&query.code) {
        Some(s) => s.clone(),
        None => return HttpResponse::NotFound().json(serde_json::json!({"error": "Invalid code"})),
    };
    drop(relay); // Release DashMap ref before blocking

    let mut rx = session.request_rx.lock().await;
    match tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await {
        Ok(Some(req)) => HttpResponse::Ok().json(&req),
        Ok(None) => HttpResponse::Gone().json(serde_json::json!({"error": "Session closed"})),
        Err(_) => HttpResponse::NoContent().finish(), // Timeout — Mac should retry
    }
}

/// POST /relay/call — Phone sends trait call, waits for Mac response
async fn relay_call_handler(
    req: HttpRequest,
    relay: web::Data<Arc<RelayState>>,
    rate: web::Data<RateLimitData>,
    body: web::Json<RelayCallBody>,
) -> HttpResponse {
    if let Err(resp) = check_rate_limit(&req, &rate, false) {
        return resp;
    }
    
    let session = match relay.sessions.get(&body.code) {
        Some(s) => s.clone(),
        None => return HttpResponse::NotFound().json(serde_json::json!({
            "error": "Invalid pairing code",
            "result": serde_json::Value::Null,
        })),
    };
    drop(relay);

    let id = uuid::Uuid::new_v4().to_string();
    let request = RelayRequest {
        id: id.clone(),
        path: body.path.clone(),
        args: body.args.clone(),
    };

    let (tx, rx) = oneshot::channel::<String>();
    session.response_txs.insert(id.clone(), tx);

    if session.request_tx.send(request).await.is_err() {
        session.response_txs.remove(&id);
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Helper not connected",
            "result": serde_json::Value::Null,
        }));
    }

    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(response_json)) => {
            match serde_json::from_str::<serde_json::Value>(&response_json) {
                Ok(v) => HttpResponse::Ok().json(serde_json::json!({
                    "result": v.get("result").cloned(),
                    "error": v.get("error").and_then(|e| e.as_str()),
                })),
                Err(_) => HttpResponse::Ok().body(response_json),
            }
        }
        Ok(Err(_)) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Helper disconnected",
            "result": serde_json::Value::Null,
        })),
        Err(_) => HttpResponse::GatewayTimeout().json(serde_json::json!({
            "error": "Relay timeout (30s)",
            "result": serde_json::Value::Null,
        })),
    }
}

/// POST /relay/respond — Mac sends result back
async fn relay_respond(
    req: HttpRequest,
    relay: web::Data<Arc<RelayState>>,
    rate: web::Data<RateLimitData>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    if let Err(resp) = check_rate_limit(&req, &rate, false) {
        return resp;
    }
    
    let code = match body.get("code").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return HttpResponse::BadRequest().json(serde_json::json!({"error": "Missing code"})),
    };
    let id = match body.get("id").and_then(|v| v.as_str()) {
        Some(i) => i.to_string(),
        None => return HttpResponse::BadRequest().json(serde_json::json!({"error": "Missing id"})),
    };

    let session = match relay.sessions.get(&code) {
        Some(s) => s.clone(),
        None => return HttpResponse::NotFound().json(serde_json::json!({"error": "Invalid code"})),
    };
    drop(relay);

    if let Some((_, tx)) = session.response_txs.remove(&id) {
        let _ = tx.send(body.to_string());
        HttpResponse::Ok().json(serde_json::json!({"ok": true}))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({"error": "No pending request with that id"}))
    }
}

/// GET /relay/status?code=XXXX — Check if a pairing code is active
async fn relay_status(
    req: HttpRequest,
    relay: web::Data<Arc<RelayState>>,
    rate: web::Data<RateLimitData>,
    query: web::Query<RelayCodeQuery>,
) -> HttpResponse {
    if let Err(resp) = check_rate_limit(&req, &rate, false) {
        return resp;
    }
    
    match relay.sessions.get(&query.code) {
        Some(s) => HttpResponse::Ok().json(serde_json::json!({
            "active": true,
            "code": query.code,
            "age_seconds": s.created.elapsed().as_secs(),
        })),
        None => HttpResponse::Ok().json(serde_json::json!({"active": false})),
    }
}

// ── Relay client (Mac connects to remote relay via curl) ──

fn spawn_relay_client(relay_url: String, local_port: u16) {
    let _ = crate::globals::RELAY_URL.set(relay_url.clone());
    tokio::spawn(async move {
        // Wait a moment for local server to start
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        info!("Connecting to relay at {}...", relay_url);
        loop {
            match relay_client_session(&relay_url, local_port).await {
                Ok(_) => info!("Relay session ended, reconnecting in 5s..."),
                Err(e) => info!("Relay error: {}, retrying in 5s...", e),
            }
            // Reset relay state on session end (expired, error, or clean close)
            crate::globals::RELAY_CONNECTED.store(false, std::sync::atomic::Ordering::Relaxed);
            if let Ok(mut guard) = crate::globals::RELAY_CODE.write() {
                *guard = None;
            }
            kernel_logic::platform::unregister_task("relay-client");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

async fn relay_client_session(relay_url: &str, local_port: u16) -> Result<(), String> {
    // 1. Register — always generate a fresh code (rotation is safe now that
    //    the client stores a signed token for reconnect via _syncRelayCodeFromHelper)
    let register_url = format!("{}/relay/register", relay_url);
    let output = tokio::process::Command::new("curl")
        .args(["-sf", "-X", "POST", &register_url])
        .output()
        .await
        .map_err(|e| format!("curl register failed: {}", e))?;

    if !output.status.success() {
        return Err(format!("Register failed (exit {})", output.status.code().unwrap_or(-1)));
    }

    let reg: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Invalid register response: {}", e))?;
    let code = reg["code"].as_str().ok_or("No code in response")?.to_string();

    // Publish pairing code to globals
    if let Ok(mut guard) = crate::globals::RELAY_CODE.write() {
        *guard = Some(code.clone());
    }

    info!("📡 Relay pairing code: {}", code);
    info!("   Enter this code at traits.build/#/settings to connect from anywhere");

    // Register relay client in task registry
    let relay_started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    kernel_logic::platform::register_task(
        "relay-client", "Relay Client", "service", relay_started,
        &format!("code={} → {}", code, relay_url),
    );

    // 2. Poll loop
    loop {
        let poll_url = format!("{}/relay/poll?code={}", relay_url, code);
        let output = tokio::process::Command::new("curl")
            .args(["-s", "-w", "\n%{http_code}", "--max-time", "35", &poll_url])
            .output()
            .await
            .map_err(|e| format!("curl poll failed: {}", e))?;

        let raw = String::from_utf8_lossy(&output.stdout);
        let (body, status) = match raw.rfind('\n') {
            Some(pos) => (raw[..pos].to_string(), raw[pos+1..].trim().to_string()),
            None => (String::new(), raw.trim().to_string()),
        };

        // curl exit 28 = timeout (Fly proxy may not relay 204), treat as retry
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            if code == 28 {
                // curl timeout — normal for long-poll, just retry
                continue;
            }
            return Err(format!("Poll failed (curl exit {})", code));
        }

        // 204 = no pending request (server-side timeout), retry
        if status == "204" || body.is_empty() {
            continue;
        }

        // 404/410 = session expired
        if status == "404" || status == "410" {
            return Err(format!("Session expired (HTTP {})", status));
        }

        let req: RelayRequest = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                info!("Invalid relay request: {}", e);
                continue;
            }
        };

        // Handle _ping (connection handshake from SPA)
        if req.path == "_ping" {
            crate::globals::RELAY_CONNECTED.store(true, std::sync::atomic::Ordering::Relaxed);
            info!("✅ Remote client connected via relay (code: {})", code);
            let pong = serde_json::json!({
                "code": code,
                "id": req.id,
                "result": "pong",
            });
            let pong_body = pong.to_string();
            let _ = tokio::process::Command::new("curl")
                .args(["-sf", "-X", "POST", "-H", "Content-Type: application/json",
                       "-d", &pong_body, &format!("{}/relay/respond", relay_url)])
                .output()
                .await;
            continue;
        }

        info!("Relay request: {} (id: {})", req.path, req.id);

        // 3. Dispatch via local HTTP server
        let local_url = format!("http://127.0.0.1:{}/traits/{}", local_port, req.path.replace('.', "/"));
        let dispatch_body = serde_json::json!({"args": req.args}).to_string();
        // Use -s (no -f) so we get the response body even on HTTP errors.
        // --max-time 25: prevent hanging if the local server is restarting or overloaded.
        let dispatch_output = tokio::process::Command::new("curl")
            .args(["-s", "--max-time", "25", "-w", "\n%{http_code}",
                   "-X", "POST", "-H", "Content-Type: application/json",
                   "-d", &dispatch_body, &local_url])
            .output()
            .await;

        let response = match dispatch_output {
            Ok(out) => {
                // Output format: <body>\n<http_code>
                let raw = String::from_utf8_lossy(&out.stdout);
                let (body_str, status_code) = raw.rsplit_once('\n')
                    .map(|(b, c)| (b, c.trim().parse::<u16>().unwrap_or(0)))
                    .unwrap_or((raw.as_ref(), 0));

                if status_code >= 200 && status_code < 300 {
                    let result: serde_json::Value = serde_json::from_str(body_str)
                        .unwrap_or(serde_json::Value::Null);
                    serde_json::json!({
                        "code": code,
                        "id": req.id,
                        "result": result.get("result").cloned().unwrap_or(result),
                    })
                } else {
                    // Extract error message from response body when available
                    let detail = serde_json::from_str::<serde_json::Value>(body_str)
                        .ok()
                        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(|s| s.to_string()))
                        .unwrap_or_else(|| format!("HTTP {}", status_code));
                    serde_json::json!({
                        "code": code,
                        "id": req.id,
                        "error": format!("Local dispatch failed: {}", detail),
                    })
                }
            }
            Err(e) => serde_json::json!({
                "code": code,
                "id": req.id,
                "error": format!("Local dispatch error: {}", e),
            }),
        };

        // 4. Respond
        let body_str = serde_json::to_string(&response).unwrap_or_default();
        let _ = tokio::process::Command::new("curl")
            .args(["-sf", "-X", "POST", "-H", "Content-Type: application/json",
                   "-d", &body_str, &format!("{}/relay/respond", relay_url)])
            .output()
            .await;
    }
}

fn spawn_relay_cleanup(relay: Arc<RelayState>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            let mut expired = vec![];
            for entry in relay.sessions.iter() {
                if entry.value().created.elapsed() > std::time::Duration::from_secs(3600) {
                    expired.push(entry.key().clone());
                }
            }
            for code in &expired {
                relay.sessions.remove(code);
            }
            if !expired.is_empty() {
                info!("Cleaned up {} expired relay sessions", expired.len());
            }
        }
    });
}

// ── MCP over WebSocket ──

/// WebSocket endpoint for MCP (Model Context Protocol) over JSON-RPC 2.0.
/// Clients connect at ws://host:port/mcp and send/receive JSON-RPC messages as text frames.
/// Each message is processed by the shared mcp::handle_message() handler.
async fn mcp_ws(req: HttpRequest, body: web::Payload) -> actix_web::Result<HttpResponse> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    actix_rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.recv().await {
            match msg {
                actix_ws::Message::Text(text) => {
                    let text_str = text.to_string();
                    // Spawn blocking because MCP dispatch calls compiled traits synchronously
                    let result = tokio::task::spawn_blocking(move || {
                        crate::dispatcher::compiled::mcp::handle_message(&text_str)
                    }).await;

                    if let Ok(Some(response)) = result {
                        let json_bytes = serde_json::to_vec(&response).unwrap_or_default();
                        let _ = session.text(
                            String::from_utf8(json_bytes).unwrap_or_default()
                        ).await;
                    }
                }
                actix_ws::Message::Ping(bytes) => {
                    let _ = session.pong(&bytes).await;
                }
                actix_ws::Message::Close(_) => break,
                _ => {}
            }
        }
    });

    Ok(response)
}

/// Start the HTTP server — called by the runtime when sys.serve is dispatched.
/// Uses the already-initialized globals (registry, config) instead of re-bootstrapping.
/// Resolves the www/website interface from sys.serve's [bindings] section.
pub async fn start_server(config: crate::config::Config, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let registry = crate::globals::REGISTRY.get()
        .expect("Registry must be initialized before starting server")
        .clone();
    let dispatcher = Dispatcher::new(registry, config.traits.timeout);

    // Resolve all keyed page routes from sys.serve's [requires]/[bindings]
    let page_routes = dispatcher.resolve_all_keyed("sys.serve");
    for (url_path, trait_path) in &page_routes {
        info!("Page route '{}' → {}", url_path, trait_path);
    }

    let state = web::Data::new(AppState { 
        dispatcher, 
        start_time: std::time::Instant::now(),
    });

    let rate_limit_data = web::Data::new(RateLimitData::new(
        config.traits.rate_limit_relay,
        config.traits.rate_limit_admin,
    ));
    let rate_limit_for_middleware = rate_limit_data.clone();

    // Relay state (shared across all workers)
    let relay = Arc::new(RelayState::new());
    let relay_data = web::Data::new(relay.clone());
    spawn_relay_cleanup(relay.clone());

    // If RELAY_URL is set (env var, or persistent config under sys.serve/global), connect as relay client.
    // Legacy values are normalized to the dedicated relay domain.
    let env_relay_url = std::env::var("RELAY_URL")
        .ok()
        .and_then(|v| normalize_relay_url(&v));

    let config_relay_url_raw = crate::config::trait_config_or("sys.serve", "RELAY_URL", "");
    let config_relay_url = normalize_relay_url(&config_relay_url_raw);

    if config_relay_url_raw.trim().eq_ignore_ascii_case("https://traits-build.fly.dev") {
        if let Err(e) = crate::config::write_persistent_config("sys.serve", "RELAY_URL", "https://relay.traits.build") {
            info!("Failed to migrate legacy relay URL config: {}", e);
        }
    }

    let relay_url = env_relay_url.or(config_relay_url);

    if let Some(relay_url) = relay_url {
        spawn_relay_client(relay_url, port);
    }

    info!("Starting Traits server on {}:{} ({} page routes)", config.traits.bind, port, page_routes.len());

    // Register in the platform task registry so sys.ps shows the server
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    kernel_logic::platform::register_task(
        "sys.serve", "HTTP Server", "service", now,
        &format!("{}:{}", config.traits.bind, port),
    );

    // Publish bind/port as globals so sys.info can report server status
    let _ = crate::globals::SERVER_BIND.set(config.traits.bind.clone());
    let _ = crate::globals::SERVER_PORT.set(port);

    let cors_origins = config.traits.cors_origins.clone();
    let is_local = config.traits.bind == "127.0.0.1" || config.traits.bind == "localhost";
    let server = HttpServer::new(move || {
        let origins = cors_origins.clone();
        let cors = if is_local {
            // Local helper: allow any origin (safe — only reachable from this machine)
            Cors::permissive()
        } else {
            let mut cors_builder = Cors::default()
                .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
                .allowed_headers(vec!["Content-Type", "Authorization", "Accept"])
                .max_age(3600);
            for origin in &origins {
                cors_builder = cors_builder.allowed_origin(origin);
            }
            cors_builder
        };

        App::new()
            .wrap(cors)
            .app_data(rate_limit_for_middleware.clone())
            .app_data(state.clone())
            .app_data(relay_data.clone())
            .route("/health", web::get().to(health_check))
            .route("/metrics", web::get().to(metrics))
            .route("/mcp", web::get().to(mcp_ws))
            .route("/relay/register", web::post().to(relay_register))
            .route("/relay/poll", web::get().to(relay_poll))
            .route("/relay/call", web::post().to(relay_call_handler))
            .route("/relay/respond", web::post().to(relay_respond))
            .route("/relay/status", web::get().to(relay_status))
            .route("/traits", web::get().to(list_traits))
            .route("/traits/", web::get().to(list_traits))
            .route("/traits/{path:.*}", web::post().to(call_trait))
            .route("/traits/{path:.*}", web::get().to(get_trait_info))
            .route("/static/{path:.*}", web::get().to(serve_static))
            .route("/wasm/{path:.*}", web::get().to(serve_wasm_asset))
            .route("/local/binary", web::get().to(serve_binary))
            .route("/admin/update", web::post().to(admin_update))
            .default_service(web::to(serve_page))
    })
    .workers(2)
    .bind(format!("{}:{}", config.traits.bind, port))?;

    // Spawn REPL when we have terminal IO. If stdin is piped, try reattaching /dev/tty.
    // Set TRAITS_NO_REPL=1 to run as a pure HTTP server without the interactive REPL.
    let no_repl = std::env::var("TRAITS_NO_REPL").map(|v| v == "1" || v == "true").unwrap_or(false);
    if !no_repl && ensure_repl_tty() {
        std::thread::spawn(|| {
            // Brief delay so the server's INFO log prints first
            std::thread::sleep(std::time::Duration::from_millis(200));
            crate::dispatcher::compiled::sys_cli::serve_repl();
        });
    } else {
        info!("REPL disabled: no interactive TTY detected");
    }

    server.run().await?;

    Ok(())
}
