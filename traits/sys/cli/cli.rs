use crate::config::Config;
use crate::types::TraitValue;
use crate::cli::{CliSession, CliBackend, CLEAR_SENTINEL};
use std::io::Read;

use clap::{Parser, Subcommand};

// ────────────────── CLI arg parsing (clap) ──────────────────

#[derive(Parser)]
#[command(
    name = "traits",
    about = "Trait plugin system",
    after_help = "Any subcommand not listed above is dispatched as sys.<name> (or kernel.<name>).\n\
                  Examples:\n  \
                    traits serve              → kernel.serve (default)\n  \
                    traits list               → sys.list\n  \
                    traits test_runner '*'    → sys.test_runner\n  \
                    traits call sys.checksum  → call any trait by full path\n\n\
                  Run `traits list` to see all available traits."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Call a trait by full path (e.g., sys.checksum)
    Call {
        /// Trait path in dot notation (e.g., sys.checksum)
        path: String,
        /// Interactive mode: prompt for each parameter
        #[arg(short = 'i', long = "interactive")]
        interactive: bool,
        /// Arguments as JSON values or --flag value pairs
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Any other subcommand is dispatched as sys.<name>
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Entry point: parse CLI args, load config, dispatch.
/// Called from main.rs — all logic lives here.
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load("traits.toml")?;
    let cli = Cli::parse();

    // Only show INFO logs for server mode; CLI commands use WARN to stay quiet
    let is_serve = match &cli.command {
        None => true,
        Some(Commands::External(args)) => args.first().map(|s| s.as_str()) == Some("serve"),
        _ => false,
    };
    let level = if is_serve { tracing::Level::INFO } else { tracing::Level::WARN };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .init();

    match cli.command {
        Some(Commands::Call { path, interactive, args }) => {
            if interactive || is_interactive_flag(&args) {
                interactive_call(&config, &path).await?;
            } else {
                call_trait(&config, &path, &args).await?;
            }
        }
        Some(Commands::External(args)) => {
            let name = &args[0];
            let rest: Vec<String> = args[1..].to_vec();
            if name == "mcp" {
                // MCP stdio server — bootstrap registry then run blocking stdio loop
                let _dispatcher = crate::bootstrap(&config)?;
                crate::dispatcher::compiled::mcp::run_stdio();
                return Ok(());
            } else if name.starts_with("__") && name.ends_with("__") {
                call_trait(&config, name, &rest).await?;
            } else if name == "stop" {
                call_trait(&config, "__stop__", &rest).await?;
            } else {
                let sys_path = format!("sys.{}", name);
                let kernel_path = format!("kernel.{}", name);
                let trait_path = if crate::trait_exists(&config, &sys_path) {
                    sys_path
                } else if crate::trait_exists(&config, &kernel_path) {
                    kernel_path
                } else {
                    sys_path
                };
                // Check for -i in the rest args
                if is_interactive_flag(&rest) {
                    interactive_call(&config, &trait_path).await?;
                } else {
                    call_trait(&config, &trait_path, &rest).await?;
                }
            }
        }
        None => {
            let port = std::env::var("TRAITS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(config.traits.port);
            dispatch_trait(&config, "kernel.serve", &[&port.to_string()]).await?;
        }
    }

    Ok(())
}

// ────────────────── dispatch helpers ──────────────────

/// Print a trait result, using a CLI formatter if one exists, else JSON.
/// CLI formatters only activate when stdout is a terminal (TTY).
fn print_result(trait_path: &str, result: &TraitValue) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::IsTerminal;
    let json_val = result.to_json();
    if std::io::stdout().is_terminal() {
        if let Some(formatted) = crate::dispatcher::cli_formatters::format_cli(trait_path, &json_val) {
            print!("{}", formatted);
            return Ok(());
        }
    }
    let json = serde_json::to_string_pretty(&json_val)?;
    println!("{}", json);
    Ok(())
}

/// Parse a single CLI string into a typed JSON value.
fn parse_cli_value(s: &str) -> serde_json::Value {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        return v;
    }
    if let Ok(n) = s.parse::<i64>() {
        return serde_json::Value::from(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return serde_json::Value::from(f);
    }
    match s {
        "true" => serde_json::Value::Bool(true),
        "false" => serde_json::Value::Bool(false),
        "null" => serde_json::Value::Null,
        _ => serde_json::Value::String(s.to_string()),
    }
}

/// Parse raw CLI args into ordered TraitValues using the trait's param signature.
fn parse_cli_args(trait_path: &str, raw_args: &[String]) -> Vec<TraitValue> {
    let is_flag = |a: &str| {
        a.starts_with("--")
            || (a.starts_with('-')
                && a.len() == 2
                && a.as_bytes().get(1).map_or(false, |b| b.is_ascii_alphabetic()))
    };
    let has_flags = raw_args.iter().any(|a| is_flag(a));

    if !has_flags {
        return raw_args.iter().map(|a| TraitValue::from_json(&parse_cli_value(a))).collect();
    }

    // Parse flags into a map
    let mut flag_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut i = 0;
    while i < raw_args.len() {
        if let Some(flag) = raw_args[i].strip_prefix("--") {
            let key = flag.replace('-', "_");
            if i + 1 < raw_args.len() && !is_flag(&raw_args[i + 1]) {
                flag_map.insert(key, raw_args[i + 1].clone());
                i += 2;
            } else {
                flag_map.insert(key, "true".to_string());
                i += 1;
            }
        } else if raw_args[i].starts_with('-')
            && raw_args[i].len() == 2
            && raw_args[i].as_bytes().get(1).map_or(false, |b| b.is_ascii_alphabetic())
        {
            let short = raw_args[i].chars().nth(1).unwrap().to_string();
            if i + 1 < raw_args.len() && !is_flag(&raw_args[i + 1]) {
                flag_map.insert(short, raw_args[i + 1].clone());
                i += 2;
            } else {
                flag_map.insert(short, "true".to_string());
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    // Resolve flags against param signature
    if let Some(reg) = crate::globals::REGISTRY.get() {
        if let Some(entry) = reg.get(trait_path) {
            return entry.signature.params.iter().map(|p| {
                let val = flag_map.get(&p.name)
                    .or_else(|| {
                        p.name.chars().next()
                            .map(|c| c.to_string())
                            .as_ref()
                            .and_then(|s| flag_map.get(s))
                    });
                match val {
                    Some(v) => match format!("{:?}", p.param_type).as_str() {
                        "String" => TraitValue::from_json(&serde_json::Value::String(v.clone())),
                        _ => TraitValue::from_json(&parse_cli_value(v)),
                    },
                    None => TraitValue::Null,
                }
            }).collect();
        }
    }

    // No signature — return flags as an object
    let map: serde_json::Map<String, serde_json::Value> = flag_map.into_iter()
        .map(|(k, v)| (k, parse_cli_value(&v)))
        .collect();
    vec![TraitValue::from_json(&serde_json::Value::Array(vec![serde_json::Value::Object(map)]))]
}

/// Print usage info for a trait by looking up its signature in the registry.
fn print_trait_usage(trait_path: &str) {
    if let Some(reg) = crate::globals::REGISTRY.get() {
        if let Some(entry) = reg.get(trait_path) {
            let params_str = entry.signature.params.iter()
                .map(|p| if p.optional { format!("[<{}>]", p.name) } else { format!("<{}>", p.name) })
                .collect::<Vec<_>>().join(" ");
            eprintln!();
            if let Some(short) = trait_path.strip_prefix("sys.") {
                eprintln!("Usage: traits {} {}", short, params_str);
                eprintln!("   or: traits call {} {}", trait_path, params_str);
            } else {
                eprintln!("Usage: traits call {} {}", trait_path, params_str);
            }
            if !entry.description.is_empty() {
                eprintln!("  {}", entry.description.trim());
            }
            eprintln!();
            if !entry.signature.params.is_empty() {
                eprintln!("Parameters:");
                for p in &entry.signature.params {
                    let req = if p.optional { "optional" } else { "required" };
                    let pipe = if p.pipe { " (accepts stdin)" } else { "" };
                    eprintln!("  {:12} {:?}, {}{} — {}", p.name, p.param_type, req, pipe, p.description);
                }
            }
        }
    }
}

/// Read piped stdin if available (non-TTY), trimming trailing newline.
fn read_stdin_pipe() -> Option<String> {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        return None;
    }
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).ok()?;
    let trimmed = buf.trim_end_matches('\n').trim_end_matches('\r');
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

/// If stdin is piped and a param has pipe=true but wasn't provided, inject it.
fn maybe_inject_stdin(trait_path: &str, args: &mut Vec<String>) {
    if let Some(reg) = crate::globals::REGISTRY.get() {
        if let Some(entry) = reg.get(trait_path) {
            let params = &entry.signature.params;
            // Find the pipe param: explicit pipe=true, or fall back to first param
            let pipe_idx_opt = params.iter().position(|p| p.pipe)
                .or_else(|| if params.is_empty() { None } else { Some(0) });
            if let Some(pipe_idx) = pipe_idx_opt {
                // If that positional arg is missing, try to fill from stdin
                if args.len() <= pipe_idx {
                    if let Some(input) = read_stdin_pipe() {
                        // Pad with empty strings if there are gaps (shouldn't normally happen)
                        while args.len() < pipe_idx {
                            args.push(String::new());
                        }
                        args.push(input);
                    }
                }
            }
        }
    }
}

/// Dispatch any trait by path with string args (used by main.rs for CLI subcommands)
pub async fn dispatch_trait(
    config: &Config,
    trait_path: &str,
    args: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let dispatcher = crate::bootstrap(config)?;
    let mut raw: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    collapse_shell_globs(trait_path, &mut raw);
    maybe_inject_stdin(trait_path, &mut raw);
    let trait_args = parse_cli_args(trait_path, &raw);

    match dispatcher.call(trait_path, trait_args, &crate::dispatcher::CallConfig::default()).await {
        Ok(result) => {
            print_result(trait_path, &result)?;
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("Argument count mismatch") || msg.contains("expected") {
                print_trait_usage(trait_path);
            }
            dispatcher.shutdown().await;
            return Err(format!("Trait call failed: {}", e).into());
        }
    }

    dispatcher.shutdown().await;
    Ok(())
}

/// Detect when the shell expanded a glob (e.g. `*` → list of filenames) and collapse
/// excess args back into a single value so the trait receives what the user intended.
fn collapse_shell_globs(path: &str, args: &mut Vec<String>) {
    let reg = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return,
    };
    let entry = match reg.get(path) {
        Some(e) => e,
        None => return,
    };
    let max_params = entry.signature.params.len();
    // Only trigger if we got way more positional args than the trait accepts,
    // and at least half of the excess look like existing filesystem paths.
    if args.len() <= max_params {
        return;
    }
    let excess = &args[max_params.saturating_sub(1)..]; // args that would map to first param onward
    let paths_count = excess.iter().filter(|a| {
        !a.starts_with('-') && std::path::Path::new(a).exists()
    }).count();
    if paths_count > 1 && paths_count >= excess.len() / 2 {
        eprintln!("hint: it looks like your shell expanded a glob pattern (e.g. * or sys.*).");
        eprintln!("      Quote the pattern to pass it literally:");
        eprintln!("        traits call {} '*'", path);
        eprintln!();
    }
}

/// Call a trait with full CLI arg parsing (positional or --flag style).
pub async fn call_trait(
    config: &Config,
    path: &str,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let dispatcher = crate::bootstrap(config)?;
    let mut args = args.to_vec();
    collapse_shell_globs(path, &mut args);
    maybe_inject_stdin(path, &mut args);
    let trait_args = parse_cli_args(path, &args);

    match dispatcher.call(path, trait_args, &crate::dispatcher::CallConfig::default()).await {
        Ok(result) => {
            print_result(path, &result)?;
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("Argument count mismatch") || msg.contains("expected") {
                print_trait_usage(path);
            }
            dispatcher.shutdown().await;
            return Err(format!("Trait call failed: {}", e).into());
        }
    }

    dispatcher.shutdown().await;
    Ok(())
}

// ── Interactive mode (-i) ──

/// Check if -i or --interactive appears in args (for external subcommand path)
fn is_interactive_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "-i" || a == "--interactive")
}

/// Load per-trait parameter history from .cli_history.json near cli.trait.toml
fn load_history() -> std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>> {
    let path = history_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => std::collections::HashMap::new(),
    }
}

/// Save history back to disk
fn save_history(history: &std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>>) {
    let path = history_path();
    if let Ok(json) = serde_json::to_string_pretty(history) {
        let _ = std::fs::write(&path, json);
    }
}

fn history_path() -> std::path::PathBuf {
    let traits_dir = crate::globals::TRAITS_DIR.get()
        .map(|p| p.as_path())
        .unwrap_or(std::path::Path::new("./traits"));
    traits_dir.join("sys").join("cli").join(".cli_history.json")
}

/// Load examples from a trait's .features.json file
fn load_examples(trait_path: &str) -> Vec<Vec<String>> {
    let parts: Vec<&str> = trait_path.split('.').collect();
    if parts.len() < 2 { return vec![]; }
    let traits_dir = crate::globals::TRAITS_DIR.get()
        .map(|p| p.as_path())
        .unwrap_or(std::path::Path::new("./traits"));

    // Build path: traits/{ns}/{name}/{name}.features.json
    let mut dir = traits_dir.to_path_buf();
    for part in &parts {
        dir.push(part);
    }
    let feat_file = dir.join(format!("{}.features.json", parts.last().unwrap()));
    let content = match std::fs::read_to_string(&feat_file) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let mut examples = vec![];
    if let Some(features) = parsed.get("features").and_then(|v| v.as_array()) {
        for feature in features {
            if let Some(exs) = feature.get("examples").and_then(|v| v.as_array()) {
                for ex in exs {
                    if let Some(input) = ex.get("input").and_then(|v| v.as_array()) {
                        let args: Vec<String> = input.iter().map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        }).collect();
                        examples.push(args);
                    }
                }
            }
        }
    }
    examples
}

// ── Native CliBackend — bridges kernel CliSession to the native dispatcher ──

struct NativeCliBackend {
    dispatcher: crate::dispatcher::Dispatcher,
    rt: tokio::runtime::Handle,
}

impl NativeCliBackend {
    fn new(dispatcher: crate::dispatcher::Dispatcher) -> Self {
        Self {
            rt: tokio::runtime::Handle::current(),
            dispatcher,
        }
    }
}

impl CliBackend for NativeCliBackend {
    fn call(&self, path: &str, args: &[serde_json::Value]) -> Result<serde_json::Value, String> {
        let trait_args: Vec<TraitValue> = args.iter().map(|a| TraitValue::from_json(a)).collect();
        let cc = crate::dispatcher::CallConfig::default();
        match tokio::task::block_in_place(|| {
            self.rt.block_on(self.dispatcher.call(path, trait_args, &cc))
        }) {
            Ok(result) => Ok(result.to_json()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn list_all(&self) -> Vec<serde_json::Value> {
        if let Some(reg) = crate::globals::REGISTRY.get() {
            reg.all().iter().map(|e| serde_json::json!({
                "path": e.path,
                "description": e.description,
                "version": e.version,
                "tags": e.tags,
            })).collect()
        } else {
            vec![]
        }
    }

    fn get_info(&self, path: &str) -> Option<serde_json::Value> {
        let reg = crate::globals::REGISTRY.get()?;
        let entry = reg.get(path)?;
        Some(serde_json::json!({
            "path": entry.path,
            "description": entry.description,
            "version": entry.version,
            "author": entry.author,
            "tags": entry.tags,
            "params": entry.signature.params.iter().map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "type": format!("{:?}", p.param_type).to_lowercase(),
                    "description": p.description,
                    "required": !p.optional,
                })
            }).collect::<Vec<_>>(),
            "returns": format!("{:?}", entry.signature.returns.return_type).to_lowercase(),
            "returns_description": entry.signature.returns.description,
        }))
    }

    fn search(&self, query: &str) -> Vec<serde_json::Value> {
        if let Some(reg) = crate::globals::REGISTRY.get() {
            let q = query.to_lowercase();
            reg.all().iter()
                .filter(|e| {
                    e.path.to_lowercase().contains(&q) ||
                    e.description.to_lowercase().contains(&q) ||
                    e.tags.iter().any(|t| t.to_lowercase().contains(&q))
                })
                .map(|e| serde_json::json!({
                    "path": e.path,
                    "description": e.description,
                }))
                .collect()
        } else {
            vec![]
        }
    }

    fn all_paths(&self) -> Vec<String> {
        if let Some(reg) = crate::globals::REGISTRY.get() {
            reg.all().iter().map(|e| e.path.clone()).collect()
        } else {
            vec![]
        }
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn load_param_history(&self) -> std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>> {
        load_history()
    }

    fn save_param_history(&self, history: &std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>>) {
        save_history(history);
    }

    fn load_examples(&self, path: &str) -> Vec<Vec<String>> {
        load_examples(path)
    }
}

/// Interactive call using the unified kernel CliSession.
/// Puts the terminal in raw mode and feeds crossterm key events as raw bytes
/// into CliSession.feed(), writing the ANSI output directly to stdout.
async fn interactive_call(
    config: &Config,
    trait_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::{terminal, event::{self, Event, KeyCode, KeyModifiers}};
    use std::io::{IsTerminal, Write};

    if !std::io::stdin().is_terminal() {
        return Err("Interactive mode requires a terminal (stdin must be a TTY)".into());
    }

    let dispatcher = crate::bootstrap(config)?;
    let backend = NativeCliBackend::new(dispatcher);

    let mut session = CliSession::new();
    session.load_history(&backend);

    // Pre-seed the command line and trigger interactive mode
    let init_cmd = format!("call -i {}\r", trait_path);
    let init_output = session.feed(&init_cmd, &backend);
    print!("{}", init_output);
    std::io::stdout().flush()?;

    // Enter raw mode and feed crossterm events to CliSession
    terminal::enable_raw_mode()?;
    let result = loop {
        match event::read() {
            Ok(Event::Key(key)) => {
                // Convert crossterm KeyEvent to raw terminal bytes for CliSession.feed()
                let raw = match (key.code, key.modifiers) {
                    (KeyCode::Enter, _) => Some("\r".to_string()),
                    (KeyCode::Tab, _) => Some("\t".to_string()),
                    (KeyCode::Backspace, _) => Some("\x7f".to_string()),
                    (KeyCode::Delete, _) => Some("\x1b[3~".to_string()),
                    (KeyCode::Up, _) => Some("\x1b[A".to_string()),
                    (KeyCode::Down, _) => Some("\x1b[B".to_string()),
                    (KeyCode::Left, _) => Some("\x1b[D".to_string()),
                    (KeyCode::Right, _) => Some("\x1b[C".to_string()),
                    (KeyCode::Home, _) => Some("\x1b[H".to_string()),
                    (KeyCode::End, _) => Some("\x1b[F".to_string()),
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some("\x03".to_string()),
                    (KeyCode::Char('d'), KeyModifiers::CONTROL) => Some("\x04".to_string()),
                    (KeyCode::Char('l'), KeyModifiers::CONTROL) => Some("\x0c".to_string()),
                    (KeyCode::Char('u'), KeyModifiers::CONTROL) => Some("\x15".to_string()),
                    (KeyCode::Char('w'), KeyModifiers::CONTROL) => Some("\x17".to_string()),
                    (KeyCode::Char('a'), KeyModifiers::CONTROL) => Some("\x01".to_string()),
                    (KeyCode::Char('e'), KeyModifiers::CONTROL) => Some("\x05".to_string()),
                    (KeyCode::Char(c), _) => {
                        let mut buf = [0u8; 4];
                        Some(c.encode_utf8(&mut buf).to_string())
                    }
                    _ => None,
                };

                if let Some(bytes) = raw {
                    let output = session.feed(&bytes, &backend);

                    // Handle CLEAR sentinel
                    if output.contains(CLEAR_SENTINEL) {
                        let cleaned = output.replace(CLEAR_SENTINEL, "\x1b[2J\x1b[H");
                        print!("{}", cleaned);
                        std::io::stdout().flush()?;
                        continue;
                    }

                    print!("{}", output);
                    std::io::stdout().flush()?;

                    // If session exited interactive mode (back to prompt),
                    // and we're in a single-command interactive call, we're done.
                    if !session.is_interactive() && output.contains("traits \x1b[0m") {
                        break Ok(());
                    }
                }
            }
            Ok(_) => {} // Ignore mouse/resize events
            Err(e) => break Err(Box::new(e) as Box<dyn std::error::Error>),
        }
    };

    terminal::disable_raw_mode()?;
    println!(); // Final newline after raw mode
    result
}

// ── Trait dispatch entry point ──

/// kernel.cli introspection: returns CLI configuration and capabilities.
pub fn cli(args: &[serde_json::Value]) -> serde_json::Value {
    let _ = args;
    serde_json::json!({
        "features": ["stdin_piping", "arg_parsing", "cli_formatters", "tty_detection", "glob_expansion_check"],
        "dispatch_flow": "CLI args → parse_cli_args → bootstrap → dispatcher.call → print_result",
        "formatter_active": std::io::IsTerminal::is_terminal(&std::io::stdout())
    })
}
