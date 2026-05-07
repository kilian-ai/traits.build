//! gen-component — Materialize WebAssembly Component Model delegator crates
//! from `*.trait.toml` files that opt in via `[component] auto = true`.
//!
//! For every opted-in trait this tool generates a crate under
//! `target/component-gen/<dotted-path>/` containing:
//!
//!   Cargo.toml       — minimal cargo-component crate, world = `<name>-world`
//!   wit/<name>.wit   — same shape as gen-wit emits, plus a kernel-host import
//!   wit/deps/...     — local copy of the kernel-host WIT contract
//!   src/lib.rs       — delegator shim that packs typed params into JSON and
//!                      calls `host::call` (or `call-native` for self-loops)
//!
//! The generated crates are designed to be built en-masse by
//! `build-components.sh` via `cargo component build --release` and have their
//! `.component.wasm` artefact copied next to each source `.trait.toml`.
//!
//! Usage:
//!     cargo run -p gen-component                 # scan ./traits, generate
//!     cargo run -p gen-component -- --check      # exit non-zero if stale
//!     cargo run -p gen-component -- <traits-dir> # custom source root

use kernel_logic::registry::{ComponentToml, ParamToml, TraitToml};
use kernel_logic::types::TraitType;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const OUT_DIR: &str = "target/component-gen";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut check = false;
    let mut root: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--check" => check = true,
            "-h" | "--help" => {
                eprintln!("usage: gen-component [--check] [<traits-root>]");
                return ExitCode::SUCCESS;
            }
            other => root = Some(PathBuf::from(other)),
        }
    }
    let traits_root = root.unwrap_or_else(|| PathBuf::from("traits"));
    if !traits_root.is_dir() {
        eprintln!("error: traits root not found: {}", traits_root.display());
        return ExitCode::from(2);
    }

    let out_root = PathBuf::from(OUT_DIR);

    let mut generated: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut stale: Vec<PathBuf> = Vec::new();
    let mut errors: Vec<(PathBuf, String)> = Vec::new();

    for entry in walkdir::WalkDir::new(&traits_root)
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
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                errors.push((path.to_path_buf(), e.to_string()));
                continue;
            }
        };
        let parsed: TraitToml = match toml::from_str(&raw) {
            Ok(p) => p,
            Err(e) => {
                errors.push((path.to_path_buf(), e.to_string()));
                continue;
            }
        };
        let comp = parsed.component.clone().unwrap_or_default();
        if !comp.auto {
            continue;
        }
        if parsed.signature.is_none() {
            errors.push((
                path.to_path_buf(),
                "[component].auto = true requires a [signature] section".into(),
            ));
            continue;
        }

        match render_crate(path, &parsed, &comp, &out_root, check) {
            Ok(GenOutcome::Wrote(p)) => {
                generated.push(p.display().to_string());
            }
            Ok(GenOutcome::Stale(paths)) => {
                stale.extend(paths);
            }
            Ok(GenOutcome::Unchanged) => {
                skipped.push(trait_path_from_file(path));
            }
            Err(e) => errors.push((path.to_path_buf(), e)),
        }
    }

    if check && !stale.is_empty() {
        eprintln!("stale generated files (run `cargo run -p gen-component`):");
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
        "gen-component: {} generated, {} unchanged{}",
        generated.len(),
        skipped.len(),
        if check { " (check mode)" } else { "" }
    );
    ExitCode::SUCCESS
}

enum GenOutcome {
    Wrote(PathBuf),
    Stale(Vec<PathBuf>),
    Unchanged,
}

fn render_crate(
    toml_path: &Path,
    parsed: &TraitToml,
    comp: &ComponentToml,
    out_root: &Path,
    check: bool,
) -> Result<GenOutcome, String> {
    let dotted = trait_path_from_file(toml_path);
    let last = dotted.rsplit('.').next().unwrap_or(&dotted).to_string();
    // Everything before the final segment is the namespace prefix; for a
    // trait `www.local.helper` the prefix is `www.local` and the leaf is
    // `helper`. We use the full prefix in the package name so deeply nested
    // traits get unique crate identifiers.
    let prefix_len = dotted.len().saturating_sub(last.len());
    let prefix = dotted[..prefix_len].trim_end_matches('.');
    let ns = if prefix.is_empty() { "traits".to_string() } else { prefix.to_string() };
    let kebab = sanitize_ident(&last);
    let ns_kebab = sanitize_ident(&ns);
    let pkg_name = format!("{ns_kebab}-{kebab}");
    let world = format!("{kebab}-world");

    // Crate dir: <out_root>/<dotted-path-with-dashes>/
    let crate_dir = out_root.join(dotted.replace('.', "-"));

    let cargo_toml = render_cargo_toml(&dotted, &pkg_name, &world);
    let wit_main = render_wit(&pkg_name, &kebab, &dotted, parsed);
    let wit_host = KERNEL_HOST_WIT.to_string();
    let lib_rs = render_lib_rs(&dotted, &kebab, &ns_kebab, parsed, comp);

    let cargo_path = crate_dir.join("Cargo.toml");
    let wit_path = crate_dir.join("wit").join(format!("{kebab}.wit"));
    let host_wit_path = crate_dir
        .join("wit/deps/kernel-host/kernel-host.wit");
    let lib_path = crate_dir.join("src/lib.rs");

    let plan = [
        (cargo_path.clone(), cargo_toml),
        (wit_path.clone(), wit_main),
        (host_wit_path.clone(), wit_host),
        (lib_path.clone(), lib_rs),
    ];

    let mut stale_paths: Vec<PathBuf> = Vec::new();
    let mut any_changed = false;
    for (p, content) in &plan {
        let existing = std::fs::read_to_string(p).ok();
        if existing.as_deref() != Some(content.as_str()) {
            stale_paths.push(p.clone());
            any_changed = true;
        }
    }

    if check {
        return Ok(if stale_paths.is_empty() {
            GenOutcome::Unchanged
        } else {
            GenOutcome::Stale(stale_paths)
        });
    }

    if !any_changed {
        return Ok(GenOutcome::Unchanged);
    }

    for (p, content) in &plan {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(p, content).map_err(|e| e.to_string())?;
    }
    Ok(GenOutcome::Wrote(crate_dir))
}

fn render_cargo_toml(dotted: &str, pkg_name: &str, world: &str) -> String {
    format!(
        r##"# @generated by gen-component for trait `{dotted}` — DO NOT EDIT
[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen-rt = {{ version = "0.39", features = ["bitflags"] }}
serde_json = "1"

[package.metadata.component]
package = "traits:{pkg_name}"

[package.metadata.component.target]
path = "wit"
world = "{world}"

[package.metadata.component.target.dependencies]
"traits:kernel-host" = {{ path = "wit/deps/kernel-host" }}

[profile.release]
opt-level = "s"
lto = true
strip = true
codegen-units = 1

# Generated crates live outside the main workspace.
[workspace]
"##,
        name = format!("{pkg_name}-component"),
    )
}

const KERNEL_HOST_WIT: &str = "// @generated by gen-component — DO NOT EDIT
package traits:kernel-host@0.1.0;

interface dispatch {
  /// Re-enter the kernel dispatcher with `args-json` (a serialised JSON array).
  /// Goes through the full cascade: components → dylibs → builtins.
  call: func(trait-path: string, args-json: string) -> result<string, string>;

  /// Same as `call`, but skips the component-model loader. Use this from a
  /// generated delegator that targets its own trait path so it forwards to
  /// the native (dylib/builtin) implementation without re-entering wasm.
  call-native: func(trait-path: string, args-json: string) -> result<string, string>;
}
";

fn wit_escape(name: &str) -> String {
    // WIT reserves a number of keywords that can collide with our trait stems
    // (most notably `list`). Escape with `%` per the WIT identifier syntax.
    const RESERVED: &[&str] = &[
        "list", "option", "result", "string", "bool", "char", "u8", "u16", "u32", "u64",
        "s8", "s16", "s32", "s64", "f32", "f64", "tuple", "record", "variant", "enum",
        "flags", "type", "interface", "world", "package", "import", "export", "use",
        "func", "as", "from", "static", "include", "with", "resource",
    ];
    if RESERVED.contains(&name) {
        format!("%{name}")
    } else {
        name.to_string()
    }
}

fn render_wit(pkg_name: &str, kebab: &str, dotted: &str, t: &TraitToml) -> String {
    let kebab_w = wit_escape(kebab);
    let mut out = String::new();
    out.push_str("// @generated by gen-component — DO NOT EDIT\n");
    out.push_str(&format!("// Trait path: {dotted}\n\n"));
    out.push_str(&format!("package traits:{pkg_name}@0.1.0;\n\n"));
    if !t.trait_def.description.is_empty() {
        for l in t.trait_def.description.lines() {
            out.push_str("/// ");
            out.push_str(l);
            out.push('\n');
        }
    }
    out.push_str(&format!("interface {kebab_w} {{\n"));

    let sig = t.signature.as_ref().expect("signature checked earlier");
    out.push_str(&format!("  {kebab_w}: func("));
    if sig.params.is_empty() {
        out.push(')');
    } else {
        out.push('\n');
        let last_idx = sig.params.len() - 1;
        for (i, p) in sig.params.iter().enumerate() {
            let comma = if i == last_idx { "" } else { "," };
            out.push_str(&format!(
                "    {}: {}{}\n",
                wit_escape(&sanitize_ident(&p.name)),
                wit_type_for_param(p),
                comma
            ));
        }
        out.push_str("  )");
    }
    // Always JSON-encode the return value as a string. The kernel host
    // auto-decodes JSON into the appropriate Value when consuming the result.
    out.push_str(" -> result<string, string>;\n");
    out.push_str("}\n\n");

    out.push_str(&format!("world {kebab}-world {{\n"));
    out.push_str("  import traits:kernel-host/dispatch@0.1.0;\n");
    out.push_str(&format!("  export {kebab_w};\n"));
    out.push_str("}\n");
    out
}

fn render_lib_rs(
    dotted: &str,
    kebab: &str,
    ns_kebab: &str,
    t: &TraitToml,
    comp: &ComponentToml,
) -> String {
    let target = comp
        .delegates_to
        .clone()
        .unwrap_or_else(|| dotted.to_string());
    // Self-target → call-native (skip component loader to avoid a loop).
    let host_fn = if target == dotted { "call_native" } else { "call" };
    let mod_kebab = ns_kebab.replace('-', "_");
    let kebab_underscore = kebab.replace('-', "_");

    let sig = t.signature.as_ref().expect("checked");

    // Build the typed Rust signature and the JSON-pack body.
    let mut rust_params: Vec<String> = Vec::new();
    let mut pack_lines: Vec<String> = Vec::new();
    for p in &sig.params {
        let pname = sanitize_ident(&p.name).replace('-', "_");
        let ty = kernel_logic::registry::parse_type(&p.param_type);
        let rust_ty = rust_type_for(&ty, p.is_optional());
        rust_params.push(format!("{pname}: {rust_ty}"));
        pack_lines.push(pack_arg(&pname, &ty, p.is_optional()));
    }
    let params_joined = rust_params.join(", ");
    let pack_body = pack_lines.join("\n        ");

    let header = format!(
        "//! @generated by gen-component for trait `{dotted}` — DO NOT EDIT\n\
         //!\n\
         //! Delegator component: forwards calls to `{target}` via the kernel-host\n\
         //! import (`{host_fn}`). Built automatically by `build-components.sh`\n\
         //! whenever the source `.trait.toml` declares `[component] auto = true`.\n\n"
    );

    format!(
        "{header}\
#[allow(warnings, clippy::all)]
mod bindings;

use bindings::traits::kernel_host::dispatch as host;

struct Component;

impl bindings::exports::traits::{mod_kebab}_{kebab_underscore}::{kebab_underscore}::Guest for Component {{
    fn {fn_name}({params}) -> Result<String, String> {{
        let mut __pkt: Vec<serde_json::Value> = Vec::new();
        {pack_body}
        let args_json = serde_json::to_string(&__pkt).unwrap_or_else(|_| \"[]\".into());
        host::{host_fn}(\"{target}\", &args_json)
    }}
}}

bindings::export!(Component with_types_in bindings);
",
        fn_name = kebab_underscore,
        params = params_joined,
        target = target,
        host_fn = host_fn,
        kebab_underscore = kebab_underscore,
        mod_kebab = mod_kebab,
        pack_body = pack_body,
    )
}

fn rust_type_for(t: &TraitType, optional: bool) -> String {
    let base = match t {
        TraitType::Int => "i64".to_string(),
        TraitType::Float => "f64".to_string(),
        TraitType::String => "String".to_string(),
        TraitType::Bool => "bool".to_string(),
        TraitType::Bytes => "Vec<u8>".to_string(),
        TraitType::Null => "()".to_string(),
        TraitType::Any => "String".to_string(),
        TraitType::Handle => "u64".to_string(),
        TraitType::List(inner) => format!("Vec<{}>", rust_type_for(inner, false)),
        TraitType::Map(_, _) => "Vec<(String, String)>".to_string(),
        TraitType::Optional(inner) => format!("Option<{}>", rust_type_for(inner, false)),
    };
    if optional && !matches!(t, TraitType::Optional(_)) {
        format!("Option<{base}>")
    } else {
        base
    }
}

/// Generate a Rust expression that pushes the parameter `pname` into `args`
/// as a JSON value. Optional params are wrapped in `if let Some`.
fn pack_arg(pname: &str, ty: &TraitType, optional: bool) -> String {
    let push = json_push_for(ty, pname);
    if optional || matches!(ty, TraitType::Optional(_)) {
        let inner_name = format!("__{pname}");
        let inner_push = json_push_for(unwrap_optional(ty), &inner_name);
        format!(
            "if let Some({inner}) = {pname}.as_ref() {{ {push} }}",
            inner = inner_name,
            pname = pname,
            push = inner_push
        )
    } else {
        push
    }
}

fn unwrap_optional(t: &TraitType) -> &TraitType {
    if let TraitType::Optional(inner) = t { inner } else { t }
}

fn json_push_for(ty: &TraitType, name: &str) -> String {
    match ty {
        TraitType::Int => format!("__pkt.push(serde_json::json!({name}));"),
        TraitType::Float => format!("__pkt.push(serde_json::json!({name}));"),
        TraitType::Bool => format!("__pkt.push(serde_json::json!({name}));"),
        TraitType::String => {
            format!("__pkt.push(serde_json::Value::String({name}.to_string()));")
        }
        TraitType::Bytes => format!("__pkt.push(serde_json::json!({name}));"),
        TraitType::Null => format!("__pkt.push(serde_json::Value::Null); let _ = {name};"),
        TraitType::Handle => format!("__pkt.push(serde_json::json!({name}));"),
        TraitType::Any => format!(
            "__pkt.push(serde_json::from_str(&{name}).unwrap_or_else(|_| serde_json::Value::String({name}.to_string())));"
        ),
        TraitType::List(inner) => {
            // For list<any> the elements arrive as opaque JSON-encoded strings
            // (Any → string in WIT). Parse each element back into a Value so
            // the receiving trait sees the structured form, mirroring what the
            // CLI passes to non-component traits.
            if matches!(inner.as_ref(), TraitType::Any) {
                format!(
                    "__pkt.push(serde_json::Value::Array({name}.iter().map(|s| serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.to_string()))).collect()));"
                )
            } else {
                format!("__pkt.push(serde_json::json!({name}));")
            }
        }
        TraitType::Map(_, _) => format!(
            "__pkt.push(serde_json::Value::Object({name}.iter().cloned().map(|(k,v)| (k, serde_json::Value::String(v))).collect()));"
        ),
        TraitType::Optional(inner) => json_push_for(inner, name),
    }
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
        TraitType::Any => "string".into(),
        TraitType::Handle => "u64".into(),
        TraitType::List(inner) => format!("list<{}>", wit_type_str(inner)),
        TraitType::Map(k, v) => {
            format!("list<tuple<{}, {}>>", wit_type_str(k), wit_type_str(v))
        }
        TraitType::Optional(inner) => format!("option<{}>", wit_type_str(inner)),
    }
}

fn trait_path_from_file(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
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
    out
}
