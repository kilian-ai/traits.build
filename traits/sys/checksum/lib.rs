#[path = "checksum.rs"]
mod checksum_mod;

plugin_api::export_trait!(checksum_mod::checksum_dispatch);
