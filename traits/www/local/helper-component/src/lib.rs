//! www.local.helper — Component Model implementation.
//! Returns the helper.sh script content embedded at build time.

#[allow(warnings, clippy::all)]
mod bindings;

struct Component;

impl bindings::exports::traits::www_helper::helper::Guest for Component {
    fn helper() -> Result<String, String> {
        Ok(include_str!("../../helper/helper.sh").to_string())
    }
}

bindings::export!(Component with_types_in bindings);
