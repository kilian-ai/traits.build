//! WebAssembly Component Model host loader.
//!
//! Scans `traits/**/*.component.wasm` and registers each as a callable trait,
//! parallel to (and checked before) the dylib loader. Type marshaling between
//! JSON and the component-model `Val` ABI is driven by each trait's `.trait.toml`
//! signature — the same source of truth used everywhere else.
//!
//! Each component is expected to follow the gen-wit convention:
//!   package traits:<ns>-<name>@0.1.0;
//!   interface <name> { <name>: func(p1, p2, ...) -> result<T, string>; }
//!   world <name>-world { export <name>; }
//!
//! That is: one interface, one function, named after the trait's last segment,
//! returning `result<T, string>`.

use anyhow::{anyhow, Result};
use kernel_logic::registry::{parse_type, TraitToml};
use kernel_logic::types::{ParamDef, ReturnDef, TraitSignature, TraitType};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use tracing::{info, warn};
use wasmtime::component::{Component, Linker, Val};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

/// Per-store host state — required by wasmtime-wasi.
struct HostState {
    table: ResourceTable,
    wasi: WasiCtx,
}

impl WasiView for HostState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}

/// A loaded component, ready to be invoked.
struct LoadedComponent {
    component: Component,
    /// Cached export name of the interface, e.g. "traits:sys-checksum/checksum".
    interface_name: String,
    /// Cached export name of the function inside the interface.
    func_name: String,
    /// Signature used to marshal arguments and decode results.
    signature: TraitSignature,
    #[allow(dead_code)]
    wasm_path: PathBuf,
}

pub struct ComponentLoader {
    engine: Engine,
    linker: Arc<Linker<HostState>>,
    components: RwLock<HashMap<String, Arc<LoadedComponent>>>,
    search_dirs: Vec<PathBuf>,
    /// One Store is created per call (cheap with wasmtime engines), but we
    /// serialize calls per-component to avoid hitting any non-Send corners.
    call_lock: Mutex<()>,
}

impl ComponentLoader {
    pub fn new(search_dirs: Vec<PathBuf>) -> Result<Self> {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true);
        config.async_support(false);
        let engine = Engine::new(&config)?;

        let mut linker: Linker<HostState> = Linker::new(&engine);
        wasmtime_wasi::add_to_linker_sync(&mut linker)?;

        Ok(Self {
            engine,
            linker: Arc::new(linker),
            components: RwLock::new(HashMap::new()),
            search_dirs,
            call_lock: Mutex::new(()),
        })
    }

    /// Recursively scan search dirs for `*.component.wasm` files.
    /// The trait path is derived from the file's parent directory chain
    /// relative to a search dir (same rule as dylib_loader).
    pub fn load_all(&self) -> usize {
        let mut count = 0;
        for dir in &self.search_dirs {
            if !dir.exists() {
                continue;
            }
            for entry in walkdir::WalkDir::new(dir)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };
                if !name.ends_with(".component.wasm") {
                    continue;
                }
                match self.load_one(path, dir) {
                    Ok(tp) => {
                        info!(
                            "Loaded component trait: {} from {}",
                            tp,
                            path.display()
                        );
                        count += 1;
                    }
                    Err(e) => {
                        warn!("Failed to load component {}: {:#}", path.display(), e);
                    }
                }
            }
        }
        count
    }

    fn load_one(&self, wasm_path: &Path, search_dir: &Path) -> Result<String> {
        // Trait path from filesystem layout, e.g.
        //   traits/sys/checksum/checksum.component.wasm  →  sys.checksum
        let parent = wasm_path
            .parent()
            .ok_or_else(|| anyhow!("component file has no parent dir"))?;
        let rel = parent
            .strip_prefix(search_dir)
            .map_err(|_| anyhow!("component not under search dir"))?;
        let parts: Vec<&str> = rel
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();
        if parts.is_empty() {
            return Err(anyhow!("cannot derive trait path"));
        }
        let trait_path = parts.join(".");

        // Read the sibling trait.toml for signature info.
        let toml_name = format!("{}.trait.toml", parts.last().unwrap());
        let toml_path = parent.join(&toml_name);
        let signature = read_signature(&toml_path)
            .map_err(|e| anyhow!("read signature {}: {}", toml_path.display(), e))?;

        // Compile the component.
        let component = Component::from_file(&self.engine, wasm_path)?;

        // Convention: interface name = "traits:<ns>-<name>/<name>",
        //             function  name = "<name>"  (sanitized to kebab).
        let last = parts.last().unwrap();
        let kebab = last.replace('_', "-");
        let pkg_ns = parts[0];
        let interface_name = format!("traits:{}-{}/{}@0.1.0", pkg_ns, kebab, kebab);
        let func_name = kebab.clone();

        let loaded = LoadedComponent {
            component,
            interface_name,
            func_name,
            signature,
            wasm_path: wasm_path.to_path_buf(),
        };
        self.components
            .write()
            .unwrap()
            .insert(trait_path.clone(), Arc::new(loaded));
        Ok(trait_path)
    }

    /// Dispatch a call. Returns `None` if the trait path isn't loaded as a
    /// component; otherwise returns `Some(json)` with success or error embedded
    /// in the JSON (mirroring how dylib loader behaves).
    pub fn dispatch(&self, trait_path: &str, args: &[Value]) -> Option<Value> {
        let comp = self.components.read().ok()?.get(trait_path).cloned()?;
        // Serialize across calls — we only need correctness, not throughput.
        let _g = self.call_lock.lock().unwrap();
        match self.invoke(&comp, args) {
            Ok(v) => Some(v),
            Err(e) => Some(serde_json::json!({
                "error": format!("component dispatch failed: {:#}", e),
                "trait": trait_path,
            })),
        }
    }

    fn invoke(&self, comp: &LoadedComponent, args: &[Value]) -> Result<Value> {
        // Build a fresh store and instance for each call (the HostState owns
        // a WasiCtx with no inherited resources — safe and cheap).
        let host = HostState {
            table: ResourceTable::new(),
            wasi: WasiCtxBuilder::new().build(),
        };
        let mut store = Store::new(&self.engine, host);
        let instance = self.linker.instantiate(&mut store, &comp.component)?;

        // Navigate: root → interface → function.
        let iface_idx = instance
            .get_export(&mut store, None, &comp.interface_name)
            .ok_or_else(|| {
                anyhow!(
                    "interface export not found: {}",
                    comp.interface_name
                )
            })?;
        let func_idx = instance
            .get_export(&mut store, Some(&iface_idx), &comp.func_name)
            .ok_or_else(|| {
                anyhow!(
                    "function export not found: {}/{}",
                    comp.interface_name,
                    comp.func_name
                )
            })?;
        let func = instance
            .get_func(&mut store, func_idx)
            .ok_or_else(|| anyhow!("export is not a function"))?;

        // Marshal inputs.
        let params = &comp.signature.params;
        if args.len() != params.len() {
            return Err(anyhow!(
                "argument count mismatch: got {}, expected {}",
                args.len(),
                params.len()
            ));
        }
        let mut inputs: Vec<Val> = Vec::with_capacity(params.len());
        for (i, p) in params.iter().enumerate() {
            inputs.push(json_to_val(&args[i], &p.param_type).map_err(|e| {
                anyhow!("param '{}' ({}): {}", p.name, type_name(&p.param_type), e)
            })?);
        }

        // gen-wit always wraps return as result<T, string>, so we get one Val.
        let mut outputs = vec![Val::Bool(false)];
        func.call(&mut store, &inputs, &mut outputs)?;
        func.post_return(&mut store)?;

        let raw = outputs.into_iter().next().unwrap();
        result_val_to_json(raw, &comp.signature.returns)
    }

    pub fn list(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .components
            .read()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    pub fn search_dirs(&self) -> &[PathBuf] {
        &self.search_dirs
    }
}

// ── trait.toml signature reader (lightweight; reuses kernel-logic types) ──

fn read_signature(toml_path: &Path) -> Result<TraitSignature> {
    let raw = std::fs::read_to_string(toml_path)?;
    let parsed: TraitToml = toml::from_str(&raw).map_err(|e| anyhow!("toml parse: {}", e))?;
    let sig = parsed
        .signature
        .ok_or_else(|| anyhow!("[signature] missing in {}", toml_path.display()))?;
    let params = sig
        .params
        .into_iter()
        .map(|p| ParamDef {
            name: p.name,
            param_type: parse_type(&p.param_type),
            description: p.description,
            optional: p.optional,
            pipe: p.pipe,
            example: None,
        })
        .collect();
    let returns = ReturnDef {
        return_type: parse_type(&sig.returns.return_type),
        description: sig.returns.description,
    };
    Ok(TraitSignature { params, returns })
}

// ── JSON ↔ Val marshaling ──

fn type_name(t: &TraitType) -> &'static str {
    match t {
        TraitType::Int => "int",
        TraitType::Float => "float",
        TraitType::String => "string",
        TraitType::Bool => "bool",
        TraitType::Bytes => "bytes",
        TraitType::Null => "null",
        TraitType::List(_) => "list",
        TraitType::Map(_, _) => "map",
        TraitType::Optional(_) => "optional",
        TraitType::Any => "any",
        TraitType::Handle => "handle",
    }
}

fn json_to_val(v: &Value, t: &TraitType) -> Result<Val> {
    use TraitType as T;
    Ok(match t {
        T::Int => Val::S64(
            v.as_i64()
                .ok_or_else(|| anyhow!("expected int, got {}", v))?,
        ),
        T::Float => Val::Float64(
            v.as_f64()
                .ok_or_else(|| anyhow!("expected float, got {}", v))?,
        ),
        T::Bool => Val::Bool(
            v.as_bool()
                .ok_or_else(|| anyhow!("expected bool, got {}", v))?,
        ),
        T::String => Val::String(
            v.as_str()
                .ok_or_else(|| anyhow!("expected string, got {}", v))?
                .to_string(),
        ),
        T::Bytes => {
            // Accept hex string (matches TraitValue::to_json) or array of ints.
            let bytes = if let Some(s) = v.as_str() {
                hex::decode(s).map_err(|e| anyhow!("bytes hex decode: {}", e))?
            } else if let Some(arr) = v.as_array() {
                arr.iter()
                    .map(|x| {
                        x.as_u64()
                            .ok_or_else(|| anyhow!("byte not u64"))
                            .and_then(|n| {
                                u8::try_from(n).map_err(|_| anyhow!("byte > 255"))
                            })
                    })
                    .collect::<Result<Vec<u8>>>()?
            } else {
                return Err(anyhow!("expected bytes (hex string or u8 array)"));
            };
            Val::List(bytes.into_iter().map(Val::U8).collect())
        }
        T::Null => Val::Tuple(vec![]), // gen-wit emits `_` (empty tuple) for Null
        T::List(inner) => {
            let arr = v
                .as_array()
                .ok_or_else(|| anyhow!("expected list, got {}", v))?;
            let items: Result<Vec<Val>> = arr.iter().map(|x| json_to_val(x, inner)).collect();
            Val::List(items?)
        }
        T::Map(k_t, v_t) => {
            // gen-wit emits map<K,V> as list<tuple<K,V>>
            let obj = v
                .as_object()
                .ok_or_else(|| anyhow!("expected object for map, got {}", v))?;
            let mut entries = Vec::with_capacity(obj.len());
            for (k, val) in obj {
                let key_val = json_to_val(&Value::String(k.clone()), k_t)?;
                let v_val = json_to_val(val, v_t)?;
                entries.push(Val::Tuple(vec![key_val, v_val]));
            }
            Val::List(entries)
        }
        T::Optional(inner) => {
            if v.is_null() {
                Val::Option(None)
            } else {
                Val::Option(Some(Box::new(json_to_val(v, inner)?)))
            }
        }
        T::Any => {
            // gen-wit maps Any → string (opaque JSON).
            Val::String(serde_json::to_string(v).unwrap_or_default())
        }
        T::Handle => Val::U64(
            v.as_u64()
                .ok_or_else(|| anyhow!("expected handle (u64), got {}", v))?,
        ),
    })
}

fn val_to_json(v: Val) -> Value {
    match v {
        Val::Bool(b) => Value::Bool(b),
        Val::S8(n) => Value::from(n as i64),
        Val::U8(n) => Value::from(n as u64),
        Val::S16(n) => Value::from(n as i64),
        Val::U16(n) => Value::from(n as u64),
        Val::S32(n) => Value::from(n as i64),
        Val::U32(n) => Value::from(n as u64),
        Val::S64(n) => Value::from(n),
        Val::U64(n) => Value::from(n),
        Val::Float32(f) => serde_json::Number::from_f64(f as f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Val::Float64(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Val::Char(c) => Value::String(c.to_string()),
        Val::String(s) => Value::String(s),
        Val::List(items) => Value::Array(items.into_iter().map(val_to_json).collect()),
        Val::Record(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(k, v)| (k, val_to_json(v)))
                .collect(),
        ),
        Val::Tuple(items) => Value::Array(items.into_iter().map(val_to_json).collect()),
        Val::Variant(case, payload) => {
            let mut obj = serde_json::Map::new();
            obj.insert(
                case,
                payload.map(|p| val_to_json(*p)).unwrap_or(Value::Null),
            );
            Value::Object(obj)
        }
        Val::Enum(case) => Value::String(case),
        Val::Option(inner) => match inner {
            None => Value::Null,
            Some(b) => val_to_json(*b),
        },
        Val::Result(r) => match r {
            Ok(opt) => opt.map(|p| val_to_json(*p)).unwrap_or(Value::Null),
            Err(opt) => Value::Object({
                let mut m = serde_json::Map::new();
                m.insert(
                    "error".into(),
                    opt.map(|p| val_to_json(*p)).unwrap_or(Value::Null),
                );
                m
            }),
        },
        Val::Flags(flags) => Value::Array(flags.into_iter().map(Value::String).collect()),
        Val::Resource(_) => Value::String("<resource>".into()),
        _ => Value::Null,
    }
}

/// Decode the gen-wit `result<T, string>` return value.
/// On Err we surface `{"error": msg, ...}` so the kernel can see it.
///
/// If the declared return type is structured (Map/List/Any) but the component
/// returned a JSON-encoded string (a common WIT idiom because WIT lacks
/// dynamic types), auto-decode that string back into JSON so callers see
/// the same shape as the dylib path.
fn result_val_to_json(v: Val, ret: &ReturnDef) -> Result<Value> {
    let normalize = |raw: Value| -> Value {
        if let Value::String(ref s) = raw {
            let want_structured = matches!(
                ret.return_type,
                TraitType::Map(_, _)
                    | TraitType::List(_)
                    | TraitType::Any
                    | TraitType::Optional(_)
            );
            if want_structured {
                if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                    return parsed;
                }
            }
        }
        raw
    };

    match v {
        Val::Result(Ok(opt)) => {
            let raw = opt.map(|p| val_to_json(*p)).unwrap_or(Value::Null);
            Ok(normalize(raw))
        }
        Val::Result(Err(opt)) => {
            let msg = opt.map(|p| val_to_json(*p)).unwrap_or(Value::Null);
            Ok(serde_json::json!({
                "ok": false,
                "error": msg,
            }))
        }
        // Some traits may declare a non-result return; pass through.
        other => Ok(normalize(val_to_json(other))),
    }
}

// ── Global handle (mirrors dylib_loader::LOADER) ──

pub static LOADER: std::sync::OnceLock<Arc<ComponentLoader>> = std::sync::OnceLock::new();

pub fn set_global_loader(loader: Arc<ComponentLoader>) {
    let _ = LOADER.set(loader);
}

/// Trait entry point (`sys.component_loader`): returns introspection info.
pub fn component_loader_info(_args: &[Value]) -> Value {
    match LOADER.get() {
        Some(l) => serde_json::json!({
            "loaded_count": l.list().len(),
            "loaded_traits": l.list(),
            "search_dirs": l.search_dirs().iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        }),
        None => serde_json::json!({
            "loaded_count": 0,
            "loaded_traits": [],
            "status": "not initialized"
        }),
    }
}
