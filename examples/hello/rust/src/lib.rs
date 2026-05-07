#[allow(warnings)]
mod bindings;

use bindings::exports::hello::rust::hello::Guest;

struct Component;

impl Guest for Component {
    fn hello(name: String) -> Result<String, String> {
        // Return a JSON string — the host auto-decodes into a structured value
        // when the trait signature declares a non-string return type.
        Ok(format!(
            "{{\"greeting\":\"Hello, {}!\",\"language\":\"Rust\",\"toolchain\":\"cargo-component\"}}",
            escape_json(&name)
        ))
    }
}

bindings::export!(Component with_types_in bindings);

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
