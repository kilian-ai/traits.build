use serde_json::Value;

/// sys.info — delegates to sys.registry "info" action.
pub fn info(args: &[Value]) -> Value {
    #[cfg(not(target_arch = "wasm32"))]
    { crate::dispatcher::compiled::registry::info(args) }
    #[cfg(target_arch = "wasm32")]
    { super::registry::info(args) }
}
