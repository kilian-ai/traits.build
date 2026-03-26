use serde_json::Value;

/// sys.list — delegates to sys.registry "list" action.
pub fn list(args: &[Value]) -> Value {
    #[cfg(not(target_arch = "wasm32"))]
    { crate::dispatcher::compiled::registry::list(args) }
    #[cfg(target_arch = "wasm32")]
    { super::registry::list(args) }
}
