//! sys.echo — Component Model implementation.
//!
//! Pure-compute mirror of the cdylib in `traits/sys/echo`. Returns the
//! input text wrapped in `{ok: true, text: <input>}` as a JSON string.

#[allow(warnings, clippy::all)]
mod bindings;

struct Component;

impl bindings::exports::traits::sys_echo::echo::Guest for Component {
    fn echo(text: String) -> Result<String, String> {
        Ok(serde_json::json!({ "ok": true, "text": text }).to_string())
    }
}

bindings::export!(Component with_types_in bindings);
