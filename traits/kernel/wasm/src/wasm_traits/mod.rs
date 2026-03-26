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

#[path = "../../../../www/playground/playground.rs"]
pub mod www_playground;

#[path = "../../../../www/wasm/wasm.rs"]
pub mod www_wasm;

#[path = "../../../../www/chat_logs/chat_logs.rs"]
pub mod www_chat_logs;

#[path = "../../../../www/llm_test/llm_test.rs"]
pub mod www_llm_test;

#[path = "../../../../sys/openapi/openapi.rs"]
pub mod openapi;

#[path = "../../../../sys/test_runner/test_runner.rs"]
pub mod test_runner;

#[path = "../../../../sys/cli/wasm/wasm_impl.rs"]
pub mod wasm_impl;

#[path = "../../../call/call.rs"]
pub mod call;

#[path = "../../../../sys/call/call.rs"]
pub mod sys_call;

#[path = "../../../../sys/ps/wasm/wasm_ps.rs"]
pub mod wasm_ps;

#[path = "../../../../sys/llm/llm.rs"]
pub mod llm;

#[path = "../../../../llm/prompt/webllm/webllm.rs"]
pub mod webllm;

/// WASM-callable trait paths (curated list of pure-computation traits).
pub const WASM_CALLABLE: &[&str] = &[
    "kernel.call",
    "kernel.types",
    "sys.call",
    "sys.checksum",
    "sys.cli.wasm",
    "sys.info",
    "sys.list",
    "sys.llm",
    "sys.ps",
    "sys.ps.wasm",
    "sys.openapi",
    "sys.registry",
    "sys.test_runner",
    "sys.version",
    "llm.prompt.webllm",
    "www.admin",
    "www.admin.spa",
    "www.chat_logs",
    "www.docs",
    "www.docs.api",
    "www.llm_test",
    "www.playground",
    "www.static",
    "www.traits.build",
    "www.wasm",
];

/// Traits that have WASM fallbacks but prefer helper dispatch when available.
/// When a local helper (native binary) is connected, these traits emit a REST
/// sentinel so the terminal delegates to the helper — which has access to OS
/// processes, filesystem, network, and other native capabilities.
/// When no helper is connected, the WASM-local implementation runs instead.
pub const HELPER_PREFERRED: &[&str] = &[
    "sys.ps",
];

/// Dispatch a trait call by path. Returns None if the path isn't WASM-callable
/// or if the trait prefers native helper dispatch and a helper is connected.
pub fn dispatch(trait_path: &str, args: &[Value]) -> Option<Value> {
    // Helper-preferred traits delegate to native helper when connected
    if crate::is_helper_connected() && HELPER_PREFERRED.contains(&trait_path) {
        return None;
    }
    match trait_path {
        "kernel.call" => Some(call::call(args)),
        "kernel.types" => Some(types::types(args)),
        "sys.call" => Some(sys_call::call(args)),
        "sys.checksum" => Some(checksum::checksum_dispatch(args)),
        "sys.cli.wasm" => Some(wasm_impl::wasm_dispatch(args)),
        "sys.info" => Some(registry::info(args)),
        "sys.ps" | "sys.ps.wasm" => Some(wasm_ps::wasm_ps(args)),
        "sys.list" => Some(registry::list(args)),
        "sys.llm" => Some(llm::llm(args)),
        "llm.prompt.webllm" => Some(webllm::webllm(args)),
        "sys.openapi" => Some(openapi::openapi(args)),
        "sys.registry" => Some(registry::registry(args)),
        "sys.test_runner" => Some(test_runner::test_runner(args)),
        "sys.version" => Some(version::version(args)),
        "www.admin" => Some(www_admin::admin(args)),
        "www.admin.spa" => Some(www_admin_spa::spa(args)),
        "www.chat_logs" => Some(www_chat_logs::chat_logs(args)),
        "www.docs" => Some(www_docs::docs(args)),
        "www.docs.api" => Some(www_docs_api::api_docs(args)),
        "www.llm_test" => Some(www_llm_test::llm_test(args)),
        "www.playground" => Some(www_playground::playground(args)),
        "www.static" => Some(www_static::static_page(args)),
        "www.traits.build" => Some(www_build::website(args)),
        "www.wasm" => Some(www_wasm::wasm_page(args)),
        _ => None,
    }
}
