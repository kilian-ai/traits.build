use crate::config::Config;
use crate::types::TraitValue;
use crate::cli::{CliSession, CliCallBackend, CliHistoryBackend, CliExamplesBackend, PROMPT, CLEAR_SENTINEL, REST_SENTINEL_START, REST_SENTINEL_END};
use std::io::Read;

use clap::{Parser, Subcommand};

/// Stderr writer that converts \n to \r\n.
/// Used when the serve REPL is active — raw mode requires \r\n to
/// prevent the staircase effect from server log lines.
struct CrlfStderr;
impl std::io::Write for CrlfStderr {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        let fixed = s.replace('\n', "\r\n");
        std::io::stderr().write_all(fixed.as_bytes())?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::stderr().flush()
    }
}

// ────────────────── CLI arg parsing (clap) ──────────────────

#[derive(Parser)]
#[command(
    name = "traits",
    about = "Trait plugin system",
    after_help = "Any subcommand not listed above is dispatched as sys.<name> (or kernel.<name>).\n\
                  Examples:\n  \
                    traits serve              → sys.serve (default)\n  \
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
    let serve_tty = is_serve && std::io::IsTerminal::is_terminal(&std::io::stdin());
    if serve_tty {
        // REPL enables raw mode — \n must become \r\n to prevent staircase
        tracing_subscriber::fmt()
            .with_max_level(level)
            .with_writer(|| CrlfStderr)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(level)
            .with_writer(std::io::stderr)
            .init();
    }

    if is_serve {
        eprint!("traits {}\r\n", env!("TRAITS_BUILD_VERSION"));
    }

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
            } else if name == "chat" {
                chat_call(&config, &rest).await?;
            } else {
                // Strip @target for path resolution, reattach for dispatch
                let (clean_name, target) = crate::cli::strip_dispatch_target(name);
                let sys_path = format!("sys.{}", clean_name);
                let kernel_path = format!("kernel.{}", clean_name);
                let base_path = if crate::trait_exists(&config, &sys_path) {
                    sys_path
                } else if crate::trait_exists(&config, &kernel_path) {
                    kernel_path
                } else {
                    sys_path
                };
                let trait_path = match target {
                    Some(t) => format!("{}@{}", base_path, t),
                    None => base_path,
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
            dispatch_trait(&config, "sys.serve", &[&port.to_string()]).await?;
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
        if let Some(formatted) = crate::cli::format_trait_result(trait_path, &json_val) {
            print!("{}", formatted);
            return Ok(());
        }
    }
    let json = serde_json::to_string_pretty(&json_val)?;
    println!("{}", json);
    Ok(())
}

/// Parse a single CLI string into a typed JSON value.
/// Delegates to the kernel's canonical parse_value.
fn parse_cli_value(s: &str) -> serde_json::Value {
    crate::cli::parse_value(s)
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
    // Strip @target dispatch hint from path
    let (clean_path, target) = crate::cli::strip_dispatch_target(path);

    // Force remote dispatch: use sys.call HTTP POST instead of local dispatch
    if matches!(target, Some("rest") | Some("relay") | Some("helper")) {
        let _dispatcher = crate::bootstrap(config)?;
        let json_args: Vec<serde_json::Value> = args.iter().map(|a| parse_cli_value(a)).collect();
        dispatch_via_rest(clean_path, &json_args, target.unwrap());
        return Ok(());
    }

    let dispatcher = crate::bootstrap(config)?;
    let mut args = args.to_vec();
    collapse_shell_globs(clean_path, &mut args);
    maybe_inject_stdin(clean_path, &mut args);
    let trait_args = parse_cli_args(clean_path, &args);

    match dispatcher.call(clean_path, trait_args, &crate::dispatcher::CallConfig::default()).await {
        Ok(result) => {
            print_result(clean_path, &result)?;
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("Argument count mismatch") || msg.contains("expected") {
                print_trait_usage(clean_path);
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

// Helper functions (load_history, save_history, history_path, load_examples)
// have moved to sys.cli.native — accessed via dispatch("sys.cli.native", ...)

// ── Native CliBackend — thin dispatch wrapper delegating to sys.cli.native ──

struct NativeCliBackend;

impl NativeCliBackend {
    fn dispatch_method(&self, method: &str, args: &[serde_json::Value]) -> Option<serde_json::Value> {
        let mut full_args = vec![serde_json::Value::String(method.to_string())];
        full_args.extend_from_slice(args);
        // Resolve "native" via kernel.cli: bindings[native] → requires[native] → auto-discover
        let backend = crate::globals::REGISTRY.get()
            .and_then(|reg| reg.resolve_keyed("kernel.cli", "native"))
            .unwrap_or_else(|| "sys.cli.native".to_string());
        crate::dispatcher::compiled::dispatch(&backend, &full_args)
    }
}

impl CliCallBackend for NativeCliBackend {
    fn call(&self, path: &str, args: &[serde_json::Value]) -> Result<serde_json::Value, String> {
        match self.dispatch_method("call", &[serde_json::json!(path), serde_json::Value::Array(args.to_vec())]) {
            Some(v) => {
                if v.get("ok").and_then(|b| b.as_bool()) == Some(true) {
                    Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null))
                } else {
                    Err(v.get("error").and_then(|e| e.as_str()).unwrap_or("unknown error").to_string())
                }
            }
            None => Err("Backend dispatch failed".into()),
        }
    }

    fn list_all(&self) -> Vec<serde_json::Value> {
        self.dispatch_method("list_all", &[])
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    }

    fn get_info(&self, path: &str) -> Option<serde_json::Value> {
        self.dispatch_method("get_info", &[serde_json::json!(path)])
            .filter(|v| !v.is_null())
    }

    fn search(&self, query: &str) -> Vec<serde_json::Value> {
        self.dispatch_method("search", &[serde_json::json!(query)])
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    }

    fn all_paths(&self) -> Vec<String> {
        self.dispatch_method("all_paths", &[])
            .and_then(|v| v.as_array().cloned())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    }

    fn version(&self) -> String {
        self.dispatch_method("version", &[])
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
    }

}

impl CliHistoryBackend for NativeCliBackend {
    fn load_param_history(&self) -> std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>> {
        self.dispatch_method("load_param_history", &[])
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    fn save_param_history(&self, history: &std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>>) {
        if let Ok(val) = serde_json::to_value(history) {
            let _ = self.dispatch_method("save_param_history", &[val]);
        }
    }

}

impl CliExamplesBackend for NativeCliBackend {
    fn load_examples(&self, path: &str) -> Vec<Vec<String>> {
        self.dispatch_method("load_examples", &[serde_json::json!(path)])
            .and_then(|v| v.as_array().cloned())
            .map(|arr| {
                arr.iter().filter_map(|ex| {
                    ex.as_array().map(|a| {
                        a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                    })
                }).collect()
            })
            .unwrap_or_default()
    }
}

// ── Unified raw-mode REPL ──
// Both interactive_call and serve_repl share key mapping, sentinel handling,
// and the event loop. The only differences are setup/teardown and exit conditions.

/// Convert a crossterm KeyEvent to raw terminal bytes for CliSession.feed().
/// Single source of truth for the crossterm → ANSI byte mapping.
fn keycode_to_bytes(key: &crossterm::event::KeyEvent) -> Option<String> {
    use crossterm::event::{KeyCode, KeyModifiers};
    match (key.code, key.modifiers) {
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
    }
}

/// Process CliSession output: handle CLEAR and REST sentinels, print the rest.
/// Returns `true` if a sentinel was handled (caller should `continue` the loop).
fn process_session_output(output: &str, backend: &NativeCliBackend) -> bool {
    use std::io::Write;

    // Handle CLEAR sentinel
    if output.contains(CLEAR_SENTINEL) {
        let cleaned = output.replace(CLEAR_SENTINEL, "\x1b[2J\x1b[H");
        print!("{}", cleaned);
        std::io::stdout().flush().ok();
        return true;
    }

    // Handle REST sentinel — extract {p, a, t, rp} and dispatch via native backend or HTTP
    if output.contains(REST_SENTINEL_START) {
        if let Some(start) = output.find(REST_SENTINEL_START) {
            print!("{}", &output[..start]);
        }
        if let (Some(s), Some(e)) = (output.find(REST_SENTINEL_START), output.find(REST_SENTINEL_END)) {
            let json_str = &output[s + REST_SENTINEL_START.len()..e];
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                let path = parsed["p"].as_str().unwrap_or("");
                let args: Vec<serde_json::Value> = parsed["a"].as_array().cloned().unwrap_or_default();
                let target = parsed["t"].as_str();
                let use_stream = parsed["stream"].as_bool().unwrap_or(false);

                match target {
                    Some("rest") | Some("helper") => {
                        if use_stream {
                            dispatch_via_rest_stream(path, &args, target.unwrap());
                        } else {
                            dispatch_via_rest(path, &args, target.unwrap());
                        }
                    }
                    _ => {
                        if use_stream && path == "llm.prompt.acp" {
                            // Streaming: call ACP directly, print chunks as they arrive
                            // Raw mode: \n alone doesn't carriage-return, so convert to \r\n
                            let mut first = true;
                            let mut full_response = String::new();
                            let result = crate::dispatcher::compiled::acp::acp_proxy_dispatch_streaming(
                                &args,
                                &mut |chunk: &str| {
                                    if first {
                                        // Clear the "thinking…" line
                                        print!("\x1b[A\x1b[2K\r");
                                        first = false;
                                    }
                                    full_response.push_str(chunk);
                                    print!("{}", chunk.replace('\n', "\r\n"));
                                    std::io::stdout().flush().ok();
                                },
                            );
                            if first {
                                // No chunks arrived — fallback: print full result
                                print!("\x1b[A\x1b[2K\r"); // clear "thinking…"
                                if let Some(text) = result.as_str() {
                                    full_response = text.to_string();
                                    print!("{}", text.replace('\n', "\r\n"));
                                } else if let Some(err) = result.get("error").and_then(|e| e.as_str()) {
                                    print!("\x1b[31mError: {}\x1b[0m", err);
                                }
                            }
                            print!("\r\n");
                            std::io::stdout().flush().ok();

                            // Save assistant response to session on disk
                            if let Some(sid) = parsed.get("sid").and_then(|v| v.as_str()) {
                                if !full_response.is_empty() {
                                    crate::dispatcher::compiled::dispatch(
                                        "sys.chat",
                                        &[
                                            serde_json::json!("append"),
                                            serde_json::json!(sid),
                                            serde_json::json!("assistant"),
                                            serde_json::json!(full_response),
                                        ],
                                    );
                                }
                            }
                        } else if path == "sys.voice" {
                            // Voice takes over the terminal (mic/playback, eprintln output).
                            // Disable raw mode so its \n output doesn't staircase,
                            // then re-enable after it returns.
                            print!("\x1b[A\x1b[2K\r");
                            std::io::stdout().flush().ok();
                            let _ = crossterm::terminal::disable_raw_mode();
                            let _ = backend.call(path, &args);
                            let _ = crossterm::terminal::enable_raw_mode();
                        } else {
                            // Default: non-streaming compiled dispatch
                            // Clear the progress line (e.g. "Fetching models…") above
                            print!("\x1b[A\x1b[2K\r");
                            match backend.call(path, &args) {
                                Ok(result) => {
                                    let formatted = crate::cli::format_rest_result(path, &args, &result)
                                        .unwrap_or_else(|| serde_json::to_string_pretty(&result).unwrap_or_default());
                                    // Raw mode: ensure \n → \r\n (normalize first to avoid \r\r\n)
                                    print!("{}", formatted.replace("\r\n", "\n").replace('\n', "\r\n"));
                                }
                                Err(e) => print!("\x1b[31mError: {}\x1b[0m\r\n", e),
                            }
                        }
                    }
                }
                // Print the return prompt (chat mode's custom prompt, or default)
                if let Some(rp) = parsed["rp"].as_str() {
                    print!("{}", rp);
                } else {
                    print!("{}", PROMPT);
                }
            } else {
                print!("{}", PROMPT);
            }
        } else {
            print!("{}", PROMPT);
        }
        std::io::stdout().flush().ok();
        return true;
    }

    false
}

/// Dispatch a trait call via HTTP REST (used for @rest and @helper targets).
fn dispatch_via_rest(path: &str, args: &[serde_json::Value], target: &str) {
    use std::io::Write;

    // Determine the server URL based on target
    let base_url = {
        // @rest and @helper both target localhost (the running server)
        // TRAITS_SERVER env var overrides for remote REST.
        std::env::var("TRAITS_SERVER")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                let port = std::env::var("TRAITS_PORT")
                    .ok()
                    .and_then(|p| p.parse::<u16>().ok())
                    .unwrap_or(8090);
                format!("http://127.0.0.1:{}", port)
            })
    };

    let url = format!("{}/traits/{}", base_url, path.replace('.', "/"));
    let body = serde_json::json!({"args": args});

    // Use sys.call to make the HTTP POST
    let result = crate::dispatcher::compiled::dispatch(
        "sys.call",
        &[
            serde_json::Value::String(url.clone()),
            serde_json::Value::String(body.to_string()),
            serde_json::Value::Null,  // no auth
            serde_json::Value::String("POST".to_string()),
        ],
    );

    match result {
        Some(val) => {
            let ok = val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            if ok {
                // Parse the nested response: sys.call returns {ok, status, body} where body is the REST response
                let body_val = val.get("body").cloned().unwrap_or(serde_json::Value::Null);
                // The REST endpoint returns {"result": ...} — extract the inner result
                let trait_result = body_val.get("result").cloned().unwrap_or(body_val.clone());
                // Check if the REST response itself reports an error
                if let Some(err_msg) = body_val.get("error").and_then(|e| e.as_str()) {
                    print!("\x1b[31m{} error: {}\x1b[0m\r\n", target, err_msg);
                } else {
                    let formatted = crate::cli::format_rest_result(path, args, &trait_result)
                        .unwrap_or_else(|| serde_json::to_string_pretty(&trait_result).unwrap_or_default());
                    print!("{}", formatted);
                }
            } else {
                let status = val.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
                // Check body for server error message
                let body_err = val.get("body")
                    .and_then(|b| b.get("error").and_then(|e| e.as_str()));
                let err = if let Some(msg) = body_err {
                    format!("HTTP {}: {}", status, msg)
                } else if status > 0 {
                    format!("HTTP {}", status)
                } else {
                    val.get("error").and_then(|e| e.as_str())
                        .unwrap_or("connection failed")
                        .to_string()
                };
                print!("\x1b[31m{} error: {}\x1b[0m\r\n", target, err);
            }
        }
        None => {
            print!("\x1b[31m{} error: sys.call dispatch failed\x1b[0m\r\n", target);
        }
    }
    std::io::stdout().flush().ok();
}

/// Dispatch a streaming trait call via HTTP SSE (used for @rest and @helper streaming targets).
fn dispatch_via_rest_stream(path: &str, args: &[serde_json::Value], target: &str) {
    use std::io::{BufRead, BufReader, Write};

    let base_url = {
        std::env::var("TRAITS_SERVER")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                let port = std::env::var("TRAITS_PORT")
                    .ok()
                    .and_then(|p| p.parse::<u16>().ok())
                    .unwrap_or(8090);
                format!("http://127.0.0.1:{}", port)
            })
    };

    let url = format!("{}/traits/{}?stream=1", base_url, path.replace('.', "/"));
    let body = serde_json::json!({"args": args}).to_string();

    // Use a raw HTTP POST via sys.call with stream reading
    // sys.call doesn't support SSE, so we use a subprocess curl for streaming
    let result = std::process::Command::new("curl")
        .args(["-sS", "-N", "--no-buffer"])
        .args(["-X", "POST"])
        .args(["-H", "Content-Type: application/json"])
        .args(["-d", &body])
        .arg(&url)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn();

    match result {
        Ok(mut child) => {
            let stdout = child.stdout.take().unwrap();
            let reader = BufReader::new(stdout);
            let mut first = true;
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.starts_with("data: ") {
                        let data = &line[6..];
                        if data == "[DONE]" { break; }
                        // Parse the SSE data — it's a JSON string (the chunk text)
                        let text: String = serde_json::from_str(data)
                            .unwrap_or_else(|_| data.to_string());
                        if first {
                            print!("\x1b[A\x1b[2K\r"); // Clear "thinking…" line
                            first = false;
                        }
                        // Raw mode: \n alone doesn't carriage-return
                        print!("{}", text.replace('\n', "\r\n"));
                        std::io::stdout().flush().ok();
                    }
                }
            }
            if !first { print!("\r\n"); }
            let _ = child.wait();
        }
        Err(e) => {
            print!("\x1b[31m{} stream error: {}\x1b[0m\r\n", target, e);
        }
    }
    std::io::stdout().flush().ok();
}

/// Interactive call using the unified kernel CliSession.
/// Puts the terminal in raw mode and feeds crossterm key events as raw bytes
/// into CliSession.feed(), writing the ANSI output directly to stdout.
async fn interactive_call(
    config: &Config,
    trait_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::{terminal, event::{self, Event}};
    use std::io::{IsTerminal, Write};

    if !std::io::stdin().is_terminal() {
        return Err("Interactive mode requires a terminal (stdin must be a TTY)".into());
    }

    let _dispatcher = crate::bootstrap(config)?;
    let backend = NativeCliBackend;

    let mut session = CliSession::new();
    session.load_history(&backend);

    // Pre-seed the command line and trigger interactive mode
    let init_cmd = format!("call -i {}\r", trait_path);
    let init_output = session.feed(&init_cmd, &backend);
    print!("{}", init_output);
    std::io::stdout().flush()?;

    terminal::enable_raw_mode()?;
    let result = loop {
        match event::read() {
            Ok(Event::Key(key)) => {
                if let Some(bytes) = keycode_to_bytes(&key) {
                    let output = session.feed(&bytes, &backend);
                    if process_session_output(&output, &backend) {
                        continue;
                    }
                    print!("{}", output);
                    std::io::stdout().flush()?;
                    if !session.is_interactive() && output.contains("traits \x1b[0m") {
                        break Ok(());
                    }
                }
            }
            Ok(_) => {}
            Err(e) => break Err(Box::new(e) as Box<dyn std::error::Error>),
        }
    };

    terminal::disable_raw_mode()?;
    println!();
    result
}

/// Standalone `traits chat [agent] [model]` entry point.
/// Bootstraps the registry, then runs a raw-mode REPL starting in chat mode.
async fn chat_call(
    config: &Config,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::{terminal, event::{self, Event, KeyCode, KeyModifiers}};
    use std::io::{IsTerminal, Write};

    if !std::io::stdin().is_terminal() {
        return Err("Chat mode requires a terminal (stdin must be a TTY)".into());
    }

    let _dispatcher = crate::bootstrap(config)?;
    let backend = NativeCliBackend;

    // Parse args: filter out "voice" flag, rest are [agent, model]
    let voice_mode = args.iter().any(|a| a.eq_ignore_ascii_case("voice") || a == "--voice" || a == "-v");
    let positional: Vec<&str> = args.iter()
        .filter(|a| !a.eq_ignore_ascii_case("voice") && *a != "--voice" && *a != "-v")
        .map(|s| s.as_str())
        .collect();

    let agent = positional.first().copied().unwrap_or("opencode");
    let model = positional.get(1).copied().unwrap_or("");

    // ── Voice mode: delegate to sys.voice (OpenAI Realtime API) ──
    if voice_mode {
        // Look up current session for context
        let (session_id, session_agent) = crate::dispatcher::compiled::dispatch(
            "sys.chat",
            &[serde_json::json!("current")],
        ).and_then(|r| {
            if r.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                let sid = r.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                let sa = r.get("agent").and_then(|v| v.as_str()).map(|s| s.to_string());
                Some((sid, sa))
            } else {
                None
            }
        }).unwrap_or((None, None));

        let voice_agent = if !agent.is_empty() { agent.to_string() } else { session_agent.unwrap_or_default() };
        let voice_session = session_id.unwrap_or_default();

        let result = crate::dispatcher::compiled::dispatch(
            "sys.voice",
            &[
                serde_json::json!("cedar"),
                serde_json::json!("gpt-realtime-mini-2025-12-15"),
                serde_json::json!(voice_agent),
                serde_json::json!(voice_session),
            ],
        );
        if let Some(r) = result {
            if r.get("ok").and_then(|v| v.as_bool()) != Some(true) {
                let err = r.get("error").and_then(|e| e.as_str()).unwrap_or("voice mode failed");
                return Err(err.into());
            }
        }
        return Ok(());
    }

    // ── Text mode: normal keyboard-driven chat ──
    let mut session = CliSession::new();
    session.load_history(&backend);

    // Check for last session to resume
    let resume_id: Option<String> = crate::dispatcher::compiled::dispatch(
        "sys.chat",
        &[serde_json::json!("current")],
    ).and_then(|r| {
        if r.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            r.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string())
        } else {
            None
        }
    });

    // Enter chat mode — resume last session or create new
    let banner = session.start_chat(agent, model, resume_id.as_deref());
    print!("{}", banner);
    std::io::stdout().flush()?;

    terminal::enable_raw_mode()?;
    loop {
        match event::read() {
            Ok(Event::Key(key)) => {
                // Ctrl+D: exit
                if key.code == KeyCode::Char('d') && key.modifiers == KeyModifiers::CONTROL {
                    let _ = terminal::disable_raw_mode();
                    println!();
                    return Ok(());
                }

                if let Some(bytes) = keycode_to_bytes(&key) {
                    let output = session.feed(&bytes, &backend);
                    if process_session_output(&output, &backend) {
                        continue;
                    }
                    print!("{}", output);
                    std::io::stdout().flush()?;

                    // Exit when chat mode is left (user typed /quit or Ctrl+C)
                    if !session.is_chat() {
                        let _ = terminal::disable_raw_mode();
                        println!();
                        return Ok(());
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                let _ = terminal::disable_raw_mode();
                return Err(format!("Input error: {}", e).into());
            }
        }
    }
}

/// REPL that runs alongside `traits serve`.
/// Same CliSession as the WASM terminal — green prompt, tab completion, history.
pub fn serve_repl() {
    use crossterm::{terminal, event::{self, Event, KeyCode, KeyModifiers}};
    use std::io::Write;

    let backend = NativeCliBackend;
    let mut session = CliSession::new();
    session.load_history(&backend);

    if std::env::var("TRAITS_REPL_LINE_MODE").ok().as_deref() == Some("1") {
        serve_repl_line_mode(&mut session, &backend);
        return;
    }

    let welcome = session.welcome(&backend);
    print!("{}", welcome);
    std::io::stdout().flush().ok();

    if terminal::enable_raw_mode().is_err() {
        eprintln!("Failed to enable raw mode — switching to line-mode REPL");
        serve_repl_line_mode(&mut session, &backend);
        return;
    }

    loop {
        let event = match event::read() {
            Ok(ev) => ev,
            Err(e) => {
                let _ = terminal::disable_raw_mode();
                eprintln!("REPL raw input error ({}) — switching to line-mode REPL", e);
                serve_repl_line_mode(&mut session, &backend);
                return;
            }
        };

        match event {
            Event::Key(key) => {
                // Ctrl+D: exit REPL and server
                if key.code == KeyCode::Char('d') && key.modifiers == KeyModifiers::CONTROL {
                    let _ = terminal::disable_raw_mode();
                    println!();
                    std::process::exit(0);
                }

                if let Some(bytes) = keycode_to_bytes(&key) {
                    let output = session.feed(&bytes, &backend);
                    if process_session_output(&output, &backend) {
                        continue;
                    }
                    print!("{}", output);
                    std::io::stdout().flush().ok();
                }
            }
            _ => {}
        }
    }
}

fn serve_repl_line_mode(session: &mut CliSession, backend: &NativeCliBackend) {
    use std::fs::OpenOptions;
    use std::io::{BufRead, BufReader, Write};

    let tty = OpenOptions::new().read(true).write(true).open("/dev/tty");

    let (mut out, mut reader): (Box<dyn Write>, Box<dyn BufRead>) = match tty {
        Ok(file) => {
            let in_file = file.try_clone();
            match in_file {
                Ok(in_file) => (Box::new(file), Box::new(BufReader::new(in_file))),
                Err(_) => (Box::new(std::io::stdout()), Box::new(BufReader::new(std::io::stdin()))),
            }
        }
        Err(_) => (Box::new(std::io::stdout()), Box::new(BufReader::new(std::io::stdin()))),
    };

    let _ = writeln!(out, "\n[repl] line mode active (reduced editing features)");
    let _ = out.flush();

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let line = line.trim_end_matches(['\r', '\n']);
                let input = format!("{}\r", line);
                let output = session.feed(&input, backend);
                if process_session_output(&output, backend) {
                    continue;
                }
                let _ = write!(out, "{}", output);
                let _ = out.flush();
            }
            Err(_) => break,
        }
    }

    let _ = writeln!(out);
    let _ = out.flush();
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
