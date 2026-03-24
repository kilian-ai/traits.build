use serde_json::Value;

pub mod checksum;
pub mod version;

// kernel.types — include the standalone Rust file directly (zero crate:: deps)
#[path = "../../../types/types.rs"]
pub mod types;

/// WASM-callable trait paths (curated list of pure-computation traits).
pub const WASM_CALLABLE: &[&str] = &[
    "kernel.types",
    "sys.checksum",
    "sys.version",
];

/// Dispatch a trait call by path. Returns None if the path isn't WASM-callable.
pub fn dispatch(trait_path: &str, args: &[Value]) -> Option<Value> {
    match trait_path {
        "kernel.types" => Some(types::types(args)),
        "sys.checksum" => Some(checksum::checksum_dispatch(args)),
        "sys.version" => Some(version::version(args)),
        _ => None,
    }
}
