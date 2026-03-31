use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

#[path = "../../../scripts/cli_formatters_codegen.rs"]
mod cli_formatters_codegen;

/// Rust reserved keywords that need `r#` prefix when used as identifiers.
const RUST_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const",
    "continue", "crate", "do", "dyn", "else", "enum", "extern", "false",
    "final", "fn", "for", "if", "impl", "in", "let", "loop", "macro",
    "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true",
    "try", "type", "typeof", "union", "unsafe", "unsized", "use", "virtual",
    "where", "while", "yield",
];

fn rust_ident(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// A WASM-compilable trait discovered from .trait.toml with wasm = true.
struct WasmTrait {
    trait_path: String,
    mod_name: String,
    entry: String,
    abs_rs_path: String,
    callable: bool,
    helper_preferred: bool,
}

/// A forwarding entry: trait A dispatches to trait B in WASM.
struct WasmForward {
    trait_path: String,
    target: String,
    helper_preferred: bool,
}

struct CliFormatter {
    trait_path: String,
    mod_name: String,
    abs_rs_path: String,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // workspace root is 3 levels up: traits/kernel/wasm → traits/kernel → traits → root
    let root_dir = manifest_dir.parent().unwrap().parent().unwrap().parent().unwrap();
    let traits_dir = root_dir.join("traits");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Set TRAITS_BUILD_VERSION for WASM — read from env var (set by build.sh)
    // or fall back to parsing version.trait.toml
    let version_toml = root_dir.join("traits/sys/version/version.trait.toml");
    let build_version = env::var("TRAITS_BUILD_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| {
            fs::read_to_string(&version_toml).ok().and_then(|content| {
                content.lines().find_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with("version") && trimmed.contains('=') {
                        let val = trimmed.split('=').nth(1)?;
                        let v = val.trim().trim_matches('"').trim();
                        if !v.is_empty() { Some(v.to_string()) } else { None }
                    } else {
                        None
                    }
                })
            })
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=TRAITS_BUILD_VERSION={}", build_version);
    println!("cargo:rerun-if-env-changed=TRAITS_BUILD_VERSION");

    println!("cargo:rerun-if-changed=build.rs");
    watch_dirs_recursive(&traits_dir);
    let docs_dir_watch = root_dir.join("docs");
    if docs_dir_watch.is_dir() {
        watch_dirs_recursive(&docs_dir_watch);
    }

    let mut entries: Vec<(String, String)> = Vec::new();
    let mut wasm_traits: Vec<WasmTrait> = Vec::new();
    let mut wasm_forwards: Vec<WasmForward> = Vec::new();
    let mut cli_formatters: Vec<CliFormatter> = Vec::new();
    visit_traits(&traits_dir, root_dir, &traits_dir, &mut entries,
                 &mut wasm_traits, &mut wasm_forwards, &mut cli_formatters);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    wasm_traits.sort_by(|a, b| a.trait_path.cmp(&b.trait_path));
    wasm_forwards.sort_by(|a, b| a.trait_path.cmp(&b.trait_path));
    cli_formatters.sort_by(|a, b| a.trait_path.cmp(&b.trait_path));

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

    // ── Generate BUILTIN_FEATURES ──
    // Tuple: (trait_path, vfs_rel_path, content)
    // vfs_rel_path is the natural file path used as the VFS key in LayeredVfs.
    bt.push_str("\npub const BUILTIN_FEATURES: &[(&str, &str, &str)] = &[\n");
    for (path, rel_path) in &entries {
        let features_rel = rel_path.replace(".trait.toml", ".features.json");
        let features_abs = root_dir.join(&features_rel);
        if features_abs.exists() {
            bt.push_str(&format!(
                "    ({:?}, {:?}, include_str!({:?})),\n",
                path, features_rel, features_abs.to_string_lossy()
            ));
        }
    }
    bt.push_str("];\n");

    // ── Generate BUILTIN_DOCS ──
    // Embeds docs/*.md files + trait-specific .md files for browser VFS.
    // Tuple: (vfs_rel_path, content)
    bt.push_str("\npub const BUILTIN_DOCS: &[(&str, &str)] = &[\n");
    let docs_dir = root_dir.join("docs");
    if docs_dir.is_dir() {
        let mut doc_paths: Vec<PathBuf> = Vec::new();
        collect_md_files(&docs_dir, &mut doc_paths);
        doc_paths.sort();
        for doc_path in &doc_paths {
            if let Ok(rel) = doc_path.strip_prefix(root_dir) {
                let rel_str = rel.to_string_lossy().to_string();
                bt.push_str(&format!(
                    "    ({:?}, include_str!({:?})),\n",
                    rel_str, doc_path.to_string_lossy()
                ));
            }
        }
    }
    // Also bundle .md files from traits/ directories (e.g. voice instructions)
    let traits_dir = root_dir.join("traits");
    if traits_dir.is_dir() {
        let mut trait_md_paths: Vec<PathBuf> = Vec::new();
        collect_md_files(&traits_dir, &mut trait_md_paths);
        trait_md_paths.sort();
        for md_path in &trait_md_paths {
            if let Ok(rel) = md_path.strip_prefix(root_dir) {
                let rel_str = rel.to_string_lossy().to_string();
                bt.push_str(&format!(
                    "    ({:?}, include_str!({:?})),\n",
                    rel_str, md_path.to_string_lossy()
                ));
            }
        }
    }
    bt.push_str("];\n");

    fs::write(out_dir.join("wasm_builtin_traits.rs"), bt).expect("write builtin_traits");

    // ── Generate cli_formatters.rs (portable *.cli.rs registry for kernel/cli) ──
    let cli_entries: Vec<(String, String, String)> = cli_formatters
        .iter()
        .map(|f| {
            (
                f.trait_path.clone(),
                rust_ident(&f.mod_name),
                f.abs_rs_path.clone(),
            )
        })
        .collect();
    cli_formatters_codegen::write_cli_formatters(&out_dir.join("cli_formatters.rs"), &cli_entries);

    // ── Resolve WASM module name collisions ──
    {
        let mut name_counts: HashMap<String, usize> = HashMap::new();
        for m in &wasm_traits {
            *name_counts.entry(m.mod_name.clone()).or_insert(0) += 1;
        }
        for m in &mut wasm_traits {
            if name_counts.get(&m.mod_name).copied().unwrap_or(0) > 1 {
                let parts: Vec<&str> = m.trait_path.rsplitn(3, '.').collect();
                if parts.len() >= 2 {
                    m.mod_name = rust_ident(&format!("{}_{}", parts[1], parts[0]));
                }
            }
        }
    }

    // ── Generate wasm_compiled_traits.rs ──
    let mut ct = String::new();

    // Module declarations
    for m in &wasm_traits {
        ct.push_str(&format!(
            "#[allow(dead_code)]\n#[path = {:?}]\npub mod {};\n\n",
            m.abs_rs_path, rust_ident(&m.mod_name)
        ));
        println!("cargo:rerun-if-changed={}", m.abs_rs_path);
    }

    // WASM_CALLABLE const (callable traits + forward targets)
    ct.push_str("/// WASM-callable trait paths (auto-generated from wasm = true in .trait.toml).\n");
    ct.push_str("pub const WASM_CALLABLE: &[&str] = &[\n");
    for m in &wasm_traits {
        if m.callable {
            ct.push_str(&format!("    {:?},\n", m.trait_path));
        }
    }
    for f in &wasm_forwards {
        ct.push_str(&format!("    {:?},\n", f.trait_path));
    }
    ct.push_str("];\n\n");

    // HELPER_PREFERRED const
    ct.push_str("/// Traits that prefer native helper dispatch when available.\n");
    ct.push_str("pub const HELPER_PREFERRED: &[&str] = &[\n");
    for m in &wasm_traits {
        if m.helper_preferred {
            ct.push_str(&format!("    {:?},\n", m.trait_path));
        }
    }
    for f in &wasm_forwards {
        if f.helper_preferred {
            ct.push_str(&format!("    {:?},\n", f.trait_path));
        }
    }
    ct.push_str("];\n\n");

    // Build forward target lookup: trait_path → (mod_name, entry)
    let dispatch_lookup: HashMap<String, (String, String)> = wasm_traits.iter()
        .filter(|m| m.callable)
        .map(|m| (m.trait_path.clone(), (m.mod_name.clone(), m.entry.clone())))
        .collect();

    // Dispatch function
    ct.push_str("/// Dispatch a trait call by path. Auto-generated from wasm = true in .trait.toml.\n");
    ct.push_str("pub fn dispatch(trait_path: &str, args: &[serde_json::Value]) -> Option<serde_json::Value> {\n");
    ct.push_str("    if crate::is_helper_connected() && HELPER_PREFERRED.contains(&trait_path) {\n");
    ct.push_str("        return None;\n");
    ct.push_str("    }\n");
    ct.push_str("    match trait_path {\n");
    for m in &wasm_traits {
        if m.callable {
            ct.push_str(&format!(
                "        {:?} => Some({}::{}(args)),\n",
                m.trait_path, rust_ident(&m.mod_name), m.entry
            ));
        }
    }
    // Forward entries: resolve to target's module and function
    for f in &wasm_forwards {
        if let Some((mod_name, entry)) = dispatch_lookup.get(&f.target) {
            ct.push_str(&format!(
                "        {:?} => Some({}::{}(args)),\n",
                f.trait_path, rust_ident(mod_name), entry
            ));
        } else {
            eprintln!("cargo:warning=WASM forward target {:?} not found for {:?}",
                       f.target, f.trait_path);
        }
    }
    ct.push_str("        _ => None,\n");
    ct.push_str("    }\n");
    ct.push_str("}\n");

    fs::write(out_dir.join("wasm_compiled_traits.rs"), &ct)
        .expect("write wasm_compiled_traits.rs");

    let callable_count = wasm_traits.iter().filter(|m| m.callable).count() + wasm_forwards.len();
    let module_count = wasm_traits.len();
    eprintln!("cargo:warning=WASM codegen: {} modules, {} callable traits, {} forwards",
              module_count, callable_count, wasm_forwards.len());
}

fn visit_traits(dir: &Path, root_dir: &Path, traits_dir: &Path,
                entries: &mut Vec<(String, String)>,
                wasm_traits: &mut Vec<WasmTrait>,
                wasm_forwards: &mut Vec<WasmForward>,
                cli_formatters: &mut Vec<CliFormatter>) {
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.ends_with("kernel/wasm") { continue; }
            visit_traits(&path, root_dir, traits_dir, entries, wasm_traits, wasm_forwards, cli_formatters);
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
            entries.push((tp.clone(), rel_path));

            let toml_dir = path.parent().unwrap();
            let dir_name = toml_dir.file_name().unwrap().to_string_lossy();

            // Discover companion .cli.rs file for portable CLI formatting.
            let cli_file = toml_dir.join(format!("{}.cli.rs", dir_name));
            if cli_file.exists() {
                cli_formatters.push(CliFormatter {
                    trait_path: tp.clone(),
                    mod_name: format!("{}_cli", tp.rsplit('.').next().unwrap_or(&tp)),
                    abs_rs_path: cli_file.to_string_lossy().to_string(),
                });
            }

            // ── Parse WASM fields from .trait.toml ──
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut is_wasm = false;
            let mut wasm_callable = true;
            let mut wasm_entry = String::new();
            let mut wasm_source = String::new();
            let mut wasm_forward = String::new();
            let mut entry_name = String::new();
            let mut helper_preferred = false;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed == "wasm = true" {
                    is_wasm = true;
                }
                if trimmed == "wasm_callable = false" {
                    wasm_callable = false;
                }
                if trimmed == "helper_preferred = true" {
                    helper_preferred = true;
                }
                if trimmed.starts_with("entry = ") || trimmed.starts_with("entry=") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        entry_name = val.trim().trim_matches('"').to_string();
                    }
                }
                if trimmed.starts_with("wasm_entry = ") || trimmed.starts_with("wasm_entry=") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        wasm_entry = val.trim().trim_matches('"').to_string();
                    }
                }
                if trimmed.starts_with("wasm_source = ") || trimmed.starts_with("wasm_source=") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        wasm_source = val.trim().trim_matches('"').to_string();
                    }
                }
                if trimmed.starts_with("wasm_forward = ") || trimmed.starts_with("wasm_forward=") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        wasm_forward = val.trim().trim_matches('"').to_string();
                    }
                }
            }

            if !is_wasm { continue; }

            // Forward: no module needed, just a dispatch alias
            if !wasm_forward.is_empty() {
                wasm_forwards.push(WasmForward {
                    trait_path: tp,
                    target: wasm_forward,
                    helper_preferred,
                });
                continue;
            }

            // Resolve entry function name
            let effective_entry = if !wasm_entry.is_empty() {
                wasm_entry
            } else if !entry_name.is_empty() {
                entry_name.clone()
            } else {
                tp.rsplit('.').next().unwrap_or(&tp).to_string()
            };

            // Resolve source .rs file
            let dir_name = toml_dir.file_name().unwrap().to_string_lossy().to_string();

            let rs_file = if !wasm_source.is_empty() {
                // Explicit wasm_source override
                let f = toml_dir.join(&wasm_source);
                if f.exists() { Some(f) } else { None }
            } else {
                // Try {dir_name}.rs first
                let f = toml_dir.join(format!("{}.rs", dir_name));
                if f.exists() {
                    Some(f)
                } else {
                    // Fallback: try {entry_name}.rs
                    let f2 = toml_dir.join(format!("{}.rs", entry_name));
                    if f2.exists() {
                        Some(f2)
                    } else {
                        // Last resort: find any single .rs file (excluding *.cli.rs)
                        find_single_rs(toml_dir)
                    }
                }
            };

            let rs_file = match rs_file {
                Some(f) => f,
                None => {
                    eprintln!("cargo:warning=WASM trait {:?}: no .rs file found in {}",
                              tp, toml_dir.display());
                    continue;
                }
            };

            let mod_name = rust_ident(tp.rsplit('.').next().unwrap_or(&tp));

            wasm_traits.push(WasmTrait {
                trait_path: tp,
                mod_name,
                entry: effective_entry,
                abs_rs_path: rs_file.to_string_lossy().to_string(),
                callable: wasm_callable,
                helper_preferred,
            });
        }
    }
}

/// Find a single .rs file in a directory, excluding *.cli.rs files.
fn find_single_rs(dir: &Path) -> Option<PathBuf> {
    let mut rs_files: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".rs") && !name.ends_with(".cli.rs") {
                    rs_files.push(p);
                }
            }
        }
    }
    if rs_files.len() == 1 {
        Some(rs_files.remove(0))
    } else {
        None
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

/// Recursively collect all .md files under a directory.
fn collect_md_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_md_files(&path, out);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                out.push(path);
            }
        }
    }
}
