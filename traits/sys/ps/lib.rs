#[path = "ps.rs"]
mod ps_mod;

plugin_api::export_trait!(ps_mod::ps);
