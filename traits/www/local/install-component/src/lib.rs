//! www.local.install — Component Model implementation.
//! Returns the install.sh script content embedded at build time.

#[allow(warnings, clippy::all)]
mod bindings;

struct Component;

impl bindings::exports::traits::www_install::install::Guest for Component {
    fn install() -> Result<String, String> {
        Ok(include_str!("../../install/install.sh").to_string())
    }
}

bindings::export!(Component with_types_in bindings);
