//! gen-trait — register a foreign Component Model component as a traits.build trait.
//!
//! Workflow:
//!   1. Caller produces a `.component.wasm` in any language (Rust, Go via TinyGo,
//!      JS via jco, Python via componentize-py, C/C++ via wasi-sdk + wasm-tools,
//!      …) targeting a WIT contract whose exported world contains a single
//!      interface whose stem matches its single function (the same convention
//!      `gen-component` emits).
//!   2. `cargo run -p gen-trait -- <wasm>` decodes the WIT, picks the export,
//!      writes `traits/<ns>/<name>/<name>.trait.toml`, and copies the wasm
//!      next to it as `<name>.component.wasm`.
//!   3. Restart `traits` (or call `sys.component_loader`) — the trait is now
//!      live with full host-side cascade and `kernel.call` access.
//!
//! No Rust required from the foreign author.

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};
use wit_component::DecodedWasm;
use wit_parser::{Resolve, Type, TypeDefKind, WorldItem};

#[derive(Parser, Debug)]
#[command(
    name = "gen-trait",
    about = "Register a foreign component as a traits.build trait"
)]
struct Args {
    /// Path to the `.component.wasm` produced by any Component Model toolchain.
    wasm: PathBuf,

    /// Override the resulting trait path (dot-notation). Default: derived
    /// from the component's WIT package + interface name.
    #[arg(long, value_name = "DOTTED")]
    r#as: Option<String>,

    /// Root traits directory (defaults to `./traits`).
    #[arg(long, default_value = "traits", value_name = "DIR")]
    traits_root: PathBuf,

    /// Print what would be written without touching the filesystem.
    #[arg(long)]
    dry_run: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let bytes = std::fs::read(&args.wasm)
        .with_context(|| format!("read wasm {}", args.wasm.display()))?;

    let decoded = wit_component::decode(&bytes).context("decode component WIT")?;
    let (resolve, world_id) = match decoded {
        DecodedWasm::Component(resolve, world) => (resolve, world),
        DecodedWasm::WitPackage(_, _) => bail!("input is a raw WIT package, not a component"),
    };

    let world = &resolve.worlds[world_id];

    // Find a single exported interface that contains a single function.
    // Prefer named (`WorldKey::Interface`) exports — anonymous inline exports
    // (`WorldKey::Name`) are typically toolchain-internal (e.g.
    // componentize-py's runtime "exports" interface) and not what users mean
    // to expose.
    let mut iface_pick = None;
    for (key, item) in &world.exports {
        if !matches!(key, wit_parser::WorldKey::Interface(_)) {
            continue;
        }
        if let WorldItem::Interface { id, .. } = item {
            let iface = &resolve.interfaces[*id];
            if iface.functions.len() != 1 {
                continue;
            }
            let (fn_name, func) = iface.functions.iter().next().unwrap();
            iface_pick = Some((key.clone(), *id, fn_name.clone(), func.clone()));
            break;
        }
    }
    // Fallback: accept anonymous/inline exports too if no named interface
    // matched (covers components built without a package-level WIT world).
    if iface_pick.is_none() {
        for (key, item) in &world.exports {
            if let WorldItem::Interface { id, .. } = item {
                let iface = &resolve.interfaces[*id];
                if iface.functions.len() != 1 {
                    continue;
                }
                let (fn_name, func) = iface.functions.iter().next().unwrap();
                iface_pick = Some((key.clone(), *id, fn_name.clone(), func.clone()));
                break;
            }
        }
    }
    let (key, iface_id, fn_name, func) = iface_pick
        .ok_or_else(|| anyhow!("no exported interface with a single function found in component"))?;

    // ── Derive trait path ──
    let dotted = if let Some(v) = args.r#as.clone() {
        v
    } else {
        derive_trait_path(&resolve, iface_id, &fn_name)?
    };
    let parts: Vec<&str> = dotted.split('.').collect();
    if parts.len() < 2 {
        bail!(
            "trait path '{}' must have at least one namespace segment (e.g. 'foo.bar')",
            dotted
        );
    }

    // ── Derive signature ──
    let mut params_toml = String::new();
    for (pname, ty) in &func.params {
        let (toml_ty, optional) = wit_to_trait_type(&resolve, ty);
        let opt_attr = if optional { ", optional = true" } else { "" };
        params_toml.push_str(&format!(
            "  {{ name = \"{name}\", type = \"{ty}\", description = \"{name}\"{opt} }},\n",
            name = sanitize_param_name(pname),
            ty = toml_ty,
            opt = opt_attr,
        ));
    }
    let return_ty_str = match &func.result {
        Some(r) => {
            let (s, _opt) = wit_to_trait_type(&resolve, r);
            s
        }
        None => "any".to_string(),
    };

    // The on-disk path mirrors the dotted trait path.
    let trait_dir: PathBuf = parts.iter().fold(args.traits_root.clone(), |acc, p| acc.join(p));
    let leaf = parts.last().unwrap();
    let toml_path = trait_dir.join(format!("{leaf}.trait.toml"));
    let wasm_dst = trait_dir.join(format!("{leaf}.component.wasm"));

    let pkg_id = resolve.interfaces[iface_id]
        .package
        .ok_or_else(|| anyhow!("interface has no package"))?;
    let pkg = &resolve.packages[pkg_id];
    let pkg_name = format!("{}:{}", pkg.name.namespace, pkg.name.name);
    let iface_name = match &key {
        wit_parser::WorldKey::Name(s) => s.clone(),
        wit_parser::WorldKey::Interface(id) => {
            resolve.interfaces[*id].name.clone().unwrap_or_default()
        }
    };

    let toml = render_trait_toml(
        &dotted,
        &pkg_name,
        &iface_name,
        &fn_name,
        &params_toml,
        &return_ty_str,
        &args.wasm,
    );

    println!("gen-trait: source     = {}", args.wasm.display());
    println!("gen-trait: package    = {pkg_name}");
    println!("gen-trait: interface  = {iface_name}");
    println!("gen-trait: function   = {fn_name}");
    println!("gen-trait: trait path = {dotted}");
    println!("gen-trait: target dir = {}", trait_dir.display());

    if args.dry_run {
        println!("\n--- {} ---", toml_path.display());
        println!("{toml}");
        return Ok(());
    }

    if toml_path.exists() {
        bail!(
            "{} already exists — refusing to overwrite (delete it first or pass --as)",
            toml_path.display()
        );
    }
    std::fs::create_dir_all(&trait_dir)
        .with_context(|| format!("mkdir {}", trait_dir.display()))?;
    std::fs::write(&toml_path, toml)
        .with_context(|| format!("write {}", toml_path.display()))?;
    std::fs::copy(&args.wasm, &wasm_dst)
        .with_context(|| format!("copy {} → {}", args.wasm.display(), wasm_dst.display()))?;

    println!("\n✓ wrote {}", toml_path.display());
    println!("✓ wrote {}", wasm_dst.display());
    println!("\nNext: restart `traits` (or call sys.component_loader) and try:");
    println!("    traits call {dotted}");

    Ok(())
}

fn derive_trait_path(resolve: &Resolve, iface_id: wit_parser::InterfaceId, fn_name: &str) -> Result<String> {
    let iface = &resolve.interfaces[iface_id];
    let pkg = iface
        .package
        .ok_or_else(|| anyhow!("interface has no package"))?;
    let pkg = &resolve.packages[pkg];
    let ns_field = &pkg.name.namespace;
    let name_field = &pkg.name.name; // e.g. "my-pkg-foo"

    // Convention (mirrors gen-component): package name segments become the
    // dot-path namespace; the interface name is the leaf. If the package has
    // a single segment (e.g. `traits:foo`), treat it as `<ns>.foo`.
    //
    // External authors who picked their own namespace just get whatever they
    // wrote (e.g. package `mycorp:utils-greet` → `mycorp.utils.greet`).
    let iface_kebab = iface
        .name
        .clone()
        .or_else(|| Some(fn_name.to_string()))
        .unwrap();
    let ns_prefix = if ns_field == "traits" {
        // strip the leaf from the package name: `sys-echo` → `sys`, `www-local-helper` → `www-local`
        let segs: Vec<&str> = name_field.split('-').collect();
        if segs.len() <= 1 {
            "external".to_string()
        } else {
            segs[..segs.len() - 1].join(".")
        }
    } else {
        // External vendor — combine namespace + package, drop the leaf if it
        // duplicates the interface name (so `mycorp:greet` interface `greet`
        // becomes `mycorp.greet`, not `mycorp.greet.greet`).
        let segs: Vec<&str> = name_field.split('-').collect();
        let leaf = iface_kebab.replace('_', "-");
        let trimmed: Vec<&str> = if segs.last().map(|s| *s) == Some(leaf.as_str()) {
            segs[..segs.len() - 1].to_vec()
        } else {
            segs.clone()
        };
        let mut prefix = ns_field.replace('-', ".");
        if !trimmed.is_empty() {
            prefix.push('.');
            prefix.push_str(&trimmed.join("."));
        }
        prefix
    };

    let leaf = iface_kebab.replace('-', "_");
    Ok(format!("{ns_prefix}.{leaf}"))
}

/// Map a WIT `Type` to (trait-toml type string, is_optional).
fn wit_to_trait_type(resolve: &Resolve, ty: &Type) -> (String, bool) {
    match ty {
        Type::Bool => ("bool".to_string(), false),
        Type::U8 | Type::U16 | Type::U32 | Type::U64 => ("int".to_string(), false),
        Type::S8 | Type::S16 | Type::S32 | Type::S64 => ("int".to_string(), false),
        Type::F32 | Type::F64 => ("float".to_string(), false),
        Type::Char | Type::String => ("string".to_string(), false),
        Type::Id(id) => match &resolve.types[*id].kind {
            TypeDefKind::Option(inner) => {
                let (s, _) = wit_to_trait_type(resolve, inner);
                (s, true)
            }
            TypeDefKind::List(inner) => {
                if matches!(inner, Type::U8) {
                    ("bytes".to_string(), false)
                } else {
                    let (s, _) = wit_to_trait_type(resolve, inner);
                    (format!("list<{s}>"), false)
                }
            }
            // Convention: foreign components wrap their real return in
            // `result<T, string>` so the host can surface errors. Unwrap T.
            TypeDefKind::Result(r) => match r.ok.as_ref() {
                Some(t) => wit_to_trait_type(resolve, t),
                None => ("null".to_string(), false),
            },
            TypeDefKind::Tuple(t) if t.types.is_empty() => ("null".to_string(), false),
            TypeDefKind::Type(inner) => wit_to_trait_type(resolve, inner),
            // Records / variants / enums / flags — flatten to opaque any.
            // The trait still works; the host marshals them as JSON strings.
            _ => ("any".to_string(), false),
        },
        // wit-parser may grow new primitive variants; default safely.
        _ => ("any".to_string(), false),
    }
}

fn sanitize_param_name(name: &str) -> String {
    name.replace('-', "_")
}

fn render_trait_toml(
    dotted: &str,
    pkg_name: &str,
    iface_name: &str,
    fn_name: &str,
    params_toml: &str,
    return_ty: &str,
    source_path: &Path,
) -> String {
    let leaf = dotted.rsplit('.').next().unwrap_or(dotted);
    format!(
        r##"# @generated by gen-trait — DO NOT EDIT (re-run gen-trait to refresh)
#
# This trait wraps an external Component Model component imported via gen-trait.
# Source wasm:    {src}
# WIT package:    {pkg_name}
# WIT interface:  {iface_name}/{fn_name}

[trait]
description = "External component {dotted} (imported via gen-trait)"
tags = ["external", "component"]

[signature]
params = [
{params}]

[signature.returns]
type = "{ret}"
description = "Result returned by the foreign component (wrapped in result<T, string>)."

[implementation]
# `language` here is descriptive metadata — the actual implementation is the
# adjacent `{leaf}.component.wasm`, which the kernel's ComponentLoader picks
# up via filesystem path. Use whatever value reflects how the component was
# authored (rust, go, javascript, python, c, c++, …).
language = "rust"
source = "builtin"
entry = "{leaf}"
"##,
        dotted = dotted,
        src = source_path.display(),
        pkg_name = pkg_name,
        iface_name = iface_name,
        fn_name = fn_name,
        params = params_toml,
        ret = return_ty,
        leaf = leaf,
    )
}
