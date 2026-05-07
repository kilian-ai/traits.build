//! gen-wit — Convert every `*.trait.toml` in the workspace into a sibling
//! `*.wit` file describing the trait as a WebAssembly Component Model world.
//!
//! Stage 1 of the component-model migration: WIT is a *derived artifact*.
//! Nothing at runtime consumes these files yet — they are documentation /
//! contract that downstream stages (wasmtime host, jco transpile) will use.
//!
//! Usage:
//!     cargo run -p gen-wit            # scan workspace, write/update .wit files
//!     cargo run -p gen-wit -- --check # exit non-zero if any file is stale
//!     cargo run -p gen-wit -- <root>  # custom traits root

use kernel_logic::registry::{ParamToml, TraitToml};
use kernel_logic::types::TraitType;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut check = false;
    let mut root: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--check" => check = true,
            "-h" | "--help" => {
                eprintln!("usage: gen-wit [--check] [<traits-root>]");
                return ExitCode::SUCCESS;
            }
            other => root = Some(PathBuf::from(other)),
        }
    }
    let root = root.unwrap_or_else(|| PathBuf::from("traits"));

    if !root.is_dir() {
        eprintln!("error: traits root not found: {}", root.display());
        return ExitCode::from(2);
    }

    let mut written = 0usize;
    let mut unchanged = 0usize;
    let mut stale: Vec<PathBuf> = Vec::new();
    let mut errors: Vec<(PathBuf, String)> = Vec::new();

    for entry in walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !name.ends_with(".trait.toml") {
            continue;
        }
        match generate_one(path) {
            Ok((wit_path, content)) => {
                let existing = std::fs::read_to_string(&wit_path).ok();
                if existing.as_deref() == Some(content.as_str()) {
                    unchanged += 1;
                } else if check {
                    stale.push(wit_path);
                } else {
                    if let Err(e) = std::fs::write(&wit_path, &content) {
                        errors.push((wit_path, e.to_string()));
                    } else {
                        written += 1;
                    }
                }
            }
            Err(e) => errors.push((path.to_path_buf(), e)),
        }
    }

    if check && !stale.is_empty() {
        eprintln!("stale WIT files (run `cargo run -p gen-wit` to update):");
        for p in &stale {
            eprintln!("  {}", p.display());
        }
        return ExitCode::from(1);
    }

    if !errors.is_empty() {
        eprintln!("errors:");
        for (p, e) in &errors {
            eprintln!("  {}: {}", p.display(), e);
        }
        return ExitCode::from(1);
    }

    eprintln!(
        "gen-wit: {} written, {} unchanged{}",
        written,
        unchanged,
        if check { " (check mode)" } else { "" }
    );
    ExitCode::SUCCESS
}

fn generate_one(toml_path: &Path) -> Result<(PathBuf, String), String> {
    let raw = std::fs::read_to_string(toml_path).map_err(|e| e.to_string())?;
    let parsed: TraitToml = toml::from_str(&raw).map_err(|e| e.to_string())?;

    // The basename without the .trait.toml suffix is the trait's local name.
    let file_stem = toml_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid filename")?
        .trim_end_matches(".trait.toml");

    // Namespace = parent dir name's parent (e.g. traits/sys/checksum/checksum.trait.toml → "sys")
    // We derive the canonical dotted path from the file path, then split on '.'.
    let dotted = trait_path_from_file(toml_path);
    let ns_root = dotted.split('.').next().unwrap_or("traits").to_string();

    let local = sanitize_ident(file_stem);
    let pkg_name = format!("{}-{}", sanitize_ident(&ns_root), local);

    let wit = render_wit(&pkg_name, &local, &dotted, &parsed);

    let wit_path = toml_path.with_file_name(format!("{file_stem}.wit"));
    Ok((wit_path, wit))
}

/// Mirror of registry.rs path-derivation logic, simplified.
fn trait_path_from_file(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    // Strip everything up to and including the last "traits/"
    let after = match s.rfind("/traits/") {
        Some(i) => &s[i + "/traits/".len()..],
        None => s.trim_start_matches("traits/"),
    };
    let stripped = after
        .strip_suffix(".trait.toml")
        .or_else(|| after.strip_suffix(".strait.toml"))
        .unwrap_or(after);
    let mut parts: Vec<&str> = stripped.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() >= 2 && parts[parts.len() - 1] == parts[parts.len() - 2] {
        parts.pop();
    }
    parts.join(".")
}

fn render_wit(pkg_name: &str, local: &str, dotted: &str, t: &TraitToml) -> String {
    let mut out = String::new();
    out.push_str("// @generated from ");
    out.push_str(local);
    out.push_str(".trait.toml — DO NOT EDIT\n");
    out.push_str("// Regenerate with: cargo run -p gen-wit\n");
    out.push_str("//\n");
    out.push_str("// Trait path: ");
    out.push_str(dotted);
    out.push_str("\n");
    out.push_str("// Source version: ");
    out.push_str(&t.trait_def.version);
    out.push_str("\n\n");

    // Stage 1 always emits package version 0.1.0 — the original semver-incompatible
    // trait version is preserved in the comment header above.
    out.push_str(&format!("package traits:{pkg_name}@0.1.0;\n\n"));

    // Doc comment for the interface = trait description.
    for line in wrap_doc(&t.trait_def.description) {
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str(&format!("interface {local} {{\n"));

    if let Some(sig) = &t.signature {
        // Per-action / per-trait function — currently traits expose ONE signature.
        // The function name is the kebab-case local trait name.
        let fn_name = local;

        // Function-level doc: collect param descriptions, examples, pipe notes.
        let fn_docs = build_fn_docs(t);
        for line in fn_docs {
            out.push_str("  ");
            out.push_str(&line);
            out.push('\n');
        }

        // Render signature.
        out.push_str(&format!("  {fn_name}: func("));
        if sig.params.is_empty() {
            out.push(')');
        } else {
            out.push('\n');
            let last = sig.params.len() - 1;
            for (i, p) in sig.params.iter().enumerate() {
                let pname = sanitize_ident(&p.name);
                let pty = wit_type_for_param(p);
                let comma = if i == last { "" } else { "," };
                out.push_str(&format!("    {pname}: {pty}{comma}\n"));
            }
            out.push_str("  )");
        }

        let ret = wit_type_str(&kernel_logic::registry::parse_type(&sig.returns.return_type));
        // Wrap in result<T, string> to model dispatcher errors. For void/null returns
        // use the WIT placeholder `_`.
        let ret_inner = if ret == "_" { "_".to_string() } else { ret };
        out.push_str(&format!(" -> result<{ret_inner}, string>;\n"));
    } else {
        out.push_str("  // No signature declared in trait.toml — metadata-only trait.\n");
    }

    out.push_str("}\n\n");

    // World re-exports the interface so consumers can `world.import` or `world.export`.
    out.push_str(&format!("world {local}-world {{\n"));
    out.push_str(&format!("  export {local};\n"));
    out.push_str("}\n");

    out
}

fn build_fn_docs(t: &TraitToml) -> Vec<String> {
    let mut docs: Vec<String> = Vec::new();
    if !t.trait_def.description.is_empty() {
        for line in wrap_doc(&t.trait_def.description) {
            docs.push(line);
        }
    }
    if let Some(sig) = &t.signature {
        if !sig.params.is_empty() {
            docs.push("///".into());
            docs.push("/// Parameters:".into());
            for p in &sig.params {
                let opt = if p.is_optional() { " (optional)" } else { "" };
                let pipe = if p.pipe { " (accepts stdin)" } else { "" };
                let desc = if p.description.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", p.description)
                };
                let example = match &p.example {
                    Some(v) => format!(" [example: {}]", toml_value_compact(v)),
                    None => String::new(),
                };
                docs.push(format!(
                    "/// - `{}`: {}{}{}{}{}",
                    p.name, p.param_type, opt, pipe, desc, example
                ));
            }
        }
        if !sig.returns.description.is_empty() {
            docs.push("///".into());
            docs.push(format!("/// Returns: {}", sig.returns.description));
        }
    }
    docs
}

fn toml_value_compact(v: &toml::Value) -> String {
    let s = match v {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let s = s.replace('\n', " ").replace('\r', " ");
    if s.len() > 80 { format!("{}…", &s[..80]) } else { s }
}

fn wrap_doc(desc: &str) -> Vec<String> {
    if desc.is_empty() {
        return Vec::new();
    }
    desc.lines().map(|l| format!("/// {}", l)).collect()
}

fn wit_type_for_param(p: &ParamToml) -> String {
    let t = kernel_logic::registry::parse_type(&p.param_type);
    let base = wit_type_str(&t);
    if p.is_optional() && !matches!(t, TraitType::Optional(_)) {
        format!("option<{base}>")
    } else {
        base
    }
}

fn wit_type_str(t: &TraitType) -> String {
    match t {
        TraitType::Int => "s64".into(),
        TraitType::Float => "f64".into(),
        TraitType::String => "string".into(),
        TraitType::Bool => "bool".into(),
        TraitType::Bytes => "list<u8>".into(),
        TraitType::Null => "_".into(),
        // No native dynamic type in WIT — the dispatcher already passes JSON
        // strings on the wire, so model `any` and bare maps as opaque JSON.
        TraitType::Any => "string".into(),
        TraitType::Handle => "u64".into(),
        TraitType::List(inner) => format!("list<{}>", wit_type_str(inner)),
        TraitType::Map(k, v) => format!(
            "list<tuple<{}, {}>>",
            wit_type_str(k),
            wit_type_str(v)
        ),
        TraitType::Optional(inner) => format!("option<{}>", wit_type_str(inner)),
    }
}

/// WIT identifiers are kebab-case ASCII. Reserved keywords are escaped with `%`.
fn sanitize_ident(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut prev_dash = false;
    for ch in lower.chars() {
        let c = match ch {
            '_' | '.' | ' ' | '/' => '-',
            c if c.is_ascii_alphanumeric() || c == '-' => c,
            _ => '-',
        };
        if c == '-' {
            if prev_dash || out.is_empty() {
                continue;
            }
            prev_dash = true;
        } else {
            prev_dash = false;
        }
        out.push(c);
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        return "x".into();
    }
    if out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) || is_wit_keyword(&out) {
        format!("%{out}")
    } else {
        out
    }
}

fn is_wit_keyword(s: &str) -> bool {
    matches!(
        s,
        "use" | "type" | "interface" | "world" | "package" | "export" | "import"
            | "func" | "record" | "variant" | "enum" | "flags" | "resource"
            | "list" | "option" | "result" | "tuple" | "string" | "bool" | "char"
            | "s8" | "s16" | "s32" | "s64" | "u8" | "u16" | "u32" | "u64"
            | "f32" | "f64" | "future" | "stream" | "static" | "constructor"
            | "include" | "with" | "as" | "own" | "borrow"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_basic() {
        assert_eq!(sanitize_ident("foo_bar"), "foo-bar");
        assert_eq!(sanitize_ident("FooBar"), "foobar");
        assert_eq!(sanitize_ident("a.b.c"), "a-b-c");
        assert_eq!(sanitize_ident("9foo"), "%9foo");
        assert_eq!(sanitize_ident("interface"), "%interface");
    }

    #[test]
    fn type_mapping() {
        assert_eq!(wit_type_str(&TraitType::Int), "s64");
        assert_eq!(
            wit_type_str(&TraitType::List(Box::new(TraitType::String))),
            "list<string>"
        );
        assert_eq!(
            wit_type_str(&TraitType::Map(
                Box::new(TraitType::String),
                Box::new(TraitType::Int)
            )),
            "list<tuple<string, s64>>"
        );
    }
}
