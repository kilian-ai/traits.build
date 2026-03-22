#[path = "website.rs"]
mod build;

plugin_api::export_trait!(build::website);
