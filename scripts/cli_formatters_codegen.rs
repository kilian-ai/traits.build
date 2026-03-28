use std::fs;
use std::path::Path;

pub fn write_cli_formatters(out_path: &Path, entries: &[(String, String, String)]) {
    let mut cf = String::new();
    for (trait_path, mod_name, abs_rs_path) in entries {
        cf.push_str(&format!(
            "#[path = {:?}]\npub mod {};\n\n",
            abs_rs_path, mod_name
        ));
        println!("cargo:rerun-if-changed={}", abs_rs_path);
        let _ = trait_path;
    }
    cf.push_str("/// Look up a CLI formatter for the given trait path.\n");
    cf.push_str("pub fn format_cli(trait_path: &str, result: &serde_json::Value) -> Option<String> {\n");
    cf.push_str("    match trait_path {\n");
    for (trait_path, mod_name, _abs_rs_path) in entries {
        cf.push_str(&format!(
            "        {:?} => Some({}::format_cli(result)),\n",
            trait_path, mod_name
        ));
    }
    cf.push_str("        _ => None,\n");
    cf.push_str("    }\n");
    cf.push_str("}\n");
    fs::write(out_path, cf).expect("Failed to write cli_formatters.rs");
}
