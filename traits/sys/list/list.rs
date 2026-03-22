use serde_json::Value;

/// sys.list — delegates to sys.registry "list" action.
pub fn list(args: &[Value]) -> Value {
    crate::dispatcher::compiled::registry::list(args)
}
