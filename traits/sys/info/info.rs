use serde_json::Value;

/// sys.info — delegates to sys.registry "info" action.
pub fn info(args: &[Value]) -> Value {
    crate::dispatcher::compiled::registry::info(args)
}
