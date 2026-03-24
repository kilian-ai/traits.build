use crate::config::Config;
use crate::types::TraitValue;
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
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();

    let config = Config::load("traits.toml")?;
    let cli = Cli::parse();

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

/// Interactive prompt for a single parameter with history and arrow key navigation.
/// Returns the user's input string, or None if they pressed Ctrl-C/Ctrl-D.
fn prompt_param(
    param_name: &str,
    param_type: &str,
    description: &str,
    required: bool,
    default_val: &str,
    history: &[String],
) -> Option<String> {
    use crossterm::{terminal, event::{self, Event, KeyCode, KeyModifiers}, cursor, execute};
    use std::io::Write;

    let mut stdout = std::io::stdout();
    let req_badge = if required { "\x1b[91m*\x1b[0m" } else { " " };
    let type_dim = format!("\x1b[90m{}\x1b[0m", param_type);

    // Print param header
    eprint!("  {} \x1b[1m{}\x1b[0m  {}  \x1b[90m{}\x1b[0m", req_badge, param_name, type_dim, description);
    if !default_val.is_empty() {
        eprint!("  \x1b[90m[{}]\x1b[0m", default_val);
    }
    eprintln!();

    // Build completion list: history (most recent first) + default
    let mut completions: Vec<String> = history.iter().rev().cloned().collect();
    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    completions.retain(|v| seen.insert(v.clone()));
    if !default_val.is_empty() && !seen.contains(default_val) {
        completions.push(default_val.to_string());
    }

    // Prompt line
    eprint!("  \x1b[96m❯\x1b[0m ");
    let _ = stdout.flush();

    // Enter raw mode for key-by-key reading
    let _ = terminal::enable_raw_mode();
    let mut input = String::new();
    let mut hist_idx: Option<usize> = None; // None = typing new, Some(i) = browsing completions[i]
    let mut cursor_pos: usize = 0;

    loop {
        if let Ok(Event::Key(key)) = event::read() {
            match (key.code, key.modifiers) {
                (KeyCode::Enter, _) => {
                    let _ = terminal::disable_raw_mode();
                    eprintln!();
                    let result = input.trim().to_string();
                    if result.is_empty() && !default_val.is_empty() {
                        return Some(default_val.to_string());
                    }
                    if result.is_empty() && !required {
                        return Some(String::new());
                    }
                    if result.is_empty() && required {
                        eprint!("  \x1b[91m  required — try again\x1b[0m\n");
                        return prompt_param(param_name, param_type, description, required, default_val, history);
                    }
                    return Some(result);
                }
                (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                    let _ = terminal::disable_raw_mode();
                    eprintln!();
                    return None; // Abort
                }
                (KeyCode::Up, _) => {
                    if completions.is_empty() { continue; }
                    hist_idx = Some(match hist_idx {
                        None => 0,
                        Some(i) => (i + 1).min(completions.len() - 1),
                    });
                    input = completions[hist_idx.unwrap()].clone();
                    cursor_pos = input.len();
                    redraw_input(&mut stdout, &input, cursor_pos);
                }
                (KeyCode::Down, _) => {
                    if completions.is_empty() { continue; }
                    match hist_idx {
                        Some(0) => {
                            hist_idx = None;
                            input.clear();
                            cursor_pos = 0;
                            redraw_input(&mut stdout, &input, cursor_pos);
                        }
                        Some(i) => {
                            hist_idx = Some(i - 1);
                            input = completions[hist_idx.unwrap()].clone();
                            cursor_pos = input.len();
                            redraw_input(&mut stdout, &input, cursor_pos);
                        }
                        None => {}
                    }
                }
                (KeyCode::Left, _) => {
                    if cursor_pos > 0 {
                        cursor_pos -= 1;
                        let _ = execute!(stdout, cursor::MoveLeft(1));
                    }
                }
                (KeyCode::Right, _) => {
                    if cursor_pos < input.len() {
                        cursor_pos += 1;
                        let _ = execute!(stdout, cursor::MoveRight(1));
                    }
                }
                (KeyCode::Backspace, _) => {
                    if cursor_pos > 0 {
                        cursor_pos -= 1;
                        input.remove(cursor_pos);
                        hist_idx = None;
                        redraw_input(&mut stdout, &input, cursor_pos);
                    }
                }
                (KeyCode::Delete, _) => {
                    if cursor_pos < input.len() {
                        input.remove(cursor_pos);
                        hist_idx = None;
                        redraw_input(&mut stdout, &input, cursor_pos);
                    }
                }
                (KeyCode::Home, _) => {
                    cursor_pos = 0;
                    redraw_input(&mut stdout, &input, cursor_pos);
                }
                (KeyCode::End, _) => {
                    cursor_pos = input.len();
                    redraw_input(&mut stdout, &input, cursor_pos);
                }
                (KeyCode::Tab, _) => {
                    // Tab cycles through completions like arrow up
                    if completions.is_empty() { continue; }
                    hist_idx = Some(match hist_idx {
                        None => 0,
                        Some(i) => (i + 1) % completions.len(),
                    });
                    input = completions[hist_idx.unwrap()].clone();
                    cursor_pos = input.len();
                    redraw_input(&mut stdout, &input, cursor_pos);
                }
                (KeyCode::Char(c), _) => {
                    input.insert(cursor_pos, c);
                    cursor_pos += 1;
                    hist_idx = None;
                    redraw_input(&mut stdout, &input, cursor_pos);
                }
                _ => {}
            }
        }
    }
}

/// Redraw the input line (erase current line, rewrite)
fn redraw_input(stdout: &mut std::io::Stdout, input: &str, cursor_pos: usize) {
    use std::io::Write;
    // Move to column 0, clear line, print prompt + input, position cursor
    let _ = write!(stdout, "\r\x1b[2K  \x1b[96m❯\x1b[0m {}", input);
    // Move cursor to correct position: prompt is "  ❯ " = 4 visible chars
    let total = 4 + input.len();
    let target = 4 + cursor_pos;
    if target < total {
        let back = total - target;
        let _ = write!(stdout, "\x1b[{}D", back);
    }
    let _ = stdout.flush();
}

/// Interactive call: prompt for each parameter, then dispatch
async fn interactive_call(
    config: &Config,
    trait_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return Err("Interactive mode requires a terminal (stdin must be a TTY)".into());
    }

    let dispatcher = crate::bootstrap(config)?;

    let reg = crate::globals::REGISTRY.get()
        .ok_or("Registry not initialized")?;
    let entry = reg.get(trait_path)
        .ok_or_else(|| format!("Trait '{}' not found", trait_path))?;

    // Header
    eprintln!();
    eprintln!("  \x1b[1m{}\x1b[0m  \x1b[90m{}\x1b[0m", trait_path, entry.description);
    eprintln!("  \x1b[90m{}\x1b[0m", "─".repeat(50));

    if entry.signature.params.is_empty() {
        eprintln!("  \x1b[90m(no parameters)\x1b[0m");
        eprintln!();
    }

    // Load history and examples
    let mut all_history = load_history();
    let trait_history = all_history.entry(trait_path.to_string()).or_default();
    let examples = load_examples(trait_path);

    let mut collected_args: Vec<String> = Vec::new();

    for (i, param) in entry.signature.params.iter().enumerate() {
        let param_type_str = format!("{:?}", param.param_type).to_lowercase();
        let required = !param.optional;

        // Build default from examples
        let default_val = examples.iter()
            .filter_map(|ex| ex.get(i).cloned())
            .next()
            .unwrap_or_default();

        // Get per-param history
        let param_hist = trait_history.entry(param.name.clone()).or_insert_with(Vec::new);

        match prompt_param(
            &param.name,
            &param_type_str,
            &param.description,
            required,
            &default_val,
            param_hist,
        ) {
            Some(val) => {
                // Save to history (deduplicate, keep last 20)
                if !val.is_empty() {
                    param_hist.retain(|v| v != &val);
                    param_hist.push(val.clone());
                    if param_hist.len() > 20 {
                        param_hist.remove(0);
                    }
                }
                collected_args.push(val);
            }
            None => {
                eprintln!("  \x1b[90maborted\x1b[0m");
                dispatcher.shutdown().await;
                return Ok(());
            }
        }
    }

    // Save history
    save_history(&all_history);

    eprintln!("  \x1b[90m{}\x1b[0m", "─".repeat(50));

    // Parse and dispatch
    let trait_args = parse_cli_args(trait_path, &collected_args);
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
