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
use crate::dispatcher::{CallConfig, Dispatcher};
use crate::types::{CallRequest, CallResponse, TraitValue};
use tracing::info;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

struct AppState {
    dispatcher: Dispatcher,
    start_time: std::time::Instant,
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

    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "version": env!("TRAITS_BUILD_VERSION"),
        "trait_count": trait_count,
        "namespace_count": namespace_count,
        "uptime_human": uptime_human,
        "uptime_seconds": uptime_secs
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
        Some((content, content_type)) => HttpResponse::Ok()
            .content_type(content_type)
            .insert_header(("Cache-Control", "public, max-age=3600"))
            .body(content.to_vec()),
        None => HttpResponse::NotFound()
            .content_type("text/plain")
            .body("WASM asset not found"),
    }
}

/// Serve pages by resolving keyed interface bindings from sys.serve's [requires]/[bindings].
/// Each key is a URL path (e.g. "/", "/admin"), resolved to a page trait.
async fn serve_page(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let url_path = req.path();

    // Protect /admin and /llm-test paths with HTTP Basic Auth
    if url_path.starts_with("/admin") || url_path.starts_with("/llm-test") {
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
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
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

    let state = web::Data::new(AppState { dispatcher, start_time: std::time::Instant::now() });

    info!("Starting Traits server on {}:{} ({} page routes)", config.traits.bind, port, page_routes.len());

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(state.clone())
            .route("/health", web::get().to(health_check))
            .route("/metrics", web::get().to(metrics))
            .route("/traits", web::get().to(list_traits))
            .route("/traits/", web::get().to(list_traits))
            .route("/traits/{path:.*}", web::post().to(call_trait))
            .route("/traits/{path:.*}", web::get().to(get_trait_info))
            .route("/static/{path:.*}", web::get().to(serve_static))
            .route("/wasm/{path:.*}", web::get().to(serve_wasm_asset))
            .default_service(web::to(serve_page))
    })
    .workers(2)
    .bind(format!("{}:{}", config.traits.bind, port))?
    .run()
    .await?;

    Ok(())
}
