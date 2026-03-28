use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};

#[path = "scripts/cli_formatters_codegen.rs"]
mod cli_formatters_codegen;

// Shared SHA-256 helpers (canonical copy at root, mirrored in sys/checksum/)
include!("sha256.rs");

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

/// Escape a name if it's a Rust keyword (e.g. "static" → "r#static").
fn rust_ident(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// Represents a discovered builtin trait with its source .rs file.
struct TraitModule {
    /// Trait path: "sys.checksum", "kernel.serve", etc.
    trait_path: String,
    /// Module name for Rust: "checksum", "serve", etc.
    mod_name: String,
    /// Entry function name from .trait.toml
    entry: String,
    /// Relative path to .rs source from manifest dir: "traits/sys/checksum/checksum.rs"
    rs_rel_path: String,
    /// Whether this is a background (async) trait
    background: bool,
    /// Whether this is a kernel module promoted to builtin (dispatch via crate:: prefix)
    is_kernel_builtin: bool,
}

/// A kernel module (crate-level mod) discovered from traits/kernel/.
struct KernelModule {
    /// Module name: "types", "config", "dispatcher", etc.
    mod_name: String,
    /// Absolute path to the .rs source file.
    abs_path: String,
}

/// A discovered *.cli.rs companion providing format_cli(result) -> String.
struct CliFormatter {
    trait_path: String,
    mod_name: String,
    rs_rel_path: String,
}

/// A discovered static asset (.css, .js) that lives alongside a trait.
struct StaticAsset {
    /// Serve path: "www/playground/playground.css"
    serve_path: String,
    /// Absolute path to the file on disk.
    abs_path: String,
    /// MIME content type: "text/css" or "application/javascript"
    content_type: &'static str,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Detect cargo publish verification sandbox (read-only source tree)
    let is_publish = manifest_dir.to_string_lossy().contains("target/package/");

    // Tell rustc this is the kernel binary, so #[cfg(kernel)] code compiles
    println!("cargo:rustc-cfg=kernel");
    println!("cargo::rustc-check-cfg=cfg(kernel)");

    // ── Compute build version: vYYMMDD or vYYMMDD.HHMMSS if same day ──
    let build_version = compute_build_version(&manifest_dir, is_publish);
    println!("cargo:rustc-env=TRAITS_BUILD_VERSION={}", build_version);
    println!("cargo:rerun-if-env-changed=TRAITS_BUILD_VERSION");

    let traits_dir = manifest_dir.join("traits");

    // Watch the traits directory tree so new trait files trigger a rebuild
    watch_dirs_recursive(&traits_dir);

    let mut entries: Vec<(String, String)> = Vec::new();
    let mut modules: Vec<TraitModule> = Vec::new();
    let mut cli_formatters: Vec<CliFormatter> = Vec::new();
    let mut kernel_modules: Vec<KernelModule> = Vec::new();
    let mut static_assets: Vec<StaticAsset> = Vec::new();

    visit_traits(&traits_dir, &manifest_dir, &traits_dir, &mut entries, &mut modules, &mut cli_formatters, &mut kernel_modules, &mut static_assets, is_publish);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    modules.sort_by(|a, b| a.trait_path.cmp(&b.trait_path));

    // ── Kernel 3-layer architecture lint ──
    // Verify kernel.* trait WASM status: portable (Layer 1) vs infrastructure (Layer 2)
    lint_kernel_layers(&traits_dir);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // ── Generate builtin_traits.rs (TOML definitions for registry) ──
    let out_path = out_dir.join("builtin_traits.rs");
    let mut output = String::new();
    output.push_str("pub const BUILTIN_TRAIT_DEFS: &[BuiltinTraitDef] = &[\n");
    for (path, rel_path) in entries {
        output.push_str(&format!(
            "    BuiltinTraitDef {{ path: {:?}, rel_path: {:?}, toml: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\")) }},\n",
            path,
            rel_path,
            rel_path
        ));
    }
    output.push_str("];\n");
    fs::write(out_path, output).expect("Failed to write builtin_traits.rs");

    // ── Resolve module name collisions ──
    // Multiple traits can produce the same mod_name (e.g. sys.cli.wasm and www.wasm both → "wasm").
    // Detect collisions and qualify with parent segment to make them unique.
    {
        let mut name_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for m in &modules {
            if m.is_kernel_builtin { continue; }
            *name_counts.entry(m.mod_name.clone()).or_insert(0) += 1;
        }
        for m in &mut modules {
            if m.is_kernel_builtin { continue; }
            if name_counts.get(&m.mod_name).copied().unwrap_or(0) > 1 {
                // Qualify: sys.cli.wasm → cli_wasm, www.wasm → www_wasm
                let parts: Vec<&str> = m.trait_path.rsplitn(3, '.').collect();
                if parts.len() >= 2 {
                    m.mod_name = rust_ident(&format!("{}_{}", parts[1], parts[0]));
                }
            }
        }
    }

    // ── Generate compiled_traits.rs (module declarations + dispatch) ──
    let compiled_path = out_dir.join("compiled_traits.rs");
    let mut ct = String::new();

    // Module declarations (skip kernel builtins — they're declared at crate root via kernel_modules.rs)
    for m in &modules {
        if m.is_kernel_builtin { continue; }
        let abs_path = manifest_dir.join(&m.rs_rel_path);
        ct.push_str(&format!(
            "#[path = {:?}]\npub mod {};\n\n",
            abs_path.to_string_lossy(), rust_ident(&m.mod_name)
        ));
        println!("cargo:rerun-if-changed={}", abs_path.display());
    }

    // Dispatch function
    ct.push_str("/// Auto-generated dispatch to compiled Rust trait modules.\n");
    ct.push_str("pub fn dispatch_compiled(trait_path: &str, args: &[serde_json::Value]) -> Option<serde_json::Value> {\n");
    ct.push_str("    match trait_path {\n");
    for m in &modules {
        let func = if m.entry == "checksum" {
            format!("{}::checksum_dispatch", rust_ident(&m.mod_name))
        } else if m.is_kernel_builtin && m.mod_name == "main" {
            // main.rs IS the crate root — no module prefix
            format!("crate::{}", m.entry)
        } else if m.is_kernel_builtin {
            format!("crate::{}::{}", rust_ident(&m.mod_name), m.entry)
        } else {
            format!("{}::{}", rust_ident(&m.mod_name), m.entry)
        };
        ct.push_str(&format!(
            "        {:?} => Some({}(args)),\n",
            m.trait_path, func
        ));
    }
    ct.push_str("        _ => None,\n");
    ct.push_str("    }\n");
    ct.push_str("}\n\n");

    // dispatch_trait_value: TraitValue interface for worker
    ct.push_str("/// Dispatch with TraitValue args/result (for worker integration).\n");
    ct.push_str("pub fn dispatch_trait_value(trait_path: &str, args: &[crate::types::TraitValue]) -> Option<crate::types::TraitValue> {\n");
    ct.push_str("    let json_args: Vec<serde_json::Value> = args.iter().map(|a| a.to_json()).collect();\n");
    ct.push_str("    dispatch(trait_path, &json_args).map(|v| crate::types::TraitValue::from_json(&v))\n");
    ct.push_str("}\n\n");

    // Unified dispatch: dylib first, then compiled
    ct.push_str("/// Unified dispatch: tries dylib loader first, then compiled-in modules.\n");
    ct.push_str("pub fn dispatch(trait_path: &str, args: &[serde_json::Value]) -> Option<serde_json::Value> {\n");
    ct.push_str("    if let Some(loader) = crate::dylib_loader::LOADER.get() {\n");
    ct.push_str("        if let Some(result) = loader.dispatch(trait_path, args) {\n");
    ct.push_str("            return Some(result);\n");
    ct.push_str("        }\n");
    ct.push_str("    }\n");
    ct.push_str("    dispatch_compiled(trait_path, args)\n");
    ct.push_str("}\n\n");

    // dispatch_async: async dispatch for background traits
    let bg_modules: Vec<&TraitModule> = modules.iter().filter(|m| m.background).collect();
    ct.push_str("/// Async dispatch for background traits (background = true in trait.toml).\n");
    ct.push_str("pub async fn dispatch_async(trait_path: &str, args: &[crate::types::TraitValue]) -> Option<Result<crate::types::TraitValue, Box<dyn std::error::Error + Send + Sync>>> {\n");
    ct.push_str("    match trait_path {\n");
    for m in &bg_modules {
        let prefix = if m.is_kernel_builtin { "crate::" } else { "" };
        ct.push_str(&format!(
            "        {:?} => Some({}{}::start(args).await),\n",
            m.trait_path, prefix, m.mod_name
        ));
    }
    ct.push_str("        _ => None,\n");
    ct.push_str("    }\n");
    ct.push_str("}\n\n");

    // list_compiled: returns trait paths of all compiled modules
    ct.push_str("/// Returns the list of all compiled trait paths.\n");
    ct.push_str("pub fn list_compiled() -> Vec<&'static str> {\n");
    ct.push_str("    vec![\n");
    for m in &modules {
        ct.push_str(&format!("        {:?},\n", m.trait_path));
    }
    ct.push_str("    ]\n");
    ct.push_str("}\n");

    fs::write(compiled_path, ct).expect("Failed to write compiled_traits.rs");

    // ── Generate cli_formatters.rs (optional CLI output formatters) ──
    cli_formatters.sort_by(|a, b| a.trait_path.cmp(&b.trait_path));
    let cli_path = out_dir.join("cli_formatters.rs");
    let cli_entries: Vec<(String, String, String)> = cli_formatters
        .iter()
        .map(|f| {
            (
                f.trait_path.clone(),
                rust_ident(&f.mod_name),
                manifest_dir.join(&f.rs_rel_path).to_string_lossy().to_string(),
            )
        })
        .collect();
    cli_formatters_codegen::write_cli_formatters(&cli_path, &cli_entries);

    // ── Generate kernel_modules.rs (crate-level mod declarations for kernel/) ──
    let kernel_path = out_dir.join("kernel_modules.rs");
    let mut km = String::new();
    km.push_str("// Auto-generated by build.rs — kernel module declarations\n");
    for k in &kernel_modules {
        km.push_str(&format!(
            "#[path = {:?}]\npub mod {};\n\n",
            k.abs_path, rust_ident(&k.mod_name)
        ));
        println!("cargo:rerun-if-changed={}", k.abs_path);
    }
    fs::write(kernel_path, km).expect("Failed to write kernel_modules.rs");

    // ── Discover JS client library from kernel/wasm/js/ ──
    let wasm_js_dir = traits_dir.join("kernel/wasm/js");
    if wasm_js_dir.exists() {
        if let Ok(rd) = fs::read_dir(&wasm_js_dir) {
            for entry in rd.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let ct = if fname.ends_with(".js") {
                    Some("application/javascript")
                } else if fname.ends_with(".css") {
                    Some("text/css")
                } else {
                    None
                };
                if let Some(content_type) = ct {
                    static_assets.push(StaticAsset {
                        serve_path: format!("js/{}", fname),
                        abs_path: entry.path().to_string_lossy().to_string(),
                        content_type,
                    });
                    println!("cargo:rerun-if-changed={}", entry.path().display());
                }
            }
        }
    }

    // ── Generate static_assets.rs (embedded CSS/JS files served at /static/) ──
    static_assets.sort_by(|a, b| a.serve_path.cmp(&b.serve_path));
    let sa_path = out_dir.join("static_assets.rs");
    let mut sa = String::new();
    sa.push_str("/// Look up an embedded static asset by its serve path.\n");
    sa.push_str("/// Returns (content, content_type) if found.\n");
    sa.push_str("pub fn get_static_asset(path: &str) -> Option<(&'static str, &'static str)> {\n");
    sa.push_str("    match path {\n");
    for a in &static_assets {
        sa.push_str(&format!(
            "        {:?} => Some((include_str!({:?}), {:?})),\n",
            a.serve_path, a.abs_path, a.content_type
        ));
        println!("cargo:rerun-if-changed={}", a.abs_path);
    }
    sa.push_str("        _ => None,\n");
    sa.push_str("    }\n");
    sa.push_str("}\n");
    fs::write(sa_path, sa).expect("Failed to write static_assets.rs");

    // ── Generate wasm_static_assets.rs (binary WASM module files from wasm-pack) ──
    let wasm_pkg_dir = traits_dir.join("kernel/wasm/pkg");
    let wsa_path = out_dir.join("wasm_static_assets.rs");
    let mut wsa = String::new();
    wsa.push_str("/// Look up an embedded WASM asset by filename.\n");
    wsa.push_str("/// Returns (bytes, content_type) if found.\n");
    wsa.push_str("pub fn get_wasm_asset(path: &str) -> Option<(&'static [u8], &'static str)> {\n");
    wsa.push_str("    match path {\n");
    if wasm_pkg_dir.exists() {
        if let Ok(rd) = fs::read_dir(&wasm_pkg_dir) {
            for entry in rd.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let ct = if fname.ends_with(".wasm") {
                    "application/wasm"
                } else if fname.ends_with(".js") {
                    "application/javascript"
                } else {
                    continue;
                };
                let abs_path = entry.path().to_string_lossy().to_string();
                wsa.push_str(&format!(
                    "        {:?} => Some((include_bytes!({:?}), {:?})),\n",
                    fname, abs_path, ct
                ));
                println!("cargo:rerun-if-changed={}", abs_path);
            }
        }
    }
    wsa.push_str("        _ => None,\n");
    wsa.push_str("    }\n");
    wsa.push_str("}\n");
    fs::write(wsa_path, wsa).expect("Failed to write wasm_static_assets.rs");
}

/// Compute YYMMDD from current UTC time (no chrono dependency).
fn yymmdd_build() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let days = secs / 86400;
    let d = days as i64 + 719468;
    let era = if d >= 0 { d } else { d - 146096 } / 146097;
    let doe = (d - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    let day = doy - (153 * mp + 2) / 5 + 1;
    format!("{:02}{:02}{:02}", year % 100, month, day)
}

/// Compute HHMMSS from current UTC time.
fn hhmmss_build() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let tod = secs % 86400;
    let h = tod / 3600;
    let m = (tod % 3600) / 60;
    let s = tod % 60;
    format!("{:02}{:02}{:02}", h, m, s)
}

/// Read current version from version.trait.toml, bump with dot notation if
/// same day, write back, and return the new version string (with "v" prefix).
fn compute_version_from_toml(toml_path: &Path, today: &str) -> String {
    let current = fs::read_to_string(toml_path)
        .ok()
        .and_then(|content| {
            content.lines().find_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("version") && trimmed.contains('=') {
                    let val = trimmed.split('=').nth(1)?;
                    let v = val.trim().trim_matches('"').trim();
                    let v = v.strip_prefix('v').unwrap_or(v);
                    if !v.is_empty() { Some(v.to_string()) } else { None }
                } else {
                    None
                }
            })
        })
        .unwrap_or_default();

    if current.starts_with(today) {
        format!("v{}.{}", today, hhmmss_build())
    } else {
        format!("v{}", today)
    }
}

fn compute_build_version(manifest_dir: &Path, is_publish: bool) -> String {
    let toml_path = manifest_dir.join("traits/sys/version/version.trait.toml");
    let today = yymmdd_build();

    // If TRAITS_BUILD_VERSION is set (e.g. by build.sh), use it directly.
    // This ensures WASM and native builds share the exact same version.
    let new_ver = if let Ok(override_ver) = std::env::var("TRAITS_BUILD_VERSION") {
        if !override_ver.is_empty() {
            override_ver
        } else {
            compute_version_from_toml(&toml_path, &today)
        }
    } else {
        compute_version_from_toml(&toml_path, &today)
    };

    // Write back to version.trait.toml (skip if read-only, e.g. during cargo publish)
    if is_publish {
        return new_ver;
    }
    if let Ok(content) = fs::read_to_string(&toml_path) {
        let updated: String = content
            .lines()
            .map(|line| {
                if line.trim().starts_with("version") && line.contains('=') {
                    format!("version = \"{}\"", new_ver)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let _ = fs::write(&toml_path, updated);
    }

    // Sync version to all workspace member Cargo.toml files
    let ver_str = new_ver.strip_prefix('v').unwrap_or(&new_ver);
    let cargo_ver = if let Some((date, time)) = ver_str.split_once('.') {
        // Semver forbids leading zeros: parse HHMMSS as integer to strip them
        let patch: u32 = time.parse().unwrap_or(0);
        format!("0.{}.{}", date, patch)
    } else {
        format!("0.{}.0", ver_str)
    };

    // All workspace Cargo.toml paths to keep in sync
    let workspace_tomls = [
        "Cargo.toml",
        "traits/kernel/logic/Cargo.toml",
        "traits/kernel/plugin_api/Cargo.toml",
        "traits/kernel/wasm/Cargo.toml",
        "traits/sys/checksum/Cargo.toml",
        "traits/sys/ps/Cargo.toml",
        "traits/www/traits/build/Cargo.toml",
    ];
    for rel_path in &workspace_tomls {
        let cargo_path = manifest_dir.join(rel_path);
        sync_cargo_version(&cargo_path, &cargo_ver);
    }

    new_ver
}

/// Sync [package] version and all local path-dep versions in a Cargo.toml to `cargo_ver`.
fn sync_cargo_version(cargo_path: &std::path::Path, cargo_ver: &str) {
    let content = match fs::read_to_string(cargo_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut in_package = false;
    let mut pkg_version_replaced = false;
    let updated: String = content
        .lines()
        .map(|line| {
            // Track which TOML section we're in
            if line.trim() == "[package]" {
                in_package = true;
            } else if line.trim().starts_with('[') {
                in_package = false;
            }

            // Replace [package] version = "..."
            if in_package && !pkg_version_replaced
                && line.trim().starts_with("version")
                && line.contains('=')
            {
                pkg_version_replaced = true;
                return format!("version = \"{}\"", cargo_ver);
            }

            // Replace version = "..." inside local path dependency lines
            // e.g.: kernel-logic = { path = "...", version = "OLD" }
            if line.contains("path = \"") && line.contains("version = \"") {
                return replace_dep_version(line, cargo_ver);
            }

            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    if updated != content {
        let _ = fs::write(cargo_path, updated);
    }
}

/// Replace the `version = "..."` value inside a dependency line, leaving everything else intact.
fn replace_dep_version(line: &str, new_ver: &str) -> String {
    // Find `version = "` (11 chars) and replace the quoted value
    if let Some(ver_start) = line.find("version = \"") {
        let prefix = &line[..ver_start + 11]; // up to and including the opening quote
        let rest = &line[ver_start + 11..];   // from the old version value onward (after opening quote)
        if let Some(end_quote) = rest.find('"') {
            let suffix = &rest[end_quote..]; // from the closing quote onward
            return format!("{}{}{}", prefix, new_ver, suffix);
        }
    }
    line.to_string()
}

/// Compute SHA-256 hex digest of a file's contents.
fn compute_file_checksum(path: &Path) -> Option<String> {
    let content = fs::read(path).ok()?;
    Some(sha256_bytes(&content))
}

/// Compare computed checksum with stored one in .trait.toml.
/// If different (or missing), bump the version and write the new checksum.
fn update_trait_checksum(toml_path: &Path, rs_path: &Path, is_publish: bool) {
    let new_checksum = match compute_file_checksum(rs_path) {
        Some(c) => c,
        None => return,
    };

    let content = match fs::read_to_string(toml_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Extract existing checksum
    let existing_checksum = content.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with("checksum") && trimmed.contains('=') {
            trimmed.split('=').nth(1).map(|v| v.trim().trim_matches('"').to_string())
        } else {
            None
        }
    });

    // If checksum matches, no changes needed
    if existing_checksum.as_deref() == Some(new_checksum.as_str()) {
        return;
    }

    // Skip writes if file is read-only (e.g. during cargo publish)
    if is_publish {
        return;
    }

    // Checksum changed (or missing) — bump version and update checksum
    let today = yymmdd_build();

    let current_version = content.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with("version") && trimmed.contains('=') {
            trimmed.split('=').nth(1).map(|v| v.trim().trim_matches('"').to_string())
        } else {
            None
        }
    }).unwrap_or_default();

    let current_no_v = current_version.strip_prefix('v').unwrap_or(&current_version);
    let new_ver = if current_no_v.starts_with(&today) {
        format!("v{}.{}", today, hhmmss_build())
    } else {
        format!("v{}", today)
    };

    // Rewrite the toml: update version, update or insert checksum after version
    let has_checksum = content.lines().any(|l| l.trim().starts_with("checksum") && l.contains('='));
    let mut updated_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version") && trimmed.contains('=') {
            updated_lines.push(format!("version = \"{}\"", new_ver));
            if !has_checksum {
                updated_lines.push(format!("checksum = \"{}\"", new_checksum));
            }
        } else if trimmed.starts_with("checksum") && trimmed.contains('=') {
            updated_lines.push(format!("checksum = \"{}\"", new_checksum));
        } else {
            updated_lines.push(line.to_string());
        }
    }

    let _ = fs::write(toml_path, updated_lines.join("\n"));
}

/// Recursively emit cargo:rerun-if-changed for all directories under a path.
/// This ensures new trait files (added to any subdirectory) trigger a build.rs re-run.
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

/// Kernel 3-layer architecture lint.
///
/// Scans kernel/*.trait.toml files and classifies them:
///   Layer 0: kernel/logic — shared library (source = "library")
///   Layer 1: portable — wasm = true (compile for both native + WASM)
///   Layer 2: infrastructure — no wasm = true (native-only runtime)
///
/// Emits cargo warnings for:
///   - kernel traits with wasm = true whose .rs file imports native-only APIs without cfg-gating
///   - kernel traits missing explicit wasm field (ambiguous portability)
fn lint_kernel_layers(traits_dir: &Path) {
    let kernel_dir = traits_dir.join("kernel");
    if !kernel_dir.exists() { return; }

    struct KernelTraitInfo {
        name: String,
        has_wasm: bool,
        wasm_value: bool,
        source: String,
    }

    let mut infos: Vec<KernelTraitInfo> = Vec::new();

    // Scan kernel/ subdirectories for .trait.toml files
    if let Ok(rd) = fs::read_dir(&kernel_dir) {
        for entry in rd.flatten() {
            let dir_path = entry.path();
            if !dir_path.is_dir() { continue; }
            let dir_name = dir_path.file_name().unwrap().to_string_lossy().to_string();
            let toml_path = dir_path.join(format!("{}.trait.toml", dir_name));
            if !toml_path.exists() { continue; }

            let content = match fs::read_to_string(&toml_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut has_wasm = false;
            let mut wasm_value = false;
            let mut source = String::from("builtin");

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("wasm") && trimmed.contains('=') && !trimmed.starts_with("wasm_") {
                    has_wasm = true;
                    wasm_value = trimmed.contains("true");
                }
                if trimmed.starts_with("source") && trimmed.contains('=') {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        source = val.trim().trim_matches('"').to_string();
                    }
                }
            }

            infos.push(KernelTraitInfo {
                name: format!("kernel.{}", dir_name),
                has_wasm,
                wasm_value,
                source,
            });
        }
    }

    if infos.is_empty() { return; }
    infos.sort_by(|a, b| a.name.cmp(&b.name));

    // Classify and report
    let mut layer0: Vec<&str> = Vec::new();
    let mut layer1: Vec<&str> = Vec::new();
    let mut layer2: Vec<&str> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for info in &infos {
        if info.source == "library" {
            layer0.push(&info.name);
        } else if info.wasm_value {
            layer1.push(&info.name);
        } else {
            layer2.push(&info.name);
            // Warn if builtin/kernel trait doesn't explicitly declare wasm status
            if !info.has_wasm && (info.source == "builtin" || info.source == "kernel") {
                warnings.push(format!(
                    "kernel trait '{}' has no explicit wasm = true/false in .trait.toml — add wasm field to declare portability tier",
                    info.name
                ));
            }
        }
    }

    // Emit summary
    if !layer0.is_empty() {
        println!("cargo:warning=Kernel Layer 0 (shared library): {}", layer0.join(", "));
    }
    if !layer1.is_empty() {
        println!("cargo:warning=Kernel Layer 1 (portable, wasm=true): {}", layer1.join(", "));
    }
    if !layer2.is_empty() {
        println!("cargo:warning=Kernel Layer 2 (infrastructure, native-only): {}", layer2.join(", "));
    }

    for w in &warnings {
        println!("cargo:warning={}", w);
    }
}

fn visit_traits(dir: &Path, manifest_dir: &Path, traits_dir: &Path, entries: &mut Vec<(String, String)>, modules: &mut Vec<TraitModule>, cli_formatters: &mut Vec<CliFormatter>, kernel_modules: &mut Vec<KernelModule>, static_assets: &mut Vec<StaticAsset>, is_publish: bool) {
    // Watch directories so cargo re-runs build.rs when traits are added/removed
    println!("cargo:rerun-if-changed={}", dir.display());

    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_traits(&path, manifest_dir, traits_dir, entries, modules, cli_formatters, kernel_modules, static_assets, is_publish);
            continue;
        }
        if !path.to_string_lossy().ends_with(".trait.toml") {
            continue;
        }

        println!("cargo:rerun-if-changed={}", path.display());

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut is_builtin = false;
        let mut is_rest = false;
        let mut is_background = false;
        let mut is_not_callable = false;
        let mut is_kernel_module = false;
        let mut entry_name = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("source") && (trimmed.contains("\"builtin\"") || trimmed.contains("\"kernel\"")) {
                is_builtin = true;
            }
            if trimmed.starts_with("source") && trimmed.contains("\"rest\"") {
                is_rest = true;
            }
            if trimmed.starts_with("callable") && trimmed.contains("false") {
                is_not_callable = true;
            }
            if trimmed.starts_with("background") && trimmed.contains("true") {
                is_background = true;
            }
            if trimmed == "kernel_module = true" {
                is_kernel_module = true;
            }
            if trimmed.starts_with("entry") {
                // Parse entry = "function_name"
                if let Some(val) = trimmed.split('=').nth(1) {
                    entry_name = val.trim().trim_matches('"').to_string();
                }
            }
        }

        // REST traits: register in entries (for registry) but no compiled module needed
        if is_rest {
            let rel_path = path.strip_prefix(manifest_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let trait_path = path.strip_prefix(traits_dir)
                .ok()
                .and_then(|p| p.to_str())
                .and_then(|s| s.strip_suffix(".trait.toml")
                    .or_else(|| s.strip_suffix(".strait.toml")))
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
            continue;
        }

        if is_builtin {
            let rel_path = path.strip_prefix(manifest_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            // Derive trait path from filesystem
            let trait_path = path.strip_prefix(traits_dir)
                    .ok()
                    .and_then(|p| p.to_str())
                    .and_then(|s| s.strip_suffix(".trait.toml")
                        .or_else(|| s.strip_suffix(".strait.toml")))
                    .map(|s| {
                        let result = s.replace('/', ".").replace('\\', ".");
                        // Collapse trailing duplicate: sys.checksum.checksum -> sys.checksum
                        let parts: Vec<&str> = result.split('.').collect();
                        if parts.len() >= 2 && parts[parts.len() - 1] == parts[parts.len() - 2] {
                            parts[..parts.len() - 1].join(".")
                        } else {
                            result
                        }
                    });
            if let Some(tp) = trait_path.clone() {
                entries.push((tp, rel_path.clone()));
            }

            // Check for sibling .rs file for module generation
            if let Some(tp) = trait_path {
                let toml_dir = path.parent().unwrap();
                let dir_name = toml_dir.file_name().unwrap().to_string_lossy();

                // Check for companion .cli.rs file (CLI output formatter)
                let cli_file = toml_dir.join(format!("{}.cli.rs", dir_name));
                if cli_file.exists() {
                    let cli_rel = cli_file.strip_prefix(manifest_dir)
                        .unwrap_or(&cli_file)
                        .to_string_lossy()
                        .replace('\\', "/");
                    let cli_mod = format!("{}_cli", tp.rsplit('.').next().unwrap_or(&tp));
                    cli_formatters.push(CliFormatter {
                        trait_path: tp.clone(),
                        mod_name: cli_mod,
                        rs_rel_path: cli_rel,
                    });
                }

                // Discover static assets (.css, .js) in this trait's directory
                let serve_prefix = toml_dir.strip_prefix(traits_dir)
                    .unwrap_or(toml_dir)
                    .to_string_lossy()
                    .replace('\\', "/");
                if let Ok(dir_entries) = fs::read_dir(toml_dir) {
                    for de in dir_entries.flatten() {
                        let dp = de.path();
                        let fname = dp.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let content_type = if fname.ends_with(".css") {
                            Some("text/css")
                        } else if fname.ends_with(".js") {
                            Some("application/javascript")
                        } else {
                            None
                        };
                        if let Some(ct) = content_type {
                            static_assets.push(StaticAsset {
                                serve_path: format!("{}/{}", serve_prefix, fname),
                                abs_path: dp.to_string_lossy().to_string(),
                                content_type: ct,
                            });
                        }
                    }
                }

                let rs_file = toml_dir.join(format!("{}.rs", dir_name));
                if rs_file.exists() {
                    // Update checksum in .trait.toml (bumps version if source changed)
                    update_trait_checksum(&path, &rs_file, is_publish);

                    let rs_rel = rs_file.strip_prefix(manifest_dir)
                        .unwrap_or(&rs_file)
                        .to_string_lossy()
                        .replace('\\', "/");
                    // Module name: last segment of trait path (e.g., "sys.checksum" -> "checksum")
                    let mod_name = rust_ident(tp.rsplit('.').next().unwrap_or(&tp));
                    let entry = if entry_name.is_empty() { mod_name.clone() } else { entry_name.clone() };
                    let is_kb = tp.starts_with("kernel.") || is_kernel_module;
                    // Kernel traits (and kernel_module = true) need crate-level module declarations
                    // Skip "main" — it IS the crate root, not a child module
                    if is_kb && mod_name != "main" {
                        kernel_modules.push(KernelModule {
                            mod_name: mod_name.clone(),
                            abs_path: rs_file.to_string_lossy().to_string(),
                        });
                    }
                    // Add to dispatch unless explicitly non-callable (e.g. main, plugin_api)
                    if !is_not_callable {
                        modules.push(TraitModule {
                            trait_path: tp,
                            mod_name,
                            entry,
                            rs_rel_path: rs_rel,
                            background: is_background,
                            is_kernel_builtin: is_kb,
                        });
                    }
                }
            }
        }

        // ── Non-builtin traits (source = "static", "dylib", etc.): register + discover JS/CSS assets ──
        if !is_builtin && !is_rest {
            let toml_dir = path.parent().unwrap();
            let rel_path = path.strip_prefix(manifest_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let trait_path = path.strip_prefix(traits_dir)
                .ok()
                .and_then(|p| p.to_str())
                .and_then(|s| s.strip_suffix(".trait.toml")
                    .or_else(|| s.strip_suffix(".strait.toml")))
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

            let serve_prefix = toml_dir.strip_prefix(traits_dir)
                .unwrap_or(toml_dir)
                .to_string_lossy()
                .replace('\\', "/");
            if let Ok(dir_entries) = fs::read_dir(toml_dir) {
                for de in dir_entries.flatten() {
                    let dp = de.path();
                    let fname = dp.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let content_type = if fname.ends_with(".css") {
                        Some("text/css")
                    } else if fname.ends_with(".js") {
                        Some("application/javascript")
                    } else {
                        None
                    };
                    if let Some(ct) = content_type {
                        static_assets.push(StaticAsset {
                            serve_path: format!("{}/{}", serve_prefix, fname),
                            abs_path: dp.to_string_lossy().to_string(),
                            content_type: ct,
                        });
                    }
                }
            }
        }
    }
}
