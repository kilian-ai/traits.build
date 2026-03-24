use serde_json::Value;

// ═══════════════════════════════════════════
// ── Portable CLI core ──
// Pure functions for command parsing, dispatch, and ANSI formatting.
// No std::io, no std::fs, no clap, no crossterm.
// Compiled into both native kernel and WASM module.
// ═══════════════════════════════════════════

// ── ANSI color codes ──

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";
pub const CYAN: &str = "\x1b[36m";
pub const GRAY: &str = "\x1b[90m";
pub const BRIGHT_WHITE: &str = "\x1b[97m";

// ── Public dispatch trait ──

/// Backend that provides trait registry and dispatch.
/// Implemented differently in WASM vs native.
pub trait CliBackend {
    fn call(&self, path: &str, args: &[Value]) -> Result<Value, String>;
    fn list_all(&self) -> Vec<Value>;
    fn get_info(&self, path: &str) -> Option<Value>;
    fn search(&self, query: &str) -> Vec<Value>;
    fn all_paths(&self) -> Vec<String>;
    fn version(&self) -> String;
}

// ── Command execution ──

/// Process a single command line. Returns ANSI-formatted output.
pub fn exec_line(line: &str, backend: &dyn CliBackend) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let parts = parse_command(trimmed);
    if parts.is_empty() {
        return String::new();
    }

    let cmd = parts[0].to_lowercase();
    let args = &parts[1..];

    match cmd.as_str() {
        "help" | "h" | "?" => format_help(),
        "list" | "ls" => format_list(backend, args.first().map(|s| s.as_str())),
        "info" | "i" => {
            if args.is_empty() {
                return format!("{RED}Usage: info <trait_path>{RESET}");
            }
            format_info(backend, &args[0])
        }
        "call" | "c" => {
            if args.is_empty() {
                return format!("{RED}Usage: call <trait_path> [args...]{RESET}");
            }
            exec_call(backend, &args[0], &args[1..])
        }
        "search" | "s" => {
            let q = args.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
            if q.is_empty() {
                return format!("{RED}Usage: search <query>{RESET}");
            }
            format_search(backend, &q)
        }
        "version" | "v" => format!("{CYAN}traits.build{RESET} {}", backend.version()),
        "clear" | "cls" => "\x1b[CLEAR]".to_string(), // JS intercepts this
        _ => {
            // Try as trait path shorthand
            let all = backend.all_paths();
            if all.iter().any(|p| p == &cmd) || all.iter().any(|p| p == parts[0].as_str()) {
                exec_call(backend, &parts[0], args)
            } else {
                // Try sys.{cmd} or kernel.{cmd}
                let sys_path = format!("sys.{}", cmd);
                let kernel_path = format!("kernel.{}", cmd);
                if all.iter().any(|p| p == &sys_path) {
                    exec_call(backend, &sys_path, args)
                } else if all.iter().any(|p| p == &kernel_path) {
                    exec_call(backend, &kernel_path, args)
                } else {
                    format!("{RED}Unknown command: {}{RESET}. Type {BLUE}help{RESET} for usage.", cmd)
                }
            }
        }
    }
}

/// Execute a trait call and format the result.
fn exec_call(backend: &dyn CliBackend, path: &str, arg_strs: &[String]) -> String {
    let args: Vec<Value> = arg_strs.iter().map(|s| parse_value(s)).collect();

    match backend.call(path, &args) {
        Ok(result) => {
            let formatted = match &result {
                Value::String(s) => s.clone(),
                other => serde_json::to_string_pretty(other).unwrap_or_default(),
            };
            let lines: Vec<&str> = formatted.lines().collect();
            let mut out = String::new();
            if lines.len() > 100 {
                for line in &lines[..80] {
                    out.push_str(line);
                    out.push_str("\r\n");
                }
                out.push_str(&format!("{GRAY}... ({} more lines){RESET}\r\n", lines.len() - 80));
            } else {
                for line in &lines {
                    out.push_str(line);
                    out.push_str("\r\n");
                }
            }
            out
        }
        Err(e) => format!("{RED}Error: {}{RESET}\r\n", e),
    }
}

// ── Formatters ──

fn format_help() -> String {
    let mut s = String::new();
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Commands{RESET}\r\n"));
    s.push_str(&format!("  {GREEN}list{RESET} {GRAY}[namespace]{RESET}         List traits\r\n"));
    s.push_str(&format!("  {GREEN}info{RESET} {GRAY}<path>{RESET}              Show trait details\r\n"));
    s.push_str(&format!("  {GREEN}call{RESET} {GRAY}<path> [args...]{RESET}    Call a trait\r\n"));
    s.push_str(&format!("  {GREEN}search{RESET} {GRAY}<query>{RESET}           Search by name or description\r\n"));
    s.push_str(&format!("  {GRAY}<path> [args...]{RESET}           Shorthand — call trait directly\r\n"));
    s.push_str(&format!("  {GREEN}version{RESET}                    Show kernel version\r\n"));
    s.push_str(&format!("  {GREEN}clear{RESET}                      Clear terminal\r\n"));
    s.push_str(&format!("  {GREEN}help{RESET}                       Show this help\r\n"));
    s.push_str("\r\n");
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Shortcuts{RESET}\r\n"));
    s.push_str(&format!("  {CYAN}Tab{RESET}          Auto-complete trait paths\r\n"));
    s.push_str(&format!("  {CYAN}↑ / ↓{RESET}        Navigate command history\r\n"));
    s.push_str(&format!("  {CYAN}Ctrl+L{RESET}       Clear terminal\r\n"));
    s.push_str(&format!("  {CYAN}Ctrl+C{RESET}       Cancel current line\r\n"));
    s.push_str(&format!("  {CYAN}Ctrl+U{RESET}       Clear entire line\r\n"));
    s.push_str(&format!("  {CYAN}Ctrl+W{RESET}       Delete word backward\r\n"));
    s.push_str(&format!("  {CYAN}Ctrl+A/E{RESET}     Jump to start/end of line\r\n"));
    s.push_str("\r\n");
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Examples{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}call sys.checksum hash \"hello world\"{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}sys.version{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}info sys.list{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}list sys{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}search checksum{RESET}\r\n"));
    s
}

fn format_list(backend: &dyn CliBackend, namespace: Option<&str>) -> String {
    let all = backend.list_all();
    let filtered: Vec<&Value> = if let Some(ns) = namespace {
        all.iter().filter(|t| {
            t.get("path").and_then(|p| p.as_str()).map_or(false, |p| p.starts_with(ns))
        }).collect()
    } else {
        all.iter().collect()
    };

    if filtered.is_empty() {
        return if let Some(ns) = namespace {
            format!("{YELLOW}No traits in namespace \"{}\"{RESET}", ns)
        } else {
            format!("{YELLOW}No traits registered{RESET}")
        };
    }

    // Group by namespace
    let mut groups: std::collections::BTreeMap<String, Vec<&Value>> = std::collections::BTreeMap::new();
    for t in &filtered {
        let path = t.get("path").and_then(|p| p.as_str()).unwrap_or("");
        let parts: Vec<&str> = path.rsplitn(2, '.').collect();
        let ns = if parts.len() > 1 { parts[1] } else { "" };
        groups.entry(ns.to_string()).or_default().push(t);
    }

    let mut out = String::new();
    for (ns, traits) in &groups {
        out.push_str(&format!("{BOLD}{BRIGHT_WHITE}{}{RESET} {GRAY}({}){RESET}\r\n", ns, traits.len()));
        for t in traits {
            let path = t.get("path").and_then(|p| p.as_str()).unwrap_or("");
            let name = path.rsplit('.').next().unwrap_or(path);
            let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
            let wasm = t.get("wasm_callable").and_then(|w| w.as_bool()).unwrap_or(false);
            let badge = if wasm {
                format!("{GREEN}[WASM]{RESET}")
            } else {
                format!("{YELLOW}[REST]{RESET}")
            };
            out.push_str(&format!("  {} {BLUE}{}{RESET}  {GRAY}{}{RESET}\r\n", badge, name, desc));
        }
    }
    out.push_str(&format!("{GRAY}{} traits{RESET}", filtered.len()));
    out
}

fn format_info(backend: &dyn CliBackend, path: &str) -> String {
    let info = match backend.get_info(path) {
        Some(v) => v,
        None => return format!("{RED}Trait \"{}\" not found{RESET}", path),
    };

    let mut out = String::new();
    let trait_path = info.get("path").and_then(|p| p.as_str()).unwrap_or(path);
    let version = info.get("version").and_then(|v| v.as_str()).unwrap_or("");
    let desc = info.get("description").and_then(|d| d.as_str()).unwrap_or("");
    let wasm = info.get("wasm_callable").and_then(|w| w.as_bool()).unwrap_or(false);
    let badge = if wasm {
        format!("{GREEN}WASM{RESET}")
    } else {
        format!("{YELLOW}REST{RESET}")
    };

    out.push_str(&format!("{BOLD}{BRIGHT_WHITE}{}{RESET}  {}  {GRAY}{}{RESET}\r\n", trait_path, badge, version));
    if !desc.is_empty() {
        out.push_str(&format!("  {GRAY}{}{RESET}\r\n", desc));
    }

    if let Some(params) = info.get("params").and_then(|p| p.as_array()) {
        if !params.is_empty() {
            out.push_str("\r\n");
            out.push_str(&format!("{BOLD}Parameters:{RESET}\r\n"));
            for p in params {
                let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                let ptype = p.get("type").and_then(|t| t.as_str()).unwrap_or("any");
                let pdesc = p.get("description").and_then(|d| d.as_str()).unwrap_or("");
                let req = p.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
                let req_mark = if req { format!(" {RED}*{RESET}") } else { String::new() };
                out.push_str(&format!("  {BLUE}{}{RESET} {MAGENTA}({}){RESET}{}  {GRAY}{}{RESET}\r\n",
                    name, ptype, req_mark, pdesc));
            }
        }
    }

    if let Some(ret) = info.get("returns").or_else(|| info.get("returns_type")) {
        let rtype = if let Some(s) = ret.as_str() { s } else { "any" };
        let rdesc = info.get("returns_description").and_then(|d| d.as_str()).unwrap_or("");
        out.push_str("\r\n");
        out.push_str(&format!("{BOLD}Returns:{RESET} {MAGENTA}{}{RESET}  {GRAY}{}{RESET}", rtype, rdesc));
    }

    out
}

fn format_search(backend: &dyn CliBackend, query: &str) -> String {
    let results = backend.search(query);
    if results.is_empty() {
        return format!("{YELLOW}No matches for \"{}\"{RESET}", query);
    }
    let mut out = String::new();
    for t in &results {
        let path = t.get("path").and_then(|p| p.as_str()).unwrap_or("");
        let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
        let wasm = t.get("wasm_callable").and_then(|w| w.as_bool()).unwrap_or(false);
        let badge = if wasm {
            format!("{GREEN}[WASM]{RESET}")
        } else {
            format!("{YELLOW}[REST]{RESET}")
        };
        out.push_str(&format!("{} {BLUE}{}{RESET}  {GRAY}{}{RESET}\r\n", badge, path, desc));
    }
    out.push_str(&format!("{GRAY}{} matches{RESET}", results.len()));
    out
}

// ── Parsing ──

/// Parse a command line string into parts, respecting quoted strings.
pub fn parse_command(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;

    for ch in line.chars() {
        if ch == '"' {
            in_quote = !in_quote;
        } else if ch == ' ' && !in_quote {
            if !current.is_empty() {
                parts.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Parse a CLI string value into a JSON Value.
pub fn parse_value(s: &str) -> Value {
    // Try JSON first
    if let Ok(v) = serde_json::from_str::<Value>(s) {
        return v;
    }
    // Try numeric
    if let Ok(n) = s.parse::<i64>() {
        return Value::from(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::from(f);
    }
    // Booleans and null
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => Value::String(s.to_string()),
    }
}

// ── Tab completion ──

/// Get completions for a prefix. Returns (matches, common_prefix).
pub fn tab_completions(prefix: &str, all_paths: &[String]) -> (Vec<String>, String) {
    let matches: Vec<String> = all_paths.iter()
        .filter(|p| p.starts_with(prefix))
        .cloned()
        .collect();

    if matches.is_empty() {
        return (matches, String::new());
    }

    // Find common prefix among matches
    let mut common = matches[0].clone();
    for m in &matches[1..] {
        while !m.starts_with(&common) {
            common.pop();
        }
    }

    (matches, common)
}

/// Get interactive mode parameter info for a trait.
/// Returns a JSON array of param objects, or None if trait not found.
pub fn interactive_params(path: &str, backend: &dyn CliBackend) -> Option<Value> {
    backend.get_info(path).and_then(|info| {
        info.get("params").cloned()
    })
}

// ── Native dispatch entry point ──
// In native, this is wired as `kernel.cli` trait dispatch.
// In WASM, the lib.rs wraps exec_line with the WASM backend.
pub fn cli_dispatch(_args: &[Value]) -> Value {
    // This is a no-op in the compiled dispatch table.
    // Real execution goes through exec_line() with a backend.
    Value::String("kernel.cli: use exec_line() with a CliBackend".to_string())
}
