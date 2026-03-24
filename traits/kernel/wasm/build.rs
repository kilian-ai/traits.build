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
    visit_traits(&traits_dir, root_dir, &traits_dir, &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));

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
}

fn visit_traits(dir: &Path, root_dir: &Path, traits_dir: &Path,
                entries: &mut Vec<(String, String)>) {
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.ends_with("kernel/wasm") { continue; }
            visit_traits(&path, root_dir, traits_dir, entries);
            continue;
        }
        if !path.to_string_lossy().ends_with(".trait.toml") {
            continue;
        }

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

        if let Some(tp) = trait_path {
            entries.push((tp, rel_path));
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
