use serde_json::Value;
use std::collections::HashMap;

// ═══════════════════════════════════════════
// ── Portable CLI core ──
// Stateful session with line editing, command history,
// tab completion, and interactive parameter prompting.
// Compiled into both native kernel and WASM module.
// No std::io, no std::fs, no clap, no crossterm.
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

const PROMPT: &str = "\x1b[32mtraits \x1b[0m";
const IPROMPT: &str = "  \x1b[96m❯\x1b[0m ";

/// Sentinel returned by clear — frontends intercept this.
pub const CLEAR_SENTINEL: &str = "\x1b[CLEAR]";

// ── Key events ──

pub enum KeyEvent {
    Char(char),
    Enter,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Backspace,
    Delete,
    Home,
    End,
    CtrlC,
    CtrlD,
    CtrlL,
    CtrlU,
    CtrlW,
    CtrlA,
    CtrlE,
}

// ── Backend trait ──

/// Backend that provides trait registry and dispatch.
/// Implemented differently in WASM vs native.
pub trait CliBackend {
    fn call(&self, path: &str, args: &[Value]) -> Result<Value, String>;
    fn list_all(&self) -> Vec<Value>;
    fn get_info(&self, path: &str) -> Option<Value>;
    fn search(&self, query: &str) -> Vec<Value>;
    fn all_paths(&self) -> Vec<String>;
    fn version(&self) -> String;
    fn load_param_history(&self) -> HashMap<String, HashMap<String, Vec<String>>> {
        HashMap::new()
    }
    fn save_param_history(&self, _history: &HashMap<String, HashMap<String, Vec<String>>>) {}
    fn load_examples(&self, _path: &str) -> Vec<Vec<String>> {
        vec![]
    }
}

// ── Interactive mode state ──

struct ParamMeta {
    name: String,
    ptype: String,
    description: String,
    required: bool,
    default_val: String,
}

struct InteractiveState {
    path: String,
    params: Vec<ParamMeta>,
    values: Vec<String>,
    idx: usize,
    completions: Vec<String>,
    comp_idx: Option<usize>,
}

// ── CLI Session ──

pub struct CliSession {
    line_buffer: String,
    cursor_pos: usize,
    history: Vec<String>,
    hist_idx: isize,
    interactive: Option<InteractiveState>,
    param_history: HashMap<String, HashMap<String, Vec<String>>>,
}

impl CliSession {
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            hist_idx: -1,
            interactive: None,
            param_history: HashMap::new(),
        }
    }

    /// Load persisted param history from backend (call after new).
    pub fn load_history(&mut self, backend: &dyn CliBackend) {
        self.param_history = backend.load_param_history();
    }

    pub fn is_interactive(&self) -> bool {
        self.interactive.is_some()
    }

    /// Return the welcome banner + initial prompt.
    pub fn welcome(&self, backend: &dyn CliBackend) -> String {
        let all = backend.list_all();
        let wasm_count = all
            .iter()
            .filter(|t| {
                t.get("wasm_callable")
                    .and_then(|w| w.as_bool())
                    .unwrap_or(false)
            })
            .count();
        format!(
            "{BLUE}{BOLD}traits.build{RESET} terminal\r\n\
             {GRAY}{} traits loaded ({} WASM). Type \"help\" for commands.{RESET}\r\n\r\n\
             {PROMPT}",
            all.len(),
            wasm_count,
        )
    }

    // ── Raw input feed (for xterm.js / terminal byte streams) ──

    /// Parse raw terminal input bytes into key events and process them.
    /// Returns ANSI text to write to the terminal.
    pub fn feed(&mut self, data: &str, backend: &dyn CliBackend) -> String {
        let mut output = String::new();
        let bytes = data.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == 0x1b {
                // CSI sequence: ESC [ <params> <final>
                if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                    i += 2;
                    let param_start = i;
                    // Collect parameter bytes (0x30–0x3F)
                    while i < bytes.len() && (0x30..=0x3F).contains(&bytes[i]) {
                        i += 1;
                    }
                    let param_end = i;
                    // Collect intermediate bytes (0x20–0x2F)
                    while i < bytes.len() && (0x20..=0x2F).contains(&bytes[i]) {
                        i += 1;
                    }
                    // Final byte
                    if i < bytes.len() {
                        let final_byte = bytes[i];
                        i += 1;
                        let key = match final_byte {
                            b'A' => Some(KeyEvent::Up),
                            b'B' => Some(KeyEvent::Down),
                            b'C' => Some(KeyEvent::Right),
                            b'D' => Some(KeyEvent::Left),
                            b'H' => Some(KeyEvent::Home),
                            b'F' => Some(KeyEvent::End),
                            b'~' => {
                                // Tilde-terminated: check param digit
                                let param = &data[param_start..param_end];
                                match param {
                                    "3" => Some(KeyEvent::Delete),
                                    "1" => Some(KeyEvent::Home),
                                    "4" => Some(KeyEvent::End),
                                    _ => None,
                                }
                            }
                            _ => None,
                        };
                        if let Some(k) = key {
                            output.push_str(&self.handle_key(k, backend));
                        }
                    }
                } else {
                    i += 1; // lone ESC
                }
                continue;
            }

            // Control characters and regular input
            let key = match bytes[i] {
                13 => {
                    i += 1;
                    Some(KeyEvent::Enter)
                }
                9 => {
                    i += 1;
                    Some(KeyEvent::Tab)
                }
                127 | 8 => {
                    i += 1;
                    Some(KeyEvent::Backspace)
                }
                1 => {
                    i += 1;
                    Some(KeyEvent::CtrlA)
                }
                3 => {
                    i += 1;
                    Some(KeyEvent::CtrlC)
                }
                4 => {
                    i += 1;
                    Some(KeyEvent::CtrlD)
                }
                5 => {
                    i += 1;
                    Some(KeyEvent::CtrlE)
                }
                12 => {
                    i += 1;
                    Some(KeyEvent::CtrlL)
                }
                21 => {
                    i += 1;
                    Some(KeyEvent::CtrlU)
                }
                23 => {
                    i += 1;
                    Some(KeyEvent::CtrlW)
                }
                b if b >= 32 => {
                    // UTF-8 character
                    let ch = data[i..].chars().next().unwrap();
                    i += ch.len_utf8();
                    Some(KeyEvent::Char(ch))
                }
                _ => {
                    i += 1;
                    None
                }
            };

            if let Some(k) = key {
                output.push_str(&self.handle_key(k, backend));
            }
        }

        output
    }

    // ── Key dispatch ──

    pub fn handle_key(&mut self, key: KeyEvent, backend: &dyn CliBackend) -> String {
        if self.interactive.is_some() {
            self.handle_interactive_key(key, backend)
        } else {
            self.handle_normal_key(key, backend)
        }
    }

    // ── Normal mode ──

    fn handle_normal_key(&mut self, key: KeyEvent, backend: &dyn CliBackend) -> String {
        match key {
            KeyEvent::Char(c) => {
                self.line_buffer.insert(self.cursor_pos, c);
                self.cursor_pos += c.len_utf8();
                self.refresh_line()
            }
            KeyEvent::Enter => {
                let mut out = String::from("\r\n");
                let input = self.line_buffer.trim().to_string();
                self.line_buffer.clear();
                self.cursor_pos = 0;

                if input.is_empty() {
                    out.push_str(PROMPT);
                    return out;
                }

                // Check for interactive mode flag
                let parts = parse_command(&input);
                let has_i =
                    parts.iter().any(|p| p == "-i" || p == "--interactive");
                if has_i {
                    let path = parts
                        .iter()
                        .find(|p| {
                            *p != "call"
                                && *p != "c"
                                && *p != "-i"
                                && *p != "--interactive"
                        })
                        .cloned();
                    if let Some(path) = path {
                        let resolved = resolve_path(&path, backend);
                        self.history.push(input);
                        self.hist_idx = self.history.len() as isize;
                        out.push_str(&self.start_interactive(&resolved, backend));
                        return out;
                    }
                }

                // Normal execution
                self.history.push(input.clone());
                self.hist_idx = self.history.len() as isize;

                let result = exec_line(&input, backend);
                if result.contains(CLEAR_SENTINEL) {
                    return format!("{CLEAR_SENTINEL}{PROMPT}");
                }
                if !result.is_empty() {
                    out.push_str(&result);
                    if !result.ends_with('\n') && !result.ends_with("\r\n") {
                        out.push_str("\r\n");
                    }
                }
                out.push_str(PROMPT);
                out
            }
            KeyEvent::Tab => self.tab_complete_normal(backend),
            KeyEvent::Up => {
                if self.history.is_empty() {
                    return String::new();
                }
                if self.hist_idx > 0 {
                    self.hist_idx -= 1;
                } else if self.hist_idx == -1 && !self.history.is_empty() {
                    self.hist_idx = self.history.len() as isize - 1;
                }
                if self.hist_idx >= 0 {
                    self.line_buffer = self.history[self.hist_idx as usize].clone();
                    self.cursor_pos = self.line_buffer.len();
                }
                self.refresh_line()
            }
            KeyEvent::Down => {
                if self.hist_idx < 0 {
                    return String::new();
                }
                if (self.hist_idx as usize) < self.history.len() - 1 {
                    self.hist_idx += 1;
                    self.line_buffer = self.history[self.hist_idx as usize].clone();
                    self.cursor_pos = self.line_buffer.len();
                } else {
                    self.hist_idx = self.history.len() as isize;
                    self.line_buffer.clear();
                    self.cursor_pos = 0;
                }
                self.refresh_line()
            }
            KeyEvent::CtrlC => {
                self.line_buffer.clear();
                self.cursor_pos = 0;
                self.hist_idx = self.history.len() as isize;
                format!("^C\r\n{PROMPT}")
            }
            KeyEvent::CtrlL => {
                let mut out = String::from(CLEAR_SENTINEL);
                out.push_str(PROMPT);
                out.push_str(&self.line_buffer);
                let tail = self.line_buffer.len() - self.cursor_pos;
                if tail > 0 {
                    out.push_str(&format!("\x1b[{}D", tail));
                }
                out
            }
            _ => self.handle_editing_key(key),
        }
    }

    // ── Interactive mode ──

    fn start_interactive(&mut self, path: &str, backend: &dyn CliBackend) -> String {
        let info = match backend.get_info(path) {
            Some(v) => v,
            None => {
                return format!("{RED}Trait \"{path}\" not found{RESET}\r\n{PROMPT}");
            }
        };

        let params_val = match info.get("params").and_then(|p| p.as_array()) {
            Some(p) if !p.is_empty() => p.clone(),
            _ => {
                let mut out = format!("{GRAY}No parameters — calling directly{RESET}\r\n");
                let result = exec_line(&format!("call {path}"), backend);
                if !result.is_empty() && !result.contains(CLEAR_SENTINEL) {
                    out.push_str(&result);
                    if !result.ends_with('\n') && !result.ends_with("\r\n") {
                        out.push_str("\r\n");
                    }
                }
                out.push_str(PROMPT);
                return out;
            }
        };

        let examples = backend.load_examples(path);
        let params: Vec<ParamMeta> = params_val
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("?").to_string();
                let ptype = p.get("type").and_then(|t| t.as_str()).unwrap_or("any").to_string();
                let desc = p.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                let required = p.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
                let default_val = examples
                    .iter()
                    .filter_map(|ex| ex.get(i).cloned())
                    .next()
                    .unwrap_or_default();
                ParamMeta { name, ptype, description: desc, required, default_val }
            })
            .collect();

        // Build completions for first param
        let first_name = params[0].name.clone();
        let first_default = params[0].default_val.clone();
        let comps = build_completions(
            self.param_history
                .get(path)
                .and_then(|h| h.get(&first_name))
                .map(|v| v.as_slice())
                .unwrap_or(&[]),
            &first_default,
        );

        let desc = info.get("description").and_then(|d| d.as_str()).unwrap_or("");
        let header = format_param_header(&params, 0);

        self.interactive = Some(InteractiveState {
            path: path.to_string(),
            params,
            values: Vec::new(),
            idx: 0,
            completions: comps,
            comp_idx: None,
        });

        format!(
            "\r\n  {BOLD}{path}{RESET}  {GRAY}{desc}{RESET}\r\n\
             {GRAY}  {}{RESET}\r\n\
             {header}{IPROMPT}",
            "─".repeat(50),
        )
    }

    fn handle_interactive_key(&mut self, key: KeyEvent, backend: &dyn CliBackend) -> String {
        match key {
            KeyEvent::Enter => {
                let input = self.line_buffer.trim().to_string();
                self.line_buffer.clear();
                self.cursor_pos = 0;

                // Extract needed data
                let (idx, param_count, path, p_name, required, default_val) = {
                    let i = self.interactive.as_ref().unwrap();
                    let p = &i.params[i.idx];
                    (i.idx, i.params.len(), i.path.clone(), p.name.clone(),
                     p.required, p.default_val.clone())
                };

                let mut out = String::from("\r\n");

                let value = if input.is_empty() && !default_val.is_empty() {
                    default_val
                } else if input.is_empty() && !required {
                    String::new()
                } else if input.is_empty() && required {
                    out.push_str(&format!("  {RED}  required — try again{RESET}\r\n{IPROMPT}"));
                    return out;
                } else {
                    input
                };

                // Update param history
                if !value.is_empty() {
                    let th = self.param_history.entry(path.clone()).or_default();
                    let ph = th.entry(p_name).or_default();
                    ph.retain(|v| v != &value);
                    ph.push(value.clone());
                    if ph.len() > 20 { ph.remove(0); }
                }

                // Advance state
                {
                    let i = self.interactive.as_mut().unwrap();
                    i.values.push(value);
                    i.idx += 1;
                }

                let new_idx = idx + 1;
                if new_idx < param_count {
                    // Prepare next param
                    let (next_name, next_default, header) = {
                        let i = self.interactive.as_ref().unwrap();
                        let np = &i.params[new_idx];
                        (np.name.clone(), np.default_val.clone(),
                         format_param_header(&i.params, new_idx))
                    };
                    let comps = build_completions(
                        self.param_history.get(&path)
                            .and_then(|h| h.get(&next_name))
                            .map(|v| v.as_slice())
                            .unwrap_or(&[]),
                        &next_default,
                    );
                    let i = self.interactive.as_mut().unwrap();
                    i.completions = comps;
                    i.comp_idx = None;

                    out.push_str(&header);
                    out.push_str(IPROMPT);
                } else {
                    // All params collected — execute
                    let i = self.interactive.take().unwrap();
                    out.push_str(&format!("  {GRAY}{}{RESET}\r\n", "─".repeat(50)));

                    let args_str: Vec<String> = i.values.iter().map(|v| {
                        if v.contains(' ') { format!("\"{}\"", v) } else { v.clone() }
                    }).collect();
                    let cmd = format!("call {} {}", i.path, args_str.join(" "));

                    let result = exec_line(&cmd, backend);
                    backend.save_param_history(&self.param_history);

                    if !result.is_empty() && !result.contains(CLEAR_SENTINEL) {
                        out.push_str(&result);
                        if !result.ends_with('\n') && !result.ends_with("\r\n") {
                            out.push_str("\r\n");
                        }
                    }
                    out.push_str(PROMPT);
                }
                out
            }

            KeyEvent::Up => {
                let new_val = {
                    let i = self.interactive.as_mut().unwrap();
                    if i.completions.is_empty() { return String::new(); }
                    i.comp_idx = Some(match i.comp_idx {
                        None => 0,
                        Some(idx) => (idx + 1).min(i.completions.len() - 1),
                    });
                    i.completions[i.comp_idx.unwrap()].clone()
                };
                self.line_buffer = new_val;
                self.cursor_pos = self.line_buffer.len();
                self.refresh_line()
            }

            KeyEvent::Down => {
                let new_val = {
                    let i = self.interactive.as_mut().unwrap();
                    if i.completions.is_empty() { return String::new(); }
                    match i.comp_idx {
                        Some(0) => { i.comp_idx = None; String::new() }
                        Some(idx) => {
                            i.comp_idx = Some(idx - 1);
                            i.completions[i.comp_idx.unwrap()].clone()
                        }
                        None => return String::new(),
                    }
                };
                self.line_buffer = new_val;
                self.cursor_pos = self.line_buffer.len();
                self.refresh_line()
            }

            KeyEvent::Tab => {
                let new_val = {
                    let i = self.interactive.as_mut().unwrap();
                    if i.completions.is_empty() { return String::new(); }
                    i.comp_idx = Some(match i.comp_idx {
                        None => 0,
                        Some(idx) => (idx + 1) % i.completions.len(),
                    });
                    i.completions[i.comp_idx.unwrap()].clone()
                };
                self.line_buffer = new_val;
                self.cursor_pos = self.line_buffer.len();
                self.refresh_line()
            }

            KeyEvent::CtrlC | KeyEvent::CtrlD => {
                self.interactive = None;
                self.line_buffer.clear();
                self.cursor_pos = 0;
                format!("^C\r\n  {GRAY}aborted{RESET}\r\n{PROMPT}")
            }

            KeyEvent::Char(c) => {
                self.line_buffer.insert(self.cursor_pos, c);
                self.cursor_pos += c.len_utf8();
                if let Some(i) = self.interactive.as_mut() { i.comp_idx = None; }
                self.refresh_line()
            }
            KeyEvent::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.line_buffer.remove(self.cursor_pos);
                    if let Some(i) = self.interactive.as_mut() { i.comp_idx = None; }
                    self.refresh_line()
                } else {
                    String::new()
                }
            }
            KeyEvent::CtrlL => {
                let mut out = String::from(CLEAR_SENTINEL);
                out.push_str(IPROMPT);
                out.push_str(&self.line_buffer);
                let tail = self.line_buffer.len() - self.cursor_pos;
                if tail > 0 { out.push_str(&format!("\x1b[{}D", tail)); }
                out
            }
            _ => self.handle_editing_key(key),
        }
    }

    // ── Tab completion (normal mode) ──

    fn tab_complete_normal(&mut self, backend: &dyn CliBackend) -> String {
        let parts: Vec<&str> = self.line_buffer.split_whitespace().collect();
        let prefix = if parts.len() <= 1 {
            parts.first().copied().unwrap_or("")
        } else if matches!(parts[0].to_lowercase().as_str(), "call" | "info" | "c" | "i") {
            parts.last().copied().unwrap_or("")
        } else {
            return String::new();
        };

        let all_paths = backend.all_paths();
        let (matches, common) = tab_completions(prefix, &all_paths);

        if matches.len() == 1 {
            if parts.len() <= 1 {
                self.line_buffer = format!("{} ", matches[0]);
            } else {
                let before: Vec<&str> = parts[..parts.len() - 1].to_vec();
                self.line_buffer = format!("{} {} ", before.join(" "), matches[0]);
            }
            self.cursor_pos = self.line_buffer.len();
            self.refresh_line()
        } else if matches.len() > 1 && matches.len() <= 40 {
            let mut out = String::from("\r\n");
            let max_len = matches.iter().map(|m| m.len()).max().unwrap_or(0) + 2;
            let per_row = (80 / max_len).max(1);
            for chunk in matches.chunks(per_row) {
                for m in chunk {
                    out.push_str(&format!("{CYAN}{:width$}{RESET}", m, width = max_len));
                }
                out.push_str("\r\n");
            }
            if common.len() > prefix.len() {
                if parts.len() <= 1 {
                    self.line_buffer = common;
                } else {
                    let before: Vec<&str> = parts[..parts.len() - 1].to_vec();
                    self.line_buffer = format!("{} {}", before.join(" "), common);
                }
                self.cursor_pos = self.line_buffer.len();
            }
            out.push_str(&self.refresh_line());
            out
        } else {
            String::new()
        }
    }

    // ── Shared editing keys ──

    fn handle_editing_key(&mut self, key: KeyEvent) -> String {
        match key {
            KeyEvent::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    "\x1b[D".to_string()
                } else { String::new() }
            }
            KeyEvent::Right => {
                if self.cursor_pos < self.line_buffer.len() {
                    self.cursor_pos += 1;
                    "\x1b[C".to_string()
                } else { String::new() }
            }
            KeyEvent::Delete => {
                if self.cursor_pos < self.line_buffer.len() {
                    self.line_buffer.remove(self.cursor_pos);
                    self.refresh_line()
                } else { String::new() }
            }
            KeyEvent::Home | KeyEvent::CtrlA => {
                self.cursor_pos = 0;
                self.refresh_line()
            }
            KeyEvent::End | KeyEvent::CtrlE => {
                self.cursor_pos = self.line_buffer.len();
                self.refresh_line()
            }
            KeyEvent::CtrlU => {
                self.line_buffer.clear();
                self.cursor_pos = 0;
                self.refresh_line()
            }
            KeyEvent::CtrlW => {
                let before = &self.line_buffer[..self.cursor_pos];
                let trimmed = before.trim_end_matches(|c: char| !c.is_whitespace());
                let trimmed = trimmed.trim_end();
                let new_pos = trimmed.len();
                self.line_buffer = format!(
                    "{}{}",
                    &self.line_buffer[..new_pos],
                    &self.line_buffer[self.cursor_pos..]
                );
                self.cursor_pos = new_pos;
                self.refresh_line()
            }
            _ => String::new(),
        }
    }

    // ── Line refresh ──

    fn refresh_line(&self) -> String {
        let prompt = if self.interactive.is_some() { IPROMPT } else { PROMPT };
        let mut out = format!("\x1b[2K\r{}{}", prompt, self.line_buffer);
        let tail = self.line_buffer.len() - self.cursor_pos;
        if tail > 0 {
            out.push_str(&format!("\x1b[{}D", tail));
        }
        out
    }
}

// ═══════════════════════════════════════════
// ── Command execution (stateless) ──
// ═══════════════════════════════════════════

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
            // Strip -i/--interactive from args (handled by session)
            let clean: Vec<&String> = args
                .iter()
                .filter(|a| *a != "-i" && *a != "--interactive")
                .collect();
            if clean.is_empty() {
                return format!("{RED}Usage: call <trait_path> [args...]{RESET}");
            }
            let rest: Vec<String> = clean[1..].iter().map(|s| s.to_string()).collect();
            exec_call(backend, clean[0], &rest)
        }
        "search" | "s" => {
            let q = args.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
            if q.is_empty() {
                return format!("{RED}Usage: search <query>{RESET}");
            }
            format_search(backend, &q)
        }
        "version" | "v" => format!("{CYAN}traits.build{RESET} {}", backend.version()),
        "clear" | "cls" => CLEAR_SENTINEL.to_string(),
        _ => {
            let all = backend.all_paths();
            if all.iter().any(|p| p == &cmd) || all.iter().any(|p| p == parts[0].as_str()) {
                exec_call(backend, &parts[0], &args.to_vec())
            } else {
                let sys_path = format!("sys.{}", cmd);
                let kernel_path = format!("kernel.{}", cmd);
                if all.iter().any(|p| p == &sys_path) {
                    exec_call(backend, &sys_path, &args.to_vec())
                } else if all.iter().any(|p| p == &kernel_path) {
                    exec_call(backend, &kernel_path, &args.to_vec())
                } else {
                    format!(
                        "{RED}Unknown command: {}{RESET}. Type {BLUE}help{RESET} for usage.",
                        cmd
                    )
                }
            }
        }
    }
}

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
    s.push_str(&format!("  {GREEN}call -i{RESET} {GRAY}<path>{RESET}           Interactive mode (prompt each param)\r\n"));
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
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Interactive mode{RESET}\r\n"));
    s.push_str(&format!("  {CYAN}↑ / ↓{RESET}        Cycle through parameter history\r\n"));
    s.push_str(&format!("  {CYAN}Tab{RESET}          Cycle through completions\r\n"));
    s.push_str(&format!("  {CYAN}Ctrl+C{RESET}       Abort interactive mode\r\n"));
    s.push_str("\r\n");
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Examples{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}call sys.checksum hash \"hello world\"{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}call -i sys.checksum{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}sys.version{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}info sys.list{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}list sys{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}search checksum{RESET}\r\n"));
    s
}

fn format_list(backend: &dyn CliBackend, namespace: Option<&str>) -> String {
    let all = backend.list_all();
    let filtered: Vec<&Value> = if let Some(ns) = namespace {
        all.iter()
            .filter(|t| {
                t.get("path").and_then(|p| p.as_str()).map_or(false, |p| p.starts_with(ns))
            })
            .collect()
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

    let mut groups: std::collections::BTreeMap<String, Vec<&Value>> =
        std::collections::BTreeMap::new();
    for t in &filtered {
        let path = t.get("path").and_then(|p| p.as_str()).unwrap_or("");
        let parts: Vec<&str> = path.rsplitn(2, '.').collect();
        let ns = if parts.len() > 1 { parts[1] } else { "" };
        groups.entry(ns.to_string()).or_default().push(t);
    }

    let mut out = String::new();
    for (ns, traits) in &groups {
        out.push_str(&format!(
            "{BOLD}{BRIGHT_WHITE}{}{RESET} {GRAY}({}){RESET}\r\n", ns, traits.len()
        ));
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

    out.push_str(&format!(
        "{BOLD}{BRIGHT_WHITE}{}{RESET}  {}  {GRAY}{}{RESET}\r\n", trait_path, badge, version
    ));
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
                out.push_str(&format!(
                    "  {BLUE}{}{RESET} {MAGENTA}({}){RESET}{}  {GRAY}{}{RESET}\r\n",
                    name, ptype, req_mark, pdesc
                ));
            }
        }
    }

    if let Some(ret) = info.get("returns").or_else(|| info.get("returns_type")) {
        let rtype = if let Some(s) = ret.as_str() { s } else { "any" };
        let rdesc = info.get("returns_description").and_then(|d| d.as_str()).unwrap_or("");
        out.push_str("\r\n");
        out.push_str(&format!(
            "{BOLD}Returns:{RESET} {MAGENTA}{}{RESET}  {GRAY}{}{RESET}", rtype, rdesc
        ));
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
    if let Ok(v) = serde_json::from_str::<Value>(s) {
        return v;
    }
    if let Ok(n) = s.parse::<i64>() {
        return Value::from(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::from(f);
    }
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
    let matches: Vec<String> = all_paths
        .iter()
        .filter(|p| p.starts_with(prefix))
        .cloned()
        .collect();

    if matches.is_empty() {
        return (matches, String::new());
    }

    let mut common = matches[0].clone();
    for m in &matches[1..] {
        while !m.starts_with(&common) {
            common.pop();
        }
    }

    (matches, common)
}

/// Get interactive mode parameter info for a trait.
pub fn interactive_params(path: &str, backend: &dyn CliBackend) -> Option<Value> {
    backend.get_info(path).and_then(|info| info.get("params").cloned())
}

// ── Helpers ──

fn resolve_path(path: &str, backend: &dyn CliBackend) -> String {
    let all = backend.all_paths();
    if all.iter().any(|p| p == path) {
        return path.to_string();
    }
    let sys = format!("sys.{}", path);
    if all.iter().any(|p| p == &sys) {
        return sys;
    }
    let kernel = format!("kernel.{}", path);
    if all.iter().any(|p| p == &kernel) {
        return kernel;
    }
    path.to_string()
}

fn format_param_header(params: &[ParamMeta], idx: usize) -> String {
    let p = &params[idx];
    let req = if p.required { format!("{RED}*{RESET}") } else { " ".to_string() };
    let mut out = format!(
        "  {} {BOLD}{}{RESET}  {GRAY}{}{RESET}  {GRAY}{}{RESET}",
        req, p.name, p.ptype, p.description
    );
    if !p.default_val.is_empty() {
        out.push_str(&format!("  {GRAY}[{}]{RESET}", p.default_val));
    }
    out.push_str("\r\n");
    out
}

fn build_completions(history: &[String], default_val: &str) -> Vec<String> {
    let mut completions: Vec<String> = history.iter().rev().cloned().collect();
    let mut seen = std::collections::HashSet::new();
    completions.retain(|v| seen.insert(v.clone()));
    if !default_val.is_empty() && !seen.contains(default_val) {
        completions.push(default_val.to_string());
    }
    completions
}

// ── Native dispatch entry point ──

pub fn cli_dispatch(_args: &[Value]) -> Value {
    Value::String("kernel.cli: use CliSession.feed() with a CliBackend".to_string())
}
