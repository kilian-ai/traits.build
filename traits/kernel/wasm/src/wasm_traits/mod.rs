use serde_json::Value;

// ── Shared trait modules (same .rs files as native, compiled for wasm32) ──

#[path = "../../../../sys/checksum/checksum.rs"]
pub mod checksum;

#[path = "../../../../sys/registry/registry.rs"]
pub mod registry;

#[path = "../../../../sys/version/version.rs"]
pub mod version;

#[path = "../../../types/types.rs"]
pub mod types;

#[path = "../../../cli/cli.rs"]
pub mod cli;

/// WASM-callable trait paths (curated list of pure-computation traits).
pub const WASM_CALLABLE: &[&str] = &[
    "kernel.types",
    "sys.checksum",
    "sys.info",
    "sys.list",
    "sys.registry",
    "sys.version",
];

/// Dispatch a trait call by path. Returns None if the path isn't WASM-callable.
pub fn dispatch(trait_path: &str, args: &[Value]) -> Option<Value> {
    match trait_path {
        "kernel.types" => Some(types::types(args)),
        "sys.checksum" => Some(checksum::checksum_dispatch(args)),
        "sys.info" => Some(registry::info(args)),
        "sys.list" => Some(registry::list(args)),
        "sys.registry" => Some(registry::registry(args)),
        "sys.version" => Some(version::version(args)),
        _ => None,
    }
}
