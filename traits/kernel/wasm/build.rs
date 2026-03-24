use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // traits/ is 3 levels up from traits/kernel/wasm/
    let traits_dir = manifest_dir.parent().unwrap().parent().unwrap().join("traits");
    let root_dir = manifest_dir.parent().unwrap().parent().unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=build.rs");
    watch_dirs_recursive(&traits_dir);

    let mut entries: Vec<(String, String)> = Vec::new(); // (trait_path, rel_from_root)
    let mut wasm_modules: Vec<WasmModule> = Vec::new();

    visit_traits(&traits_dir, root_dir, &traits_dir, &mut entries, &mut wasm_modules);

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    wasm_modules.sort_by(|a, b| a.trait_path.cmp(&b.trait_path));

    // ── Generate wasm_builtin_traits.rs (TOML definitions for registry) ──
    let mut bt = String::new();
    bt.push_str("pub const BUILTIN_TRAIT_DEFS: &[(&str, &str, &str)] = &[\n");
    for (path, rel_path) in &entries {
        let toml_abs = root_dir.join(rel_path);
        bt.push_str(&format!(
            "    ({:?}, {:?}, include_str!({:?})),\n",
            path, rel_path, toml_abs.to_string_lossy()
        ));
    }
    bt.push_str("];\n");
    fs::write(out_dir.join("wasm_builtin_traits.rs"), bt).expect("write builtin_traits");

    // ── Generate wasm_dispatch.rs (module inclusions + sync dispatch) ──
    let mut d = String::new();
    d.push_str("// Auto-generated: WASM-safe trait modules + dispatch\n\n");

    for m in &wasm_modules {
        let abs_path = root_dir.join(&m.rs_rel_path);
        d.push_str(&format!(
            "#[path = {:?}]\nmod {};\n\n",
            abs_path.to_string_lossy(), m.mod_name
        ));
        println!("cargo:rerun-if-changed={}", abs_path.display());
    }

    d.push_str("pub fn dispatch_wasm(trait_path: &str, args: &[serde_json::Value]) -> Option<serde_json::Value> {\n");
    d.push_str("    match trait_path {\n");
    for m in &wasm_modules {
        let func = if m.entry == "checksum" {
            format!("{}::checksum_dispatch", m.mod_name)
        } else {
            format!("{}::{}", m.mod_name, m.entry)
        };
        d.push_str(&format!(
            "        {:?} => Some({}(args)),\n",
            m.trait_path, func
        ));
    }
    d.push_str("        _ => None,\n");
    d.push_str("    }\n");
    d.push_str("}\n\n");

    d.push_str("pub fn list_wasm_callable() -> &'static [&'static str] {\n");
    d.push_str("    &[\n");
    for m in &wasm_modules {
        d.push_str(&format!("        {:?},\n", m.trait_path));
    }
    d.push_str("    ]\n");
    d.push_str("}\n");
    fs::write(out_dir.join("wasm_dispatch.rs"), d).expect("write wasm_dispatch");
}

struct WasmModule {
    trait_path: String,
    mod_name: String,
    entry: String,
    rs_rel_path: String,
}

/// WASM-unsafe crates that disqualify a trait from WASM compilation
const WASM_BLOCKERS: &[&str] = &[
    "std::fs", "std::net", "std::process", "tokio::", "actix",
    "libc::", "libloading", "crossterm", "dashmap", "crate::globals",
    "crate::registry", "crate::dispatcher", "crate::config",
    "crate::dylib_loader", "crate::serve", "crate::reload",
];

fn is_wasm_safe(rs_path: &Path) -> bool {
    let content = match fs::read_to_string(rs_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    for blocker in WASM_BLOCKERS {
        if content.contains(blocker) {
            return false;
        }
    }
    true
}

fn visit_traits(dir: &Path, root_dir: &Path, traits_dir: &Path,
                entries: &mut Vec<(String, String)>,
                modules: &mut Vec<WasmModule>) {
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip the wasm crate itself
            if path.ends_with("kernel/wasm") { continue; }
            visit_traits(&path, root_dir, traits_dir, entries, modules);
            continue;
        }
        if !path.to_string_lossy().ends_with(".trait.toml") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // All traits get registered in BUILTIN_TRAIT_DEFS (for the registry)
        let rel_path = path.strip_prefix(root_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        let trait_path = path.strip_prefix(traits_dir)
            .ok()
            .and_then(|p| p.to_str())
            .and_then(|s| s.strip_suffix(".trait.toml"))
            .map(|s| {
                let result = s.replace('/', ".").replace('\\', ".");
                let parts: Vec<&str> = result.split('.').collect();
                if parts.len() >= 2 && parts[parts.len() - 1] == parts[parts.len() - 2] {
                    parts[..parts.len() - 1].join(".")
                } else {
                    result
                }
            });

        let tp = match trait_path {
            Some(t) => t,
            None => continue,
        };

        entries.push((tp.clone(), rel_path));

        // Check if this trait is a compiled builtin with a WASM-safe .rs file
        let mut is_builtin = false;
        let mut is_not_callable = false;
        let mut is_background = false;
        let mut entry_name = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("source") && (trimmed.contains("\"builtin\"") || trimmed.contains("\"kernel\"")) {
                is_builtin = true;
            }
            if trimmed.starts_with("callable") && trimmed.contains("false") {
                is_not_callable = true;
            }
            if trimmed.starts_with("background") && trimmed.contains("true") {
                is_background = true;
            }
            if trimmed.starts_with("entry") {
                if let Some(val) = trimmed.split('=').nth(1) {
                    entry_name = val.trim().trim_matches('"').to_string();
                }
            }
        }

        if !is_builtin || is_not_callable || is_background {
            continue;
        }

        let toml_dir = path.parent().unwrap();
        let dir_name = toml_dir.file_name().unwrap().to_string_lossy();
        let rs_file = toml_dir.join(format!("{}.rs", dir_name));

        if rs_file.exists() && is_wasm_safe(&rs_file) {
            let rs_rel = rs_file.strip_prefix(root_dir)
                .unwrap_or(&rs_file)
                .to_string_lossy()
                .replace('\\', "/");
            let mod_name = tp.rsplit('.').next().unwrap_or(&tp).to_string();
            let entry = if entry_name.is_empty() { mod_name.clone() } else { entry_name };
            modules.push(WasmModule {
                trait_path: tp,
                mod_name,
                entry,
                rs_rel_path: rs_rel,
            });
        }
    }
}

fn watch_dirs_recursive(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            if entry.path().is_dir() {
                watch_dirs_recursive(&entry.path());
            }
        }
    }
}
