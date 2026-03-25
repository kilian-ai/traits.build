use serde_json::Value;

// ── Shared trait modules (same .rs files as native, compiled for wasm32) ──
// These modules are shared with the native build. Many items are only used
// in the native context, so we suppress dead_code warnings for them here.

#[path = "../../../../sys/checksum/checksum.rs"]
pub mod checksum;

#[path = "../../../../sys/registry/registry.rs"]
pub mod registry;

#[path = "../../../../sys/version/version.rs"]
pub mod version;

#[allow(dead_code)]
#[path = "../../../types/types.rs"]
pub mod types;

#[allow(dead_code)]
#[path = "../../../cli/cli.rs"]
pub mod cli;

// ── WWW page traits (generate HTML, compiled for wasm32) ──

#[path = "../../../../www/traits/build/build.rs"]
pub mod www_build;

#[path = "../../../../www/docs/docs.rs"]
pub mod www_docs;

#[path = "../../../../www/docs/api/api.rs"]
pub mod www_docs_api;

#[path = "../../../../www/admin/admin.rs"]
pub mod www_admin;

#[path = "../../../../www/admin/spa/spa.rs"]
pub mod www_admin_spa;

#[path = "../../../../www/static/static.rs"]
pub mod www_static;

#[path = "../../../../sys/openapi/openapi.rs"]
pub mod openapi;

#[path = "../../../../sys/cli/wasm/wasm_impl.rs"]
pub mod wasm_impl;

/// WASM-callable trait paths (curated list of pure-computation traits).
pub const WASM_CALLABLE: &[&str] = &[
    "kernel.types",
    "sys.checksum",
    "sys.cli.wasm",
    "sys.info",
    "sys.list",
    "sys.openapi",
    "sys.registry",
    "sys.version",
    "www.traits.build",
    "www.docs",
    "www.docs.api",
    "www.admin",
    "www.admin.spa",
    "www.static",
];

/// Dispatch a trait call by path. Returns None if the path isn't WASM-callable.
pub fn dispatch(trait_path: &str, args: &[Value]) -> Option<Value> {
    match trait_path {
        "kernel.types" => Some(types::types(args)),
        "sys.checksum" => Some(checksum::checksum_dispatch(args)),
        "sys.cli.wasm" => Some(wasm_impl::wasm_dispatch(args)),
        "sys.info" => Some(registry::info(args)),
        "sys.list" => Some(registry::list(args)),
        "sys.openapi" => Some(openapi::openapi(args)),
        "sys.registry" => Some(registry::registry(args)),
        "sys.version" => Some(version::version(args)),
        "www.traits.build" => Some(www_build::website(args)),
        "www.docs" => Some(www_docs::docs(args)),
        "www.docs.api" => Some(www_docs_api::api_docs(args)),
        "www.admin" => Some(www_admin::admin(args)),
        "www.admin.spa" => Some(www_admin_spa::spa(args)),
        "www.static" => Some(www_static::static_page(args)),
        _ => None,
    }
}
