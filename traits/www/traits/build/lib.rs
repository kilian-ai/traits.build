#[path = "build.rs"]
mod build;

plugin_api::export_trait!(build::website);
