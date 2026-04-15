use regex::Regex;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub mod shell;
pub mod vfs;
pub use shell::{DefaultShell, Shell, ShellCommand};
pub use vfs::{LayeredVfs, MemVfs, Vfs};

mod generated_cli_formatters {
    include!(concat!(env!("OUT_DIR"), "/cli_formatters.rs"));
}

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

pub const PROMPT: &str = "\x1b[32mtraits \x1b[0m";
const IPROMPT: &str = "  \x1b[96m❯\x1b[0m ";

/// Sentinel returned by clear — frontends intercept this.
pub const CLEAR_SENTINEL: &str = "\x1b[CLEAR]";

/// REST dispatch sentinels — frontend intercepts and makes fetch().
pub const REST_SENTINEL_START: &str = "\x1b[REST]";
pub const REST_SENTINEL_END: &str = "\x1b[/REST]";

/// WebLLM dispatch sentinels — frontend intercepts and calls WebLLM engine.
pub const WEBLLM_SENTINEL_START: &str = "\x1b[WEBLLM]";
pub const WEBLLM_SENTINEL_END: &str = "\x1b[/WEBLLM]";

// ── Key events ──

pub enum KeyEvent {
    Char(char),
    Enter,
    Tab,
    Up,
    Down,
    Left,
    Right,
    WordLeft,
    WordRight,
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

/// Registry + dispatch interface used by command execution.
pub trait CliCallBackend {
    fn call(&self, path: &str, args: &[Value]) -> Result<Value, String>;
    fn list_all(&self) -> Vec<Value>;
    fn get_info(&self, path: &str) -> Option<Value>;
    fn search(&self, query: &str) -> Vec<Value>;
    fn all_paths(&self) -> Vec<String>;
    fn version(&self) -> String;
}

/// Parameter history interface used by interactive sessions.
pub trait CliHistoryBackend {
    fn load_param_history(&self) -> HashMap<String, HashMap<String, Vec<String>>> {
        HashMap::new()
    }
    fn save_param_history(&self, _history: &HashMap<String, HashMap<String, Vec<String>>>) {}
}

/// Example source interface used for interactive suggestions.
pub trait CliExamplesBackend {
    fn load_examples(&self, _path: &str) -> Vec<Vec<String>> {
        vec![]
    }
}

/// Full backend used by the session runtime.
pub trait CliBackend: CliCallBackend + CliHistoryBackend + CliExamplesBackend {}
impl<T> CliBackend for T where T: CliCallBackend + CliHistoryBackend + CliExamplesBackend {}

// ── Interactive mode state ──

struct ParamMeta {
    name: String,
    ptype: String,
    description: String,
    required: bool,
    default_val: String,
    example_vals: Vec<String>,
}

struct InteractiveState {
    path: String,
    params: Vec<ParamMeta>,
    values: Vec<String>,
    idx: usize,
    history_values: Vec<String>,
    history_idx: Option<usize>,
    tab_values: Vec<String>,
    tab_idx: Option<usize>,
}

// ── Chat mode state ──

/// Chat sentinel — terminal.js intercepts this and handles the ACP REST call,
/// stores the conversation, then writes the return prompt.
pub const CHAT_SENTINEL_START: &str = "\x1b[CHAT]";
pub const CHAT_SENTINEL_END: &str = "\x1b[/CHAT]";

/// Voice sentinel — terminal.js intercepts this and initiates voice mode.
/// The helper must be connected for voice to work (requires native sox for mic/speaker).
pub const VOICE_SENTINEL_START: &str = "\x1b[VOICE]";
pub const VOICE_SENTINEL_END: &str = "\x1b[/VOICE]";

const CHAT_PROMPT: &str = "\x1b[96mchat❯\x1b[0m ";

struct ChatState {
    agent: String,
    model: String,
    cwd: String,
    session_id: String,
    message_count: usize,
}

// ── CLI Session ──

pub struct CliSession {
    line_buffer: String,
    cursor_pos: usize,
    history: Vec<String>,
    hist_idx: isize,
    interactive: Option<InteractiveState>,
    chat: Option<ChatState>,
    param_history: HashMap<String, HashMap<String, Vec<String>>>,
    /// Shell parser — swap via `set_shell()` to plug in a full implementation.
    shell: Arc<dyn Shell>,
    /// Virtual filesystem — swap via `set_vfs()` for richer backends.
    vfs: RefCell<Box<dyn Vfs>>,
    /// Current working directory for shell-like relative path resolution.
    cwd: String,
}

impl CliSession {
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            hist_idx: -1,
            interactive: None,
            chat: None,
            param_history: HashMap::new(),
            shell: Arc::new(DefaultShell),
            // Platform::make_vfs() returns the right backend for the current target:
            //   Native → LayeredVfs seeded by walking the real TRAITS_DIR on disk.
            //   WASM   → LayeredVfs seeded from embedded include_str! assets.
            //   Uninitialised → MemVfs fallback (tests, early init).
            vfs: RefCell::new(kernel_logic::platform::make_vfs()),
            cwd: "/".to_string(),
        }
    }

    /// Swap the shell parser.  Future: `session.set_shell(Box::new(MvdanShell::new()))`.
    pub fn set_shell(&mut self, shell: impl Shell + 'static) {
        self.shell = Arc::new(shell);
    }

    /// Swap the VFS backend.  Future: bind to Origin Private FS or a real FS.
    pub fn set_vfs(&mut self, vfs: impl Vfs + 'static) {
        self.vfs = RefCell::new(Box::new(vfs));
    }

    /// Serialise the VFS to JSON (for localStorage persistence).
    pub fn vfs_dump(&self) -> String {
        self.vfs.borrow().dump()
    }

    /// Restore the VFS from a JSON string previously produced by `vfs_dump`.
    pub fn vfs_load(&self, json: &str) {
        self.vfs.borrow_mut().load(json);
    }

    /// Read a single file from the VFS.
    pub fn vfs_read(&self, path: &str) -> Option<String> {
        self.vfs.borrow().read(path)
    }

    /// Write a single file to the VFS.
    pub fn vfs_write(&self, path: &str, content: &str) {
        self.vfs.borrow_mut().write(path, content);
    }

    /// Load persisted param history from backend (call after new).
    pub fn load_history(&mut self, backend: &dyn CliHistoryBackend) {
        self.param_history = backend.load_param_history();
    }

    pub fn is_interactive(&self) -> bool {
        self.interactive.is_some()
    }

    /// Return the current command history (most recent last).
    pub fn get_history(&self) -> &[String] {
        &self.history
    }

    /// Restore command history (e.g. from localStorage on WASM startup).
    pub fn set_history(&mut self, history: Vec<String>) {
        self.hist_idx = history.len() as isize;
        self.history = history;
    }

    /// Return the welcome banner + initial prompt.
    pub fn welcome(&self, backend: &dyn CliCallBackend) -> String {
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
                        let param = &data[param_start..param_end];
                        let word_motion = param.contains(";3") || param.contains(";9");
                        let key = match final_byte {
                            b'A' => Some(KeyEvent::Up),
                            b'B' => Some(KeyEvent::Down),
                            b'C' => Some(if word_motion { KeyEvent::WordRight } else { KeyEvent::Right }),
                            b'D' => Some(if word_motion { KeyEvent::WordLeft } else { KeyEvent::Left }),
                            b'H' => Some(KeyEvent::Home),
                            b'F' => Some(KeyEvent::End),
                            b'~' => {
                                // Tilde-terminated: check param digit
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
                    // Meta/Alt sequences commonly sent by terminals.
                    if i + 1 < bytes.len() {
                        let key = match bytes[i + 1] {
                            b'b' | b'B' => Some(KeyEvent::WordLeft),
                            b'f' | b'F' => Some(KeyEvent::WordRight),
                            _ => None,
                        };
                        if let Some(k) = key {
                            i += 2;
                            output.push_str(&self.handle_key(k, backend));
                            continue;
                        }
                    }
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
        } else if self.chat.is_some() {
            self.handle_chat_key(key, backend)
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
                let has_i = parts.iter().any(|p| p == "-i" || p == "--interactive");
                if has_i {
                    let path = parts
                        .iter()
                        .find(|p| *p != "call" && *p != "c" && *p != "-i" && *p != "--interactive")
                        .cloned();
                    if let Some(path) = path {
                        let resolved = resolve_path(&path, backend);
                        self.history.push(input);
                        self.hist_idx = self.history.len() as isize;
                        out.push_str(&self.start_interactive(&resolved, backend));
                        return out;
                    }
                }

                // Check for chat mode command
                if parts.first().map(|s| s.to_lowercase()).as_deref() == Some("chat") {
                    let agent = parts.get(1).map(|s| s.as_str()).unwrap_or("opencode");
                    let model = parts.get(2).map(|s| s.as_str()).unwrap_or("");
                    self.history.push(input);
                    self.hist_idx = self.history.len() as isize;
                    out.push_str(&self.start_chat(agent, model, None));
                    return out;
                }

                // Normal execution
                self.history.push(input.clone());
                self.hist_idx = self.history.len() as isize;

                let result = exec_line(&input, backend, &*self.shell, &self.vfs, &mut self.cwd);
                if result.contains(CLEAR_SENTINEL) {
                    return format!("{CLEAR_SENTINEL}{PROMPT}");
                }
                if result.contains(REST_SENTINEL_START) || result.contains(WEBLLM_SENTINEL_START) {
                    out.push_str(&result);
                    return out; // No prompt — JS handles async REST/WebLLM
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
                let result = exec_line(
                    &format!("call {path}"),
                    backend,
                    &*self.shell,
                    &self.vfs,
                    &mut self.cwd,
                );
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
                let name = p
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("?")
                    .to_string();
                let ptype = p
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("any")
                    .to_string();
                let desc = p
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let required = p.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
                let example_vals: Vec<String> = examples
                    .iter()
                    .filter_map(|ex| ex.get(i).cloned())
                    .filter(|v| !v.is_empty())
                    .collect();
                let default_val = p
                    .get("default")
                    .and_then(|d| d.as_str())
                    .map(|d| d.to_string())
                    .filter(|d| !d.is_empty())
                    .or_else(|| example_vals.first().cloned())
                    .unwrap_or_default();
                ParamMeta {
                    name,
                    ptype,
                    description: desc,
                    required,
                    default_val,
                    example_vals,
                }
            })
            .collect();

        // Build completions for first param
        let first_name = params[0].name.clone();
        let first_default = params[0].default_val.clone();
        let first_examples = params[0].example_vals.clone();
        let history_values = build_history_completions(
            self.param_history
                .get(path)
                .and_then(|h| h.get(&first_name))
                .map(|v| v.as_slice())
                .unwrap_or(&[]),
        );
        let tab_values = build_tab_completions(&first_default, &first_examples);

        let desc = info
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");
        let header = format_param_header(&params, 0);

        self.interactive = Some(InteractiveState {
            path: path.to_string(),
            params,
            values: Vec::new(),
            idx: 0,
            history_values,
            history_idx: None,
            tab_values,
            tab_idx: None,
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
                    (
                        i.idx,
                        i.params.len(),
                        i.path.clone(),
                        p.name.clone(),
                        p.required,
                        p.default_val.clone(),
                    )
                };

                let mut out = String::from("\r\n");

                let value = if input.is_empty() && !default_val.is_empty() {
                    default_val
                } else if input.is_empty() && !required {
                    String::new()
                } else if input.is_empty() && required {
                    out.push_str(&format!(
                        "  {RED}  required — try again{RESET}\r\n{IPROMPT}"
                    ));
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
                    if ph.len() > 20 {
                        ph.remove(0);
                    }
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
                    let (next_name, next_default, next_examples, header) = {
                        let i = self.interactive.as_ref().unwrap();
                        let np = &i.params[new_idx];
                        (
                            np.name.clone(),
                            np.default_val.clone(),
                            np.example_vals.clone(),
                            format_param_header(&i.params, new_idx),
                        )
                    };
                    let history_values = build_history_completions(
                        self.param_history
                            .get(&path)
                            .and_then(|h| h.get(&next_name))
                            .map(|v| v.as_slice())
                            .unwrap_or(&[]),
                    );
                    let tab_values = build_tab_completions(&next_default, &next_examples);
                    let i = self.interactive.as_mut().unwrap();
                    i.history_values = history_values;
                    i.history_idx = None;
                    i.tab_values = tab_values;
                    i.tab_idx = None;

                    out.push_str(&header);
                    out.push_str(IPROMPT);
                } else {
                    // All params collected — execute
                    let i = self.interactive.take().unwrap();
                    out.push_str(&format!("  {GRAY}{}{RESET}\r\n", "─".repeat(50)));

                    let args_str: Vec<String> = i
                        .values
                        .iter()
                        .map(|v| {
                            if v.contains(' ') {
                                format!("\"{}\"", v)
                            } else {
                                v.clone()
                            }
                        })
                        .collect();
                    let cmd = format!("call {} {}", i.path, args_str.join(" "));

                    let result = exec_line(&cmd, backend, &*self.shell, &self.vfs, &mut self.cwd);
                    backend.save_param_history(&self.param_history);

                    if result.contains(REST_SENTINEL_START)
                        || result.contains(WEBLLM_SENTINEL_START)
                    {
                        out.push_str(&result);
                        return out; // No prompt — JS handles async REST/WebLLM
                    }
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
                    if i.history_values.is_empty() {
                        return String::new();
                    }
                    i.history_idx = Some(match i.history_idx {
                        None => 0,
                        Some(idx) => (idx + 1).min(i.history_values.len() - 1),
                    });
                    i.history_values[i.history_idx.unwrap()].clone()
                };
                self.line_buffer = new_val;
                self.cursor_pos = self.line_buffer.len();
                self.refresh_line()
            }

            KeyEvent::Down => {
                let new_val = {
                    let i = self.interactive.as_mut().unwrap();
                    if i.history_values.is_empty() {
                        return String::new();
                    }
                    match i.history_idx {
                        Some(0) => {
                            i.history_idx = None;
                            String::new()
                        }
                        Some(idx) => {
                            i.history_idx = Some(idx - 1);
                            i.history_values[i.history_idx.unwrap()].clone()
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
                    if i.tab_values.is_empty() {
                        return String::new();
                    }
                    i.tab_idx = Some(match i.tab_idx {
                        None => 0,
                        Some(idx) => (idx + 1) % i.tab_values.len(),
                    });
                    i.tab_values[i.tab_idx.unwrap()].clone()
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
                if let Some(i) = self.interactive.as_mut() {
                    i.history_idx = None;
                    i.tab_idx = None;
                }
                self.refresh_line()
            }
            KeyEvent::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.line_buffer.remove(self.cursor_pos);
                    if let Some(i) = self.interactive.as_mut() {
                        i.history_idx = None;
                        i.tab_idx = None;
                    }
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
                if tail > 0 {
                    out.push_str(&format!("\x1b[{}D", tail));
                }
                out
            }
            _ => self.handle_editing_key(key),
        }
    }

    // ── Chat mode ──

    /// Enter interactive chat mode with an ACP agent.
    ///
    /// If `resume_id` is Some, resumes that session from disk.
    /// If None, creates a new session via sys.chat.
    pub fn start_chat(&mut self, agent: &str, model: &str, resume_id: Option<&str>) -> String {
        let cwd = ".".to_string();

        let (session_id, message_count, actual_agent, actual_model, resumed) =
            if let Some(rid) = resume_id {
                // Try resuming: load session metadata from disk
                if let Some(result) =
                    kernel_logic::platform::dispatch("sys.chat", &[json!("get"), json!(rid)])
                {
                    if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                        if let Some(sess) = result.get("session") {
                            let mc = sess
                                .get("messages")
                                .and_then(|v| v.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);
                            let sa = sess
                                .get("agent")
                                .and_then(|v| v.as_str())
                                .unwrap_or(agent)
                                .to_string();
                            let sm = sess
                                .get("model")
                                .and_then(|v| v.as_str())
                                .unwrap_or(model)
                                .to_string();
                            // Mark it as current
                            kernel_logic::platform::dispatch(
                                "sys.chat",
                                &[json!("switch"), json!(rid)],
                            );
                            (rid.to_string(), mc, sa, sm, true)
                        } else {
                            // Session data missing — create new
                            Self::create_chat_session(agent, model)
                        }
                    } else {
                        Self::create_chat_session(agent, model)
                    }
                } else {
                    Self::create_chat_session(agent, model)
                }
            } else {
                Self::create_chat_session(agent, model)
            };

        self.chat = Some(ChatState {
            agent: actual_agent.clone(),
            model: actual_model.clone(),
            cwd,
            session_id: session_id.clone(),
            message_count,
        });
        let mut out = String::new();
        out.push_str(&format!("\r\n{CYAN}{BOLD}Chat mode{RESET}"));
        out.push_str(&format!(" {GRAY}(agent: {actual_agent}"));
        if !actual_model.is_empty() {
            out.push_str(&format!(", model: {actual_model}"));
        }
        if resumed {
            out.push_str(&format!(", session: {session_id}, {message_count} msgs"));
        }
        out.push_str(&format!("){RESET}\r\n"));
        out.push_str(&format!("{GRAY}Type a message to chat. Commands: /sessions, /new, /agent, /model, /status, /history, /help, /quit{RESET}\r\n"));
        out.push_str(CHAT_PROMPT);
        out
    }

    /// Create a new chat session via sys.chat and return (id, count, agent, model, resumed).
    fn create_chat_session(agent: &str, model: &str) -> (String, usize, String, String, bool) {
        if let Some(result) = kernel_logic::platform::dispatch(
            "sys.chat",
            &[json!("new"), json!(agent), json!(model)],
        ) {
            if let Some(sid) = result.get("session_id").and_then(|v| v.as_str()) {
                return (
                    sid.to_string(),
                    0,
                    agent.to_string(),
                    model.to_string(),
                    false,
                );
            }
        }
        // Fallback: generate ID locally if sys.chat not available
        let (y, mo, d, h, m, s) = kernel_logic::platform::time::now_utc();
        let session_id = format!("{y:04}{mo:02}{d:02}_{h:02}{m:02}{s:02}");
        (session_id, 0, agent.to_string(), model.to_string(), false)
    }

    /// Check if session is in chat mode.
    pub fn is_chat(&self) -> bool {
        self.chat.is_some()
    }

    fn handle_chat_key(&mut self, key: KeyEvent, _backend: &dyn CliBackend) -> String {
        match key {
            KeyEvent::Char(c) => {
                self.line_buffer.insert(self.cursor_pos, c);
                self.cursor_pos += c.len_utf8();
                // In chat mode, echo char directly when appending at end
                // to avoid full refresh_line which breaks with line-wrapping
                // in native terminals (\x1b[2K only clears one physical line).
                if self.cursor_pos == self.line_buffer.len() {
                    c.to_string()
                } else {
                    self.refresh_line()
                }
            }
            KeyEvent::Enter => {
                let input = self.line_buffer.trim().to_string();
                self.line_buffer.clear();
                self.cursor_pos = 0;

                if input.is_empty() {
                    return format!("\r\n{CHAT_PROMPT}");
                }

                let mut out = String::from("\r\n");

                // Handle / commands
                if input.starts_with('/') {
                    let parts: Vec<&str> = input.splitn(2, char::is_whitespace).collect();
                    let cmd = parts[0].to_lowercase();
                    let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

                    match cmd.as_str() {
                        "/quit" | "/exit" | "/q" => {
                            self.chat = None;
                            out.push_str(&format!("{GRAY}Exited chat mode{RESET}\r\n{PROMPT}"));
                            return out;
                        }
                        "/agent" => {
                            if arg.is_empty() {
                                let agent = &self.chat.as_ref().unwrap().agent;
                                out.push_str(&format!(
                                    "{GRAY}Current agent: {CYAN}{agent}{RESET}\r\n"
                                ));
                                out.push_str(&format!(
                                    "{GRAY}Available: opencode, claude, codex, copilot{RESET}\r\n"
                                ));
                            } else {
                                let valid = ["opencode", "claude", "codex", "copilot"];
                                if valid.contains(&arg) {
                                    self.chat.as_mut().unwrap().agent = arg.to_string();
                                    out.push_str(&format!(
                                        "{GREEN}Agent set to {CYAN}{arg}{RESET}\r\n"
                                    ));
                                } else {
                                    out.push_str(&format!("{RED}Unknown agent: {arg}. Available: opencode, claude, codex, copilot{RESET}\r\n"));
                                }
                            }
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        "/model" => {
                            if arg.is_empty() {
                                let model = &self.chat.as_ref().unwrap().model;
                                if model.is_empty() {
                                    out.push_str(&format!(
                                        "{GRAY}Model: (agent default){RESET}\r\n"
                                    ));
                                } else {
                                    out.push_str(&format!("{GRAY}Model: {CYAN}{model}{RESET}\r\n"));
                                }
                                out.push_str(&format!("{GRAY}Use /model <id> to change, /models to list available{RESET}\r\n"));
                            } else {
                                self.chat.as_mut().unwrap().model = arg.to_string();
                                out.push_str(&format!(
                                    "{GREEN}Model set to {CYAN}{arg}{RESET}\r\n"
                                ));
                            }
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        "/models" => {
                            // List models via REST sentinel (native-only trait)
                            let agent = self.chat.as_ref().unwrap().agent.clone();
                            let cwd = self.chat.as_ref().unwrap().cwd.clone();
                            out.push_str(&format!("{GRAY}Fetching models…{RESET}\r\n"));
                            let sentinel = serde_json::json!({
                                "p": "llm.prompt.acp.list",
                                "a": [&agent, &cwd],
                                "rp": CHAT_PROMPT
                            });
                            out.push_str(&format!(
                                "{REST_SENTINEL_START}{}{REST_SENTINEL_END}",
                                sentinel
                            ));
                            return out;
                        }
                        "/status" => {
                            let chat = self.chat.as_ref().unwrap();
                            out.push_str(&format!("{GRAY}Agent: {CYAN}{}{RESET}\r\n", chat.agent));
                            let m = if chat.model.is_empty() {
                                "(agent default)"
                            } else {
                                &chat.model
                            };
                            out.push_str(&format!("{GRAY}Model: {CYAN}{m}{RESET}\r\n"));
                            out.push_str(&format!("{GRAY}Session: {}{RESET}\r\n", chat.session_id));
                            out.push_str(&format!(
                                "{GRAY}Messages: {}{RESET}\r\n",
                                chat.message_count
                            ));
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        "/clear" => {
                            out.push_str(CLEAR_SENTINEL);
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        "/history" => {
                            let session_id = self.chat.as_ref().unwrap().session_id.clone();
                            // Read from disk via sys.chat (persistent)
                            if let Some(result) = kernel_logic::platform::dispatch(
                                "sys.chat",
                                &[json!("get"), json!(&session_id)],
                            ) {
                                if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                                    if let Some(msgs) = result
                                        .get("session")
                                        .and_then(|s| s.get("messages"))
                                        .and_then(|m| m.as_array())
                                    {
                                        for msg in msgs {
                                            let role = msg
                                                .get("role")
                                                .and_then(|r| r.as_str())
                                                .unwrap_or("?");
                                            let text = msg
                                                .get("content")
                                                .and_then(|c| c.as_str())
                                                .unwrap_or("");
                                            let color = if role == "user" { GREEN } else { CYAN };
                                            let label = if role == "user" { "You" } else { "AI" };
                                            out.push_str(&format!("{color}{BOLD}{label}:{RESET} "));
                                            for line in text.lines() {
                                                out.push_str(line);
                                                out.push_str("\r\n");
                                            }
                                        }
                                        if msgs.is_empty() {
                                            out.push_str(&format!(
                                                "{GRAY}No messages yet{RESET}\r\n"
                                            ));
                                        }
                                    } else {
                                        out.push_str(&format!("{GRAY}No messages yet{RESET}\r\n"));
                                    }
                                } else {
                                    out.push_str(&format!("{GRAY}No messages yet{RESET}\r\n"));
                                }
                            } else {
                                out.push_str(&format!("{GRAY}No messages yet{RESET}\r\n"));
                            }
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        "/sessions" | "/ls" => {
                            if let Some(result) =
                                kernel_logic::platform::dispatch("sys.chat", &[json!("list")])
                            {
                                if let Some(sessions) =
                                    result.get("sessions").and_then(|v| v.as_array())
                                {
                                    if sessions.is_empty() {
                                        out.push_str(&format!(
                                            "{GRAY}No saved sessions{RESET}\r\n"
                                        ));
                                    } else {
                                        let current_sid =
                                            self.chat.as_ref().map(|c| c.session_id.as_str());
                                        out.push_str(&format!(
                                            "{BOLD}{BRIGHT_WHITE}Sessions:{RESET}\r\n"
                                        ));
                                        for s in sessions {
                                            let sid = s
                                                .get("session_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("?");
                                            let agent = s
                                                .get("agent")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("?");
                                            let mc = s
                                                .get("messages")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(0);
                                            let marker =
                                                if current_sid == Some(sid) { " ◀" } else { "" };
                                            out.push_str(&format!("  {CYAN}{sid}{RESET} {GRAY}{agent} ({mc} msgs){marker}{RESET}\r\n"));
                                        }
                                        out.push_str(&format!(
                                            "{GRAY}Switch: /session <id>{RESET}\r\n"
                                        ));
                                    }
                                }
                            } else {
                                out.push_str(&format!(
                                    "{GRAY}Session listing not available{RESET}\r\n"
                                ));
                            }
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        "/session" => {
                            if arg.is_empty() {
                                let chat = self.chat.as_ref().unwrap();
                                out.push_str(&format!(
                                    "{GRAY}Current session: {CYAN}{}{RESET}\r\n",
                                    chat.session_id
                                ));
                                out.push_str(&format!(
                                    "{GRAY}Use /session <id> to switch{RESET}\r\n"
                                ));
                            } else {
                                // Switch to a different session
                                if let Some(result) = kernel_logic::platform::dispatch(
                                    "sys.chat",
                                    &[json!("switch"), json!(arg)],
                                ) {
                                    if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                                        let mc = result
                                            .get("messages")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0)
                                            as usize;
                                        let agent = result
                                            .get("agent")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or(&self.chat.as_ref().unwrap().agent)
                                            .to_string();
                                        let model = result
                                            .get("model")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or(&self.chat.as_ref().unwrap().model)
                                            .to_string();
                                        let chat = self.chat.as_mut().unwrap();
                                        chat.session_id = arg.to_string();
                                        chat.message_count = mc;
                                        chat.agent = agent;
                                        chat.model = model.clone();
                                        out.push_str(&format!(
                                            "{GREEN}Switched to session {CYAN}{arg}{RESET}"
                                        ));
                                        out.push_str(&format!(" {GRAY}({mc} msgs){RESET}\r\n"));
                                    } else {
                                        let err = result
                                            .get("error")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("not found");
                                        out.push_str(&format!("{RED}{err}{RESET}\r\n"));
                                    }
                                }
                            }
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        "/new" => {
                            let chat = self.chat.as_ref().unwrap();
                            let agent = if !arg.is_empty() {
                                arg.to_string()
                            } else {
                                chat.agent.clone()
                            };
                            let model = chat.model.clone();
                            let (sid, _, _, _, _) = Self::create_chat_session(&agent, &model);
                            let chat = self.chat.as_mut().unwrap();
                            chat.session_id = sid.clone();
                            chat.message_count = 0;
                            chat.agent = agent.clone();
                            out.push_str(&format!("{GREEN}New session: {CYAN}{sid}{RESET}"));
                            out.push_str(&format!(" {GRAY}(agent: {agent}){RESET}\r\n"));
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        "/delete" => {
                            if arg.is_empty() {
                                out.push_str(&format!(
                                    "{GRAY}Usage: /delete <session_id>{RESET}\r\n"
                                ));
                            } else {
                                let current_sid = self.chat.as_ref().map(|c| c.session_id.as_str());
                                if current_sid == Some(arg) {
                                    out.push_str(&format!(
                                        "{RED}Cannot delete the active session{RESET}\r\n"
                                    ));
                                } else if let Some(result) = kernel_logic::platform::dispatch(
                                    "sys.chat",
                                    &[json!("delete"), json!(arg)],
                                ) {
                                    if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                                        out.push_str(&format!(
                                            "{GREEN}Deleted session {arg}{RESET}\r\n"
                                        ));
                                    } else {
                                        let err = result
                                            .get("error")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("failed");
                                        out.push_str(&format!("{RED}{err}{RESET}\r\n"));
                                    }
                                }
                            }
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        "/voice" => {
                            // Switch to voice mode — OpenAI Realtime API or local WebGPU
                            // "local" arg forces local voice (WebGPU STT + LLM + TTS)
                            // "local-realtime" arg forces Voxtral local-realtime mode
                            let is_voxtral = arg == "local-realtime";
                            let is_local = !is_voxtral && (arg == "local" || arg.starts_with("local "));
                            let voice_name = if is_voxtral || is_local {
                                let rest = if is_voxtral {
                                    ""
                                } else {
                                    arg.strip_prefix("local").unwrap_or("").trim()
                                };
                                if rest.is_empty() { "af_heart" } else { rest }
                            } else if arg.is_empty() {
                                "shimmer"
                            } else {
                                arg
                            };
                            let (agent, _model, session_id) = if let Some(ref c) = self.chat {
                                (
                                    c.agent.as_str().to_string(),
                                    c.model.as_str().to_string(),
                                    c.session_id.clone(),
                                )
                            } else {
                                ("".to_string(), "".to_string(), "".to_string())
                            };
                            let mode_label = if is_voxtral { "Voxtral local-realtime voice" } else if is_local { "local voice" } else { "voice" };
                            out.push_str(&format!("{GRAY}Switching to {mode_label} mode…{RESET}\r\n"));
                            let sentinel = serde_json::json!({
                                "v": voice_name,
                                "m": "gpt-realtime-mini-2025-12-15",
                                "a": agent,
                                "s": session_id,
                                "rp": CHAT_PROMPT,
                                "local": is_local,
                                "voxtral": is_voxtral,
                            });
                            out.push_str(&format!(
                                "{VOICE_SENTINEL_START}{}{VOICE_SENTINEL_END}",
                                sentinel
                            ));
                            return out;
                        }
                        "/help" | "/?" => {
                            out.push_str(&format!("{BOLD}{BRIGHT_WHITE}Chat commands{RESET}\r\n"));
                            out.push_str(&format!(
                                "  {GREEN}/sessions{RESET}            List all saved sessions\r\n"
                            ));
                            out.push_str(&format!("  {GREEN}/session{RESET} {GRAY}[id]{RESET}      Show or switch session\r\n"));
                            out.push_str(&format!("  {GREEN}/new{RESET} {GRAY}[agent]{RESET}       Start a new session\r\n"));
                            out.push_str(&format!("  {GREEN}/delete{RESET} {GRAY}<id>{RESET}       Delete a session\r\n"));
                            out.push_str(&format!("  {GREEN}/agent{RESET} {GRAY}[name]{RESET}      Show or switch ACP agent\r\n"));
                            out.push_str(&format!("  {GREEN}/model{RESET} {GRAY}[id]{RESET}        Show or switch model\r\n"));
                            out.push_str(&format!(
                                "  {GREEN}/models{RESET}              List available models\r\n"
                            ));
                            out.push_str(&format!("  {GREEN}/voice{RESET} {GRAY}[name]{RESET}      Switch to voice I/O (speak/listen)\r\n"));
                            out.push_str(&format!("  {GREEN}/voice local{RESET}         Local voice (WebGPU STT + LLM + TTS)\r\n"));
                            out.push_str(&format!("  {GREEN}/voice local-realtime{RESET} Voxtral STT (local) + cloud LLM + Kokoro TTS\r\n"));
                            out.push_str(&format!(
                                "  {GREEN}/status{RESET}              Show session status\r\n"
                            ));
                            out.push_str(&format!("  {GREEN}/history{RESET}             Show conversation history\r\n"));
                            out.push_str(&format!(
                                "  {GREEN}/clear{RESET}               Clear terminal\r\n"
                            ));
                            out.push_str(&format!(
                                "  {GREEN}/quit{RESET}                Exit chat mode\r\n"
                            ));
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                        _ => {
                            out.push_str(&format!(
                                "{RED}Unknown command: {cmd}. Type /help for commands.{RESET}\r\n"
                            ));
                            out.push_str(CHAT_PROMPT);
                            return out;
                        }
                    }
                }

                // Regular message — persist to disk and dispatch via ACP
                self.history.push(input.clone());
                self.hist_idx = self.history.len() as isize;

                let (agent, model, cwd, session_id) = {
                    let c = self.chat.as_mut().unwrap();
                    c.message_count += 1;
                    (
                        c.agent.clone(),
                        c.model.clone(),
                        c.cwd.clone(),
                        c.session_id.clone(),
                    )
                };

                // Save user message to disk via sys.chat
                kernel_logic::platform::dispatch(
                    "sys.chat",
                    &[
                        json!("append"),
                        json!(&session_id),
                        json!("user"),
                        json!(&input),
                    ],
                );

                // Build REST sentinel for llm.prompt.acp (streaming)
                let model_arg = if model.is_empty() { "" } else { &model };
                let sentinel = serde_json::json!({
                    "p": "llm.prompt.acp",
                    "a": [&input, &agent, &cwd, "false", model_arg],
                    "sid": &session_id,
                    "rp": CHAT_PROMPT,
                    "stream": true
                });

                out.push_str(&format!("{GRAY}thinking…{RESET}\r\n"));
                out.push_str(&format!(
                    "{REST_SENTINEL_START}{}{REST_SENTINEL_END}",
                    sentinel
                ));
                out
            }

            KeyEvent::Tab => String::new(), // No tab completion in chat mode
            KeyEvent::Up => {
                if self.history.is_empty() {
                    return String::new();
                }
                if self.hist_idx > 0 {
                    self.hist_idx -= 1;
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
                self.chat = None;
                self.line_buffer.clear();
                self.cursor_pos = 0;
                format!("^C\r\n{GRAY}Exited chat mode{RESET}\r\n{PROMPT}")
            }
            KeyEvent::CtrlD => {
                self.chat = None;
                self.line_buffer.clear();
                self.cursor_pos = 0;
                format!("\r\n{GRAY}Exited chat mode{RESET}\r\n{PROMPT}")
            }
            KeyEvent::CtrlL => {
                let mut out = String::from(CLEAR_SENTINEL);
                out.push_str(CHAT_PROMPT);
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

    // ── Tab completion (normal mode) ──

    fn tab_complete_normal(&mut self, backend: &dyn CliBackend) -> String {
        let parts: Vec<&str> = self.line_buffer.split_whitespace().collect();
        let prefix = if parts.len() <= 1 {
            parts.first().copied().unwrap_or("")
        } else if matches!(
            parts[0].to_lowercase().as_str(),
            "call" | "info" | "c" | "i"
        ) {
            parts.last().copied().unwrap_or("")
        } else {
            return String::new();
        };

        let completion_pool: Vec<String> = if parts.len() <= 1 {
            let mut set: HashSet<String> = shell_builtin_commands().into_iter().collect();
            for p in backend.all_paths() {
                set.insert(p);
            }
            let mut vals: Vec<String> = set.into_iter().collect();
            vals.sort();
            vals
        } else {
            backend.all_paths()
        };

        let (matches, common) = tab_completions(prefix, &completion_pool);

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
            KeyEvent::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.line_buffer.remove(self.cursor_pos);
                    self.refresh_line()
                } else {
                    String::new()
                }
            }
            KeyEvent::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    "\x1b[D".to_string()
                } else {
                    String::new()
                }
            }
            KeyEvent::Right => {
                if self.cursor_pos < self.line_buffer.len() {
                    self.cursor_pos += 1;
                    "\x1b[C".to_string()
                } else {
                    String::new()
                }
            }
            KeyEvent::WordLeft => {
                let new_pos = cursor_word_left(&self.line_buffer, self.cursor_pos);
                if new_pos != self.cursor_pos {
                    self.cursor_pos = new_pos;
                    self.refresh_line()
                } else {
                    String::new()
                }
            }
            KeyEvent::WordRight => {
                let new_pos = cursor_word_right(&self.line_buffer, self.cursor_pos);
                if new_pos != self.cursor_pos {
                    self.cursor_pos = new_pos;
                    self.refresh_line()
                } else {
                    String::new()
                }
            }
            KeyEvent::Delete => {
                if self.cursor_pos < self.line_buffer.len() {
                    self.line_buffer.remove(self.cursor_pos);
                    self.refresh_line()
                } else {
                    String::new()
                }
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
        let prompt = if self.interactive.is_some() {
            IPROMPT
        } else if self.chat.is_some() {
            CHAT_PROMPT
        } else {
            PROMPT
        };
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
/// Process a single command line through the Shell + Vfs abstraction layer.
///
/// `shell` handles word-splitting and redirect detection.
/// `vfs` is a `RefCell<Box<dyn Vfs>>` so builtins can mutate it without
/// requiring `exec_line` to take `&mut`.
///
/// Returns ANSI-formatted output ready to write to the terminal.
pub fn exec_line(
    line: &str,
    backend: &dyn CliBackend,
    shell: &dyn Shell,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &mut String,
) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let segments = split_logical_chain(trimmed);
    if segments.is_empty() {
        return String::new();
    }

    let mut outputs = Vec::new();
    let mut last_success = true;
    let mut has_last = false;

    for (seg, gate) in segments {
        if let Some(op) = gate {
            if !has_last {
                continue;
            }
            match op {
                LogicalOp::And if !last_success => continue,
                LogicalOp::Or if last_success => continue,
                _ => {}
            }
        }

        let mut parsed = shell.parse(&seg);
        if parsed.args.is_empty() {
            continue;
        }

        let output = execute_shell_command(&mut parsed, None, backend, vfs, cwd);
        last_success = infer_command_success(&parsed, &output);
        has_last = true;
        if !output.is_empty() {
            outputs.push(output);
        }
    }

    outputs.join("\n")
}

#[derive(Clone, Copy)]
enum LogicalOp {
    And,
    Or,
}

fn split_logical_chain(line: &str) -> Vec<(String, Option<LogicalOp>)> {
    let bytes = line.as_bytes();
    if bytes.is_empty() {
        return vec![];
    }

    let mut parts: Vec<(String, Option<LogicalOp>)> = Vec::new();
    let mut start = 0usize;
    let mut gate_for_segment: Option<LogicalOp> = None;
    let mut i = 0usize;
    let mut quote: Option<u8> = None;
    let mut escaped = false;

    while i + 1 < bytes.len() {
        let b = bytes[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        if let Some(q) = quote {
            if b == b'\\' {
                escaped = true;
                i += 1;
                continue;
            }
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }

        if b == b'\'' || b == b'"' {
            quote = Some(b);
            i += 1;
            continue;
        }
        if b == b'\\' {
            escaped = true;
            i += 1;
            continue;
        }

        let op = if b == b'&' && bytes[i + 1] == b'&' {
            Some(LogicalOp::And)
        } else if b == b'|' && bytes[i + 1] == b'|' {
            Some(LogicalOp::Or)
        } else {
            None
        };

        if let Some(found) = op {
            let seg = line[start..i].trim();
            if !seg.is_empty() {
                parts.push((seg.to_string(), gate_for_segment));
                gate_for_segment = Some(found);
            }
            start = i + 2;
            i += 2;
            continue;
        }

        i += 1;
    }

    let tail = line[start..].trim();
    if !tail.is_empty() {
        parts.push((tail.to_string(), gate_for_segment));
    }
    parts
}

fn infer_command_success(parsed: &ShellCommand, output: &str) -> bool {
    let leaf = last_pipeline_command(parsed);
    let name = leaf.cmd().to_lowercase();
    match name.as_str() {
        "true" => true,
        "false" => false,
        "test" => eval_test_expr(leaf.rest()),
        "[" => {
            let args = leaf.rest();
            if args.last().map(|s| s.as_str()) != Some("]") {
                false
            } else {
                eval_test_expr(&args[..args.len().saturating_sub(1)])
            }
        }
        _ => !output.contains(RED),
    }
}

fn last_pipeline_command(cmd: &ShellCommand) -> &ShellCommand {
    let mut cur = cmd;
    while let Some(next) = cur.pipe_next.as_ref() {
        cur = next;
    }
    cur
}

fn execute_shell_command(
    cmd: &mut ShellCommand,
    stdin_input: Option<String>,
    backend: &dyn CliBackend,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &mut String,
) -> String {
    let effective_stdin = if let Some(path) = &cmd.stdin_from {
        let full = resolve_vfs_path(cwd, path);
        match vfs.borrow().read(&full) {
            Some(content) => Some(content),
            None => return format!("{RED}<{}: no such file{RESET}", path),
        }
    } else {
        stdin_input
    };

    let output = execute_leaf_command(cmd, effective_stdin, backend, vfs, cwd);
    let piped_input = strip_ansi(&output);

    if let Some(next) = &mut cmd.pipe_next {
        return execute_shell_command(next, Some(piped_input), backend, vfs, cwd);
    }

    if let Some(redir) = &cmd.redirect {
        let plain = strip_ansi(&output);
        let full = resolve_vfs_path(cwd, &redir.file);
        if redir.append {
            vfs.borrow_mut().append(&full, &plain);
            vfs.borrow_mut().append(&full, "\n");
        } else {
            vfs.borrow_mut().write(&full, &plain);
        }
        return format!("{GRAY}→ {}{RESET}", full);
    }

    output
}

fn execute_leaf_command(
    cmd: &mut ShellCommand,
    stdin_input: Option<String>,
    backend: &dyn CliBackend,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &mut String,
) -> String {
    // ── @file argument expansion ─────────────────────────────────────────────
    {
        let vfs_ref = vfs.borrow();
        for arg in &mut cmd.args {
            if arg.starts_with('@') {
                let fname = &arg[1..];
                if let Some(contents) = vfs_ref.read(fname) {
                    *arg = contents;
                } else {
                    return format!("{RED}@{fname}: file not found in VFS{RESET}");
                }
            }
        }
    }

    let cmd_name = cmd.args[0].to_lowercase();
    let args = cmd.args[1..].to_vec();

    match cmd_name.as_str() {
        "echo" => args.join(" "),
        "pwd" => cwd.clone(),
        "true" => String::new(),
        "false" => String::new(),
        "cat" => cat_command(&args, stdin_input, vfs, cwd),
        "head" => head_tail_command(&args, stdin_input, vfs, cwd, false),
        "tail" => head_tail_command(&args, stdin_input, vfs, cwd, true),
        "grep" => grep_command(&args, stdin_input, vfs, cwd),
        "wc" => wc_command(&args, stdin_input, vfs, cwd),
        "sed" => sed_command(&args, stdin_input, vfs, cwd),
        "test" => test_command(&args),
        "[" => bracket_test_command(&args),
        "find" => find_command(&args, vfs, cwd),
        "curl" => curl_command(&args, backend, vfs, cwd),
        "vi" | "ee" => editor_stub_command(&cmd_name, &args, stdin_input, vfs, cwd),
        "stat" => stat_command(&args, vfs, cwd),

        "mkdir" => mkdir_command(&args, vfs, cwd),

        "write" | "tee" => {
            if args.len() < 2 {
                format!("{RED}Usage: write <file> <content>{RESET}")
            } else {
                let content = args[1..].join(" ");
                let full = resolve_vfs_path(cwd, &args[0]);
                vfs.borrow_mut().write(&full, &content);
                format!("{GRAY}wrote {} bytes to {}{RESET}", content.len(), full)
            }
        }
        "rm" => {
            if args.is_empty() {
                format!("{RED}Usage: rm <path>{RESET}")
            } else {
                let full = resolve_vfs_path(cwd, &args[0]);
                if vfs.borrow_mut().delete(&full) {
                    format!("{GRAY}removed {}{RESET}", full)
                } else {
                    format!("{RED}rm: {}: no such path{RESET}", args[0])
                }
            }
        }
        "ls" => {
            let target = if args.is_empty() {
                cwd.clone()
            } else {
                resolve_vfs_path(cwd, &args[0])
            };
            let prefix = target.trim_start_matches('/').trim_end_matches('/');
            let files = vfs.borrow().list();
            let dirs = vfs.borrow().list_dirs();
            let filtered: Vec<String> = files
                .into_iter()
                .filter(|f| {
                    let k = f.trim_start_matches('/');
                    if prefix.is_empty() {
                        true
                    } else {
                        k == prefix || (k.starts_with(prefix) && k.len() > prefix.len())
                    }
                })
                .collect();
            let filtered_dirs: Vec<String> = dirs
                .into_iter()
                .filter(|d| {
                    let k = d.trim_start_matches('/');
                    if prefix.is_empty() {
                        true
                    } else {
                        k == prefix || (k.starts_with(prefix) && k.len() > prefix.len())
                    }
                })
                .collect();
            if filtered.is_empty() && filtered_dirs.is_empty() && !vfs.borrow().is_dir(&target) {
                format!("{RED}ls: {}: no such path{RESET}", target)
            } else {
                let tree_prefix = if prefix.is_empty() {
                    "".to_string()
                } else {
                    format!("{prefix}/")
                };
                format_vfs_tree(&filtered, &filtered_dirs, &tree_prefix)
            }
        }
        "cd" => {
            let target = if args.is_empty() {
                "/".to_string()
            } else {
                resolve_vfs_path(cwd, &args[0])
            };
            if vfs.borrow().is_dir(&target) {
                *cwd = target;
                String::new()
            } else {
                format!("{RED}cd: {}: no such directory{RESET}", args.get(0).cloned().unwrap_or_else(|| "/".to_string()))
            }
        }

        "help" | "h" | "?" => format_help(),
        "list" => format_list(backend, args.first().map(|s| s.as_str())),
        "info" | "i" => {
            if args.is_empty() {
                format_system_status(backend)
            } else {
                format_info(backend, &args[0])
            }
        }
        "call" | "c" => {
            if args.is_empty() {
                return format!("{RED}Usage: call <trait_path> [args...]{RESET}");
            }
            let clean: Vec<String> = args
                .iter()
                .filter(|a| *a != "-i" && *a != "--interactive")
                .cloned()
                .collect();
            if clean.is_empty() {
                return format!("{RED}Usage: call <trait_path> [args...]{RESET}");
            }
            exec_call(backend, &clean[0], &clean[1..])
        }
        "search" | "s" => {
            let q = args
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            if q.is_empty() {
                return format!("{RED}Usage: search <query>{RESET}");
            }
            format_search(backend, &q)
        }
        "version" | "v" => format!("{CYAN}traits.build{RESET} {}", backend.version()),
        "clear" | "cls" => CLEAR_SENTINEL.to_string(),

        _ => {
            let all = backend.all_paths();
            let raw = &cmd.args[0];
            let (clean_cmd, _) = strip_dispatch_target(&cmd_name);
            let (clean_raw, target) = strip_dispatch_target(raw);
            if all.iter().any(|p| p == clean_cmd) || all.iter().any(|p| p == clean_raw) {
                exec_call(backend, raw, &args)
            } else {
                let sys_path = format!("sys.{}", clean_cmd);
                let kernel_path = format!("kernel.{}", clean_cmd);
                if all.iter().any(|p| p == &sys_path) {
                    let full = match target {
                        Some(t) => format!("{}@{}", sys_path, t),
                        None => sys_path,
                    };
                    exec_call(backend, &full, &args)
                } else if all.iter().any(|p| p == &kernel_path) {
                    let full = match target {
                        Some(t) => format!("{}@{}", kernel_path, t),
                        None => kernel_path,
                    };
                    exec_call(backend, &full, &args)
                } else {
                    format!(
                        "{RED}Unknown command: {}{RESET}. Type {BLUE}help{RESET} for usage.",
                        clean_cmd
                    )
                }
            }
        }
    }
}

fn cat_command(
    args: &[String],
    stdin_input: Option<String>,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &str,
) -> String {
    if args.is_empty() {
        return stdin_input.unwrap_or_default();
    }
    let mut out = String::new();
    for (idx, path) in args.iter().enumerate() {
        let full = resolve_vfs_path(cwd, path);
        match vfs.borrow().read(&full) {
            Some(content) => {
                if idx > 0 {
                    out.push('\n');
                }
                out.push_str(&content);
            }
            None => return format!("{RED}cat: {}: no such file{RESET}", path),
        }
    }
    out
}

fn head_tail_command(
    args: &[String],
    stdin_input: Option<String>,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &str,
    tail: bool,
) -> String {
    let mut n: usize = 10;
    let mut file: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        if (args[i] == "-n" || args[i] == "--lines") && i + 1 < args.len() {
            if let Ok(parsed) = args[i + 1].parse::<usize>() {
                n = parsed;
            }
            i += 2;
            continue;
        }
        if let Some(rest) = args[i].strip_prefix("-n") {
            if let Ok(parsed) = rest.parse::<usize>() {
                n = parsed;
                i += 1;
                continue;
            }
        }
        file = Some(&args[i]);
        i += 1;
    }

    let text = if let Some(path) = file {
        let full = resolve_vfs_path(cwd, path);
        match vfs.borrow().read(&full) {
            Some(content) => content,
            None => return format!("{RED}{}: {}: no such file{RESET}", if tail {"tail"} else {"head"}, path),
        }
    } else {
        stdin_input.unwrap_or_default()
    };

    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let slice: Vec<&str> = if tail {
        let start = lines.len().saturating_sub(n);
        lines[start..].to_vec()
    } else {
        lines[..lines.len().min(n)].to_vec()
    };
    slice.join("\n")
}

fn grep_command(
    args: &[String],
    stdin_input: Option<String>,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &str,
) -> String {
    let mut case_insensitive = false;
    let mut with_numbers = false;
    let mut invert = false;
    let mut idx = 0;
    while idx < args.len() && args[idx].starts_with('-') && args[idx] != "-" {
        for ch in args[idx].chars().skip(1) {
            match ch {
                'i' => case_insensitive = true,
                'n' => with_numbers = true,
                'v' => invert = true,
                _ => {}
            }
        }
        idx += 1;
    }
    if idx >= args.len() {
        return format!("{RED}Usage: grep [-inv] <pattern> [file]{RESET}");
    }
    let pattern = args[idx].clone();
    idx += 1;
    let text = if idx < args.len() {
        let full = resolve_vfs_path(cwd, &args[idx]);
        match vfs.borrow().read(&full) {
            Some(content) => content,
            None => return format!("{RED}grep: {}: no such file{RESET}", args[idx]),
        }
    } else {
        stdin_input.unwrap_or_default()
    };
    let needle = if case_insensitive {
        pattern.to_lowercase()
    } else {
        pattern
    };
    let mut out = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let hay = if case_insensitive { line.to_lowercase() } else { line.to_string() };
        let is_match = hay.contains(&needle);
        if is_match ^ invert {
            if with_numbers {
                out.push(format!("{}:{}", line_no + 1, line));
            } else {
                out.push(line.to_string());
            }
        }
    }
    out.join("\n")
}

fn wc_command(
    args: &[String],
    stdin_input: Option<String>,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &str,
) -> String {
    let mut count_lines = false;
    let mut count_words = false;
    let mut count_bytes = false;
    let mut file: Option<&str> = None;
    for arg in args {
        if arg.starts_with('-') && arg != "-" {
            for ch in arg.chars().skip(1) {
                match ch {
                    'l' => count_lines = true,
                    'w' => count_words = true,
                    'c' => count_bytes = true,
                    _ => {}
                }
            }
        } else {
            file = Some(arg);
        }
    }
    if !count_lines && !count_words && !count_bytes {
        count_lines = true;
        count_words = true;
        count_bytes = true;
    }
    let text = if let Some(path) = file {
        let full = resolve_vfs_path(cwd, path);
        match vfs.borrow().read(&full) {
            Some(content) => content,
            None => return format!("{RED}wc: {}: no such file{RESET}", path),
        }
    } else {
        stdin_input.unwrap_or_default()
    };
    let lines = text.lines().count();
    let words = text.split_whitespace().count();
    let bytes = text.len();
    let mut parts: Vec<String> = Vec::new();
    if count_lines {
        parts.push(lines.to_string());
    }
    if count_words {
        parts.push(words.to_string());
    }
    if count_bytes {
        parts.push(bytes.to_string());
    }
    if let Some(path) = file {
        parts.push(path.to_string());
    }
    parts.join(" ")
}

fn sed_command(
    args: &[String],
    stdin_input: Option<String>,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &str,
) -> String {
    if args.is_empty() {
        return format!("{RED}Usage: sed [-i] 's/pattern/repl/[gi]' [file]{RESET}");
    }

    let mut in_place = false;
    let mut idx = 0usize;
    while idx < args.len() && args[idx].starts_with('-') {
        match args[idx].as_str() {
            "-i" => {
                in_place = true;
                idx += 1;
            }
            _ => break,
        }
    }
    if idx >= args.len() {
        return format!("{RED}Usage: sed [-i] 's/pattern/repl/[gi]' [file]{RESET}");
    }

    let script = args[idx].as_str();
    idx += 1;

    let file_arg = if idx < args.len() {
        Some(args[idx].clone())
    } else {
        None
    };
    if in_place && file_arg.is_none() {
        return format!("{RED}sed: -i requires a file path{RESET}");
    }

    let text = if let Some(path) = file_arg.as_ref() {
        let full = resolve_vfs_path(cwd, path);
        match vfs.borrow().read(&full) {
            Some(content) => content,
            None => return format!("{RED}sed: {}: no such file{RESET}", path),
        }
    } else {
        stdin_input.unwrap_or_default()
    };

    let Some((pattern, replacement, flags)) = parse_sed_substitute(script) else {
        return format!("{RED}sed: only s/pattern/replacement/[gi] is supported{RESET}");
    };

    let global = flags.contains('g');
    let case_insensitive = flags.contains('i');
    let pattern_expr = if case_insensitive {
        format!("(?i){pattern}")
    } else {
        pattern.clone()
    };
    let re = match Regex::new(&pattern_expr) {
        Ok(v) => v,
        Err(e) => return format!("{RED}sed: invalid regex: {e}{RESET}"),
    };

    let rendered = if global {
        re.replace_all(&text, replacement.as_str()).to_string()
    } else {
        re.replace(&text, replacement.as_str()).to_string()
    };

    if in_place {
        if let Some(path) = file_arg {
            let full = resolve_vfs_path(cwd, &path);
            vfs.borrow_mut().write(&full, &rendered);
            format!("{GRAY}updated {}{RESET}", full)
        } else {
            String::new()
        }
    } else {
        rendered
    }
}

fn parse_sed_substitute(script: &str) -> Option<(String, String, String)> {
    let mut chars = script.chars();
    if chars.next()? != 's' {
        return None;
    }
    let delim = chars.next()?;
    let rest: String = chars.collect();
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    for ch in rest.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            cur.push(ch);
            continue;
        }
        if ch == delim {
            parts.push(cur);
            cur = String::new();
            if parts.len() == 3 {
                break;
            }
            continue;
        }
        cur.push(ch);
    }
    if parts.len() < 2 {
        return None;
    }
    let flags = if parts.len() >= 3 {
        parts[2].clone()
    } else {
        String::new()
    };
    Some((parts[0].clone(), parts[1].clone(), flags))
}

fn test_command(args: &[String]) -> String {
    let _ = eval_test_expr(args);
    String::new()
}

fn bracket_test_command(args: &[String]) -> String {
    if args.last().map(|s| s.as_str()) != Some("]") {
        return format!("{RED}[: missing closing ]{RESET}");
    }
    let _ = eval_test_expr(&args[..args.len().saturating_sub(1)]);
    String::new()
}

fn eval_test_expr(args: &[String]) -> bool {
    if args.is_empty() {
        return false;
    }
    if args[0] == "!" {
        return !eval_test_expr(&args[1..]);
    }
    if args.len() == 1 {
        return !args[0].is_empty();
    }
    if args.len() == 2 {
        match args[0].as_str() {
            "-n" => !args[1].is_empty(),
            "-z" => args[1].is_empty(),
            "-e" | "-f" => !args[1].is_empty(),
            _ => false,
        }
    } else {
        match args[1].as_str() {
            "=" => args[0] == args[2],
            "!=" => args[0] != args[2],
            "-eq" | "-ne" | "-gt" | "-ge" | "-lt" | "-le" => {
                let lhs = args[0].parse::<i64>().ok();
                let rhs = args[2].parse::<i64>().ok();
                match (lhs, rhs, args[1].as_str()) {
                    (Some(a), Some(b), "-eq") => a == b,
                    (Some(a), Some(b), "-ne") => a != b,
                    (Some(a), Some(b), "-gt") => a > b,
                    (Some(a), Some(b), "-ge") => a >= b,
                    (Some(a), Some(b), "-lt") => a < b,
                    (Some(a), Some(b), "-le") => a <= b,
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

fn find_command(args: &[String], vfs: &RefCell<Box<dyn Vfs>>, cwd: &str) -> String {
    let mut start = ".".to_string();
    let mut name_pattern: Option<String> = None;
    let mut type_filter: Option<char> = None;
    let mut min_depth: usize = 0;
    let mut max_depth: Option<usize> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-name" if i + 1 < args.len() => {
                name_pattern = Some(args[i + 1].clone());
                i += 2;
            }
            "-type" if i + 1 < args.len() => {
                type_filter = args[i + 1].chars().next();
                i += 2;
            }
            "-mindepth" if i + 1 < args.len() => {
                min_depth = args[i + 1].parse::<usize>().unwrap_or(0);
                i += 2;
            }
            "-maxdepth" if i + 1 < args.len() => {
                max_depth = args[i + 1].parse::<usize>().ok();
                i += 2;
            }
            x if !x.starts_with('-') && start == "." => {
                start = x.to_string();
                i += 1;
            }
            _ => i += 1,
        }
    }

    if let Some(t) = type_filter {
        if t != 'f' && t != 'd' {
            return format!("{RED}find: unsupported -type '{}', expected 'f' or 'd'{RESET}", t);
        }
    }

    let start_abs = resolve_vfs_path(cwd, &start);
    let start_norm = start_abs.trim_start_matches('/').to_string();
    let files = vfs.borrow().list();
    let dirs: std::collections::BTreeSet<String> = vfs.borrow().list_dirs().into_iter().collect();

    let mut entries: Vec<(String, bool)> = Vec::new();
    for d in &dirs {
        entries.push((d.trim_start_matches('/').to_string(), true));
    }
    for f in &files {
        entries.push((f.trim_start_matches('/').to_string(), false));
    }

    let mut out = Vec::new();
    for (norm, is_dir) in entries {
        if !start_norm.is_empty()
            && norm != start_norm
            && !norm.starts_with(&format!("{}/", start_norm))
        {
            continue;
        }

        let depth = if start_norm.is_empty() {
            norm.matches('/').count() + 1
        } else if norm == start_norm {
            0
        } else {
            let rel = norm
                .strip_prefix(&(start_norm.clone() + "/"))
                .unwrap_or(norm.as_str());
            rel.matches('/').count() + 1
        };

        if depth < min_depth {
            continue;
        }
        if let Some(md) = max_depth {
            if depth > md {
                continue;
            }
        }

        if matches!(type_filter, Some('d')) && !is_dir {
            continue;
        }
        if matches!(type_filter, Some('f')) && is_dir {
            continue;
        }

        if let Some(ref pat) = name_pattern {
            let base = norm.rsplit('/').next().unwrap_or(&norm);
            if !wildcard_match(pat, base) {
                continue;
            }
        }

        out.push(format!("/{}", norm));
    }
    out.sort();
    out.dedup();
    out.join("\n")
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut match_i) = (None::<usize>, 0usize);

    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            pi += 1;
            match_i = ti;
        } else if let Some(star_pi) = star {
            pi = star_pi + 1;
            match_i += 1;
            ti = match_i;
        } else {
            return false;
        }
    }

    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn curl_command(
    args: &[String],
    backend: &dyn CliBackend,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &str,
) -> String {
    let mut method: Option<String> = None;
    let mut body: Option<String> = None;
    let mut output_file: Option<String> = None;
    let mut silent = false;
    let mut headers: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut url: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-s" | "--silent" => {
                silent = true;
                i += 1;
            }
            "-X" | "--request" if i + 1 < args.len() => {
                method = Some(args[i + 1].to_uppercase());
                i += 2;
            }
            "-d" | "--data" | "--data-raw" if i + 1 < args.len() => {
                body = Some(args[i + 1].clone());
                i += 2;
            }
            "-H" | "--header" if i + 1 < args.len() => {
                if let Some((k, v)) = args[i + 1].split_once(':') {
                    headers.insert(k.trim().to_string(), Value::String(v.trim().to_string()));
                }
                i += 2;
            }
            "-o" | "--output" if i + 1 < args.len() => {
                output_file = Some(args[i + 1].clone());
                i += 2;
            }
            x if !x.starts_with('-') => {
                url = Some(x.to_string());
                i += 1;
            }
            _ => i += 1,
        }
    }

    let Some(url) = url else {
        return format!("{RED}Usage: curl [-s] [-X METHOD] [-H 'K: V'] [-d BODY] [-o FILE] <url>{RESET}");
    };

    let mut call_args: Vec<String> = vec![url];
    call_args.push(body.unwrap_or_default());
    call_args.push(String::new());
    call_args.push(method.unwrap_or_else(|| if call_args[1].is_empty() { "GET".to_string() } else { "POST".to_string() }));
    if headers.is_empty() {
        call_args.push(String::new());
    } else {
        call_args.push(Value::Object(headers).to_string());
    }

    let result = exec_call(backend, "sys.call", &call_args);
    if let Some(path) = output_file {
        let full = resolve_vfs_path(cwd, &path);
        let plain = strip_ansi(&result);
        vfs.borrow_mut().write(&full, &plain);
        return format!("{GRAY}saved response to {}{RESET}", full);
    }
    if silent {
        String::new()
    } else {
        result
    }
}

fn editor_stub_command(
    editor: &str,
    args: &[String],
    stdin_input: Option<String>,
    vfs: &RefCell<Box<dyn Vfs>>,
    cwd: &str,
) -> String {
    if args.is_empty() {
        return format!("{RED}Usage: {} [-a] <file> [content...]{RESET}", editor);
    }

    let mut append = false;
    let mut idx = 0usize;
    while idx < args.len() && args[idx].starts_with('-') {
        match args[idx].as_str() {
            "-a" | "--append" => append = true,
            _ => return format!("{RED}{}: unsupported option {}{RESET}", editor, args[idx]),
        }
        idx += 1;
    }

    if idx >= args.len() {
        return format!("{RED}Usage: {} [-a] <file> [content...]{RESET}", editor);
    }

    let path = resolve_vfs_path(cwd, &args[idx]);
    idx += 1;

    let content_from_args = if idx < args.len() {
        Some(args[idx..].join(" "))
    } else {
        None
    };
    let content_from_stdin = stdin_input.filter(|s| !s.is_empty());

    if let Some(content) = content_from_stdin.or(content_from_args) {
        if append {
            vfs.borrow_mut().append(&path, &content);
            vfs.borrow_mut().append(&path, "\n");
            return format!("{GRAY}appended {} bytes to {}{RESET}", content.len(), path);
        }
        vfs.borrow_mut().write(&path, &content);
        return format!("{GRAY}saved {} bytes to {}{RESET}", content.len(), path);
    }

    let body = vfs.borrow().read(&path).unwrap_or_default();
    if body.is_empty() {
        return format!(
            "{YELLOW}{editor}: {} (empty){RESET}\r\n{GRAY}Pipe content to save: echo \"text\" | {editor} {}{RESET}",
            path,
            path
        );
    }

    let mut numbered = String::new();
    for (n, line) in body.lines().enumerate() {
        numbered.push_str(&format!("{GRAY}{:>4} |{RESET} {}\r\n", n + 1, line));
    }
    if numbered.ends_with("\r\n") {
        numbered.truncate(numbered.len() - 2);
    }

    format!(
        "{YELLOW}{} preview mode{RESET} {GRAY}({}){RESET}\r\n{GRAY}Use: {} -a {} <text>  or  echo \"...\" | {} {}{RESET}\r\n{}",
        editor,
        path,
        editor,
        path,
        editor,
        path,
        numbered
    )
}

fn stat_command(args: &[String], vfs: &RefCell<Box<dyn Vfs>>, cwd: &str) -> String {
    if args.is_empty() {
        return format!("{RED}Usage: stat <file>{RESET}");
    }
    let path = resolve_vfs_path(cwd, &args[0]);
    let vfs_ref = vfs.borrow();
    if !vfs_ref.exists(&path) {
        return format!("{RED}stat: {}: no such file or directory{RESET}", args[0]);
    }
    if vfs_ref.is_dir(&path) {
        return format!("  {BOLD}{path}{RESET}  (directory)");
    }
    let size = vfs_ref.read(&path).map(|c| c.len()).unwrap_or(0);
    match vfs_ref.stat(&path) {
        Some((created, modified)) => {
            let fmt_ts = |ts: u64| -> String {
                if ts == 0 { return "unknown".to_string(); }
                // 2026-04-15T14:23:01Z style
                let secs = ts % 60;
                let mins = (ts / 60) % 60;
                let hours = (ts / 3600) % 24;
                let days = ts / 86400;
                // Approximate calendar date from days since epoch (good 1970-2100)
                let ymd = days_to_ymd(days);
                format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", ymd.0, ymd.1, ymd.2, hours, mins, secs)
            };
            format!(
                "  File: {BOLD}{path}{RESET}\n  Size: {} bytes\n  Created:  {}\n  Modified: {}",
                size, fmt_ts(created), fmt_ts(modified)
            )
        }
        None => {
            // File exists in builtin layer (no user timestamps)
            format!("  File: {BOLD}{path}{RESET}\n  Size: {} bytes\n  Origin: builtin (deploy file)", size)
        }
    }
}

/// Convert days since Unix epoch to (year, month, day).
/// Uses the Gregorian proleptic calendar algorithm.
fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u32, m as u32, d as u32)
}

fn mkdir_command(args: &[String], vfs: &RefCell<Box<dyn Vfs>>, cwd: &str) -> String {
    if args.is_empty() {
        return format!("{RED}Usage: mkdir [-p] <dir...>{RESET}");
    }
    let mut p_flag = false;
    let mut targets = Vec::new();
    for a in args {
        if a == "-p" {
            p_flag = true;
        } else {
            targets.push(a.clone());
        }
    }
    if targets.is_empty() {
        return format!("{RED}Usage: mkdir [-p] <dir...>{RESET}");
    }

    let mut created = Vec::new();
    for t in targets {
        let full = resolve_vfs_path(cwd, &t);
        let full_trim = full.trim_start_matches('/').to_string();
        if full_trim.is_empty() {
            continue;
        }

        if !p_flag {
            let parent = parent_dir(&full);
            if !vfs.borrow().is_dir(&parent) {
                return format!("{RED}mkdir: cannot create directory '{}': No such file or directory{RESET}", t);
            }
        } else {
            let mut cur = String::new();
            for seg in full_trim.split('/') {
                if !cur.is_empty() {
                    cur.push('/');
                }
                cur.push_str(seg);
                vfs.borrow_mut().mkdir(&cur);
            }
            created.push(full.clone());
            continue;
        }

        if vfs.borrow().exists(&full) && !vfs.borrow().is_dir(&full) {
            return format!("{RED}mkdir: cannot create directory '{}': File exists{RESET}", t);
        }
        if vfs.borrow_mut().mkdir(&full) {
            created.push(full);
        }
    }

    if created.is_empty() {
        String::new()
    } else {
        format!("{GRAY}created {}{RESET}", created.join(", "))
    }
}

fn parent_dir(path: &str) -> String {
    let t = path.trim_end_matches('/').trim_start_matches('/');
    if t.is_empty() {
        return "/".to_string();
    }
    if let Some((p, _)) = t.rsplit_once('/') {
        if p.is_empty() {
            "/".to_string()
        } else {
            format!("/{p}")
        }
    } else {
        "/".to_string()
    }
}

fn resolve_vfs_path(cwd: &str, path: &str) -> String {
    let raw = if path.is_empty() {
        cwd.to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else if cwd == "/" {
        format!("/{path}")
    } else {
        format!("{}/{}", cwd.trim_end_matches('/'), path)
    };

    let mut parts: Vec<&str> = Vec::new();
    for seg in raw.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg);
        }
    }
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

/// Render a VFS file list as a directory tree.
///
/// `prefix` is the directory context already shown (e.g. `"traits/sys/"`).
/// Files at the next depth level are shown as entries; deeper paths are
/// collapsed into `dir/  (N)` summary lines.
fn format_vfs_tree(files: &[String], dirs: &[String], prefix: &str) -> String {
    use std::collections::BTreeMap;
    if files.is_empty() && dirs.is_empty() {
        return format!("{GRAY}(vfs empty){RESET}");
    }
    let pfx = prefix.trim_end_matches('/');
    let mut dir_entries: BTreeMap<String, usize> = BTreeMap::new();
    let mut file_entries: Vec<String> = Vec::new();
    for d in dirs {
        let rel = if pfx.is_empty() {
            d.trim_start_matches('/')
        } else {
            d.trim_start_matches('/')
                .strip_prefix(pfx)
                .unwrap_or(d.as_str())
                .trim_start_matches('/')
        };
        if rel.is_empty() {
            continue;
        }
        if let Some(slash_pos) = rel.find('/') {
            *dir_entries.entry(rel[..slash_pos].to_string()).or_insert(0) += 1;
        } else {
            dir_entries.entry(rel.to_string()).or_insert(0);
        }
    }
    for f in files {
        let rel = if pfx.is_empty() {
            f.trim_start_matches('/')
        } else {
            f.trim_start_matches('/')
                .strip_prefix(pfx)
                .unwrap_or(f.as_str())
                .trim_start_matches('/')
        };
        if let Some(slash_pos) = rel.find('/') {
            *dir_entries.entry(rel[..slash_pos].to_string()).or_insert(0) += 1;
        } else if !rel.is_empty() {
            file_entries.push(rel.to_string());
        }
    }
    let mut out = String::new();
    for (dir, count) in &dir_entries {
        out.push_str(&format!("{CYAN}{dir}/{RESET}  {GRAY}({count}){RESET}\r\n"));
    }
    for f in &file_entries {
        let color = if f.ends_with(".toml") {
            YELLOW
        } else if f.ends_with(".json") {
            GREEN
        } else {
            BRIGHT_WHITE
        };
        out.push_str(&format!("{color}{f}{RESET}\r\n"));
    }
    if out.ends_with("\r\n") {
        out.truncate(out.len() - 2);
    }
    out
}

/// Strip ANSI escape sequences so VFS file contents are plain text.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // consume until a letter (final byte)
                for c2 in chars.by_ref() {
                    if c2.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Strip `@target` dispatch hint from a trait path.
/// Returns (clean_path, target) where target is "rest", "relay", "helper", "wasm", or "native".
/// Case-insensitive: `@REST`, `@Rest`, `@rest` all match.
pub fn strip_dispatch_target(path: &str) -> (&str, Option<&'static str>) {
    if let Some(at_pos) = path.rfind('@') {
        let t = &path[at_pos + 1..];
        let target = match t.to_ascii_lowercase().as_str() {
            "wasm" => Some("wasm"),
            "native" => Some("native"),
            "rest" => Some("rest"),
            "relay" => Some("relay"),
            "helper" => Some("helper"),
            _ => None,
        };
        match target {
            Some(t) => (&path[..at_pos], Some(t)),
            None => (path, None),
        }
    } else {
        (path, None)
    }
}

fn exec_call(backend: &dyn CliCallBackend, path: &str, arg_strs: &[String]) -> String {
    let (clean_path, target) = strip_dispatch_target(path);
    let args: Vec<Value> = arg_strs.iter().map(|s| parse_value(s)).collect();

    // Force remote dispatch: skip local backend, emit sentinel with target hint
    if matches!(target, Some("rest") | Some("relay") | Some("helper")) {
        let args_json = serde_json::to_string(&args).unwrap_or_else(|_| "[]".to_string());
        let t = target.unwrap();
        return format!(
            "{GRAY}calling {clean_path} via {t}…{RESET}\r\n\
             {REST_SENTINEL_START}{{\"p\":\"{clean_path}\",\"a\":{args_json},\"t\":\"{t}\"}}{REST_SENTINEL_END}"
        );
    }

    // @wasm: force WASM dispatch — if backend can't handle it, report error locally
    // @native: force native dispatch — if backend can't handle it, report error locally
    let force_local = matches!(target, Some("wasm") | Some("native"));

    match backend.call(clean_path, &args) {
        Ok(result) => {
            let formatted = match &result {
                Value::String(s) => s.clone(),
                other => format_trait_result(clean_path, other)
                    .unwrap_or_else(|| serde_json::to_string_pretty(other).unwrap_or_default()),
            };
            let lines: Vec<&str> = formatted.lines().collect();
            let mut out = String::new();
            if lines.len() > 100 {
                for line in &lines[..80] {
                    out.push_str(line);
                    out.push_str("\r\n");
                }
                out.push_str(&format!(
                    "{GRAY}... ({} more lines){RESET}\r\n",
                    lines.len() - 80
                ));
            } else {
                for line in &lines {
                    out.push_str(line);
                    out.push_str("\r\n");
                }
            }
            out
        }
        Err(e) if e.starts_with("WEBLLM:") => {
            let sentinel_json = &e[7..];
            format!(
                "{GRAY}calling WebLLM…{RESET}\r\n\
                 {WEBLLM_SENTINEL_START}{sentinel_json}{WEBLLM_SENTINEL_END}"
            )
        }
        Err(e) if e.starts_with("REST:") => {
            if force_local {
                // @wasm/@native: do NOT cascade to REST — show local dispatch failure
                return format!(
                    "{RED}Error: {clean_path} not available locally ({t}){RESET}\r\n",
                    t = target.unwrap_or("native")
                );
            }
            let rest_path = &e[5..];
            let args_json = serde_json::to_string(&args).unwrap_or_else(|_| "[]".to_string());
            format!(
                "{GRAY}calling {rest_path} via REST…{RESET}\r\n\
                 {REST_SENTINEL_START}{{\"p\":\"{rest_path}\",\"a\":{args_json}}}{REST_SENTINEL_END}"
            )
        }
        Err(e) => format!("{RED}Error: {}{RESET}\r\n", e),
    }
}

/// Format a trait result via generated *.cli.rs formatters when available.
pub fn format_trait_result(trait_path: &str, result: &Value) -> Option<String> {
    generated_cli_formatters::format_cli(trait_path, result)
}

// ── Formatters ──

fn format_help() -> String {
    let mut s = String::new();
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Commands{RESET}\r\n"));
    s.push_str(&format!(
        "  {GREEN}list{RESET} {GRAY}[namespace]{RESET}         List traits\r\n"
    ));
    s.push_str(&format!(
        "  {GREEN}info{RESET}                       System status\r\n"
    ));
    s.push_str(&format!("  {GREEN}info{RESET} {GRAY}<path>{RESET}              Show trait details + dispatch location\r\n"));
    s.push_str(&format!(
        "  {GREEN}call{RESET} {GRAY}<path> [args...]{RESET}    Call a trait\r\n"
    ));
    s.push_str(&format!("  {GREEN}call{RESET} {GRAY}<path>@rest [args]{RESET}  Force dispatch via rest, relay, helper, wasm, native\r\n"));
    s.push_str(&format!("  {GREEN}call -i{RESET} {GRAY}<path>{RESET}           Interactive mode (prompt each param)\r\n"));
    s.push_str(&format!(
        "  {GREEN}search{RESET} {GRAY}<query>{RESET}           Search by name or description\r\n"
    ));
    s.push_str(&format!(
        "  {GRAY}<path> [args...]{RESET}           Shorthand — call trait directly\r\n"
    ));
    s.push_str(&format!("  {GREEN}chat{RESET} {GRAY}[agent] [model]{RESET}     Enter interactive chat mode (ACP)\r\n"));
    s.push_str(&format!(
        "  {GREEN}version{RESET}                    Show kernel version\r\n"
    ));
    s.push_str(&format!(
        "  {GREEN}clear{RESET}                      Clear terminal\r\n"
    ));
    s.push_str(&format!(
        "  {GREEN}help{RESET}                       Show this help\r\n"
    ));
    s.push_str("\r\n");
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Unix-like tools{RESET}\r\n"));
    s.push_str(&format!(
        "  {GREEN}echo{RESET} {GRAY}[text...]{RESET}           Print text\r\n"
    ));
    s.push_str(&format!("  {GREEN}pwd{RESET}                        Show current path\r\n"));
    s.push_str(&format!("  {GREEN}cat{RESET} {GRAY}[file]{RESET}               Read file or stdin\r\n"));
    s.push_str(&format!("  {GREEN}head{RESET} {GRAY}[-n N] [file]{RESET}       First N lines\r\n"));
    s.push_str(&format!("  {GREEN}tail{RESET} {GRAY}[-n N] [file]{RESET}       Last N lines\r\n"));
    s.push_str(&format!("  {GREEN}grep{RESET} {GRAY}[-inv] <pat> [file]{RESET} Search lines\r\n"));
    s.push_str(&format!("  {GREEN}wc{RESET} {GRAY}[-lwc] [file]{RESET}         Count lines/words/bytes\r\n"));
    s.push_str(&format!("  {GREEN}sed{RESET} {GRAY}[-i] 's/a/b/[gi]' [file]{RESET} Regex substitute (optional in-place)\r\n"));
    s.push_str(&format!("  {GREEN}test{RESET}/{GREEN}[ ... ]{RESET}              Shell-style test expressions\r\n"));
    s.push_str(&format!("  {GREEN}find{RESET} {GRAY}[path] [-name P] [-type f|d] [-mindepth N] [-maxdepth N]{RESET}\r\n"));
    s.push_str(&format!("  {GREEN}curl{RESET} {GRAY}[opts] <url>{RESET}         HTTP calls via sys.call\r\n"));
    s.push_str(&format!("  {GREEN}vi{RESET}/{GREEN}ee{RESET} {GRAY}[-a] <file> [text]{RESET}  Save/append text or preview file\r\n"));
    s.push_str(&format!("  {GRAY}cmd1 && cmd2{RESET}              Run cmd2 only if cmd1 succeeded\r\n"));
    s.push_str(&format!("  {GRAY}cmd1 || cmd2{RESET}              Run cmd2 only if cmd1 failed\r\n"));
    s.push_str("\r\n");
    s.push_str(&format!(
        "{BOLD}{BRIGHT_WHITE}Virtual filesystem{RESET}\r\n"
    ));
    s.push_str(&format!(
        "  {GREEN}ls{RESET}                         List VFS root\r\n"
    ));
    s.push_str(&format!("  {GREEN}ls{RESET} {GRAY}<path>{RESET}              List directory (e.g. ls traits/sys)\r\n"));
    s.push_str(&format!(
        "  {GREEN}cd{RESET} {GRAY}<path>{RESET}              Change current directory\r\n"
    ));
    s.push_str(&format!(
        "  {GREEN}mkdir{RESET} {GRAY}[-p] <dir...>{RESET}       Create VFS directories\r\n"
    ));
    s.push_str(&format!(
        "  {GREEN}cat{RESET} {GRAY}<file>{RESET}              Read a VFS file\r\n"
    ));
    s.push_str(&format!(
        "  {GREEN}write{RESET} {GRAY}<file> <content>{RESET}  Write text to a VFS file\r\n"
    ));
    s.push_str(&format!(
        "  {GREEN}rm{RESET} {GRAY}<path>{RESET}               Delete a VFS file or directory (recursive)\r\n"
    ));
    s.push_str(&format!(
        "  {GREEN}stat{RESET} {GRAY}<file>{RESET}             Show file metadata: size, created, modified timestamps\r\n"
    ));
    s.push_str(&format!(
        "  {GRAY}cmd args > file{RESET}            Redirect output to a VFS file\r\n"
    ));
    s.push_str(&format!(
        "  {GRAY}cmd args >> file{RESET}           Append output to a VFS file\r\n"
    ));
    s.push_str(&format!(
        "  {GRAY}cmd < file{RESET}                 Read VFS file as stdin\r\n"
    ));
    s.push_str(&format!(
        "  {GRAY}cmd1 | cmd2{RESET}                Pipe stdout to next command\r\n"
    ));
    s.push_str(&format!(
        "  {GRAY}cmd @file{RESET}                  Pass VFS file contents as an argument\r\n"
    ));
    s.push_str("\r\n");
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Shortcuts{RESET}\r\n"));
    s.push_str(&format!(
        "  {CYAN}Tab{RESET}          Auto-complete commands and trait paths\r\n"
    ));
    s.push_str(&format!(
        "  {CYAN}↑ / ↓{RESET}        Navigate command history\r\n"
    ));
    s.push_str(&format!("  {CYAN}Ctrl+L{RESET}       Clear terminal\r\n"));
    s.push_str(&format!(
        "  {CYAN}Ctrl+C{RESET}       Cancel current line\r\n"
    ));
    s.push_str(&format!(
        "  {CYAN}Ctrl+U{RESET}       Clear entire line\r\n"
    ));
    s.push_str(&format!(
        "  {CYAN}Ctrl+W{RESET}       Delete word backward\r\n"
    ));
    s.push_str(&format!(
        "  {CYAN}Ctrl+A/E{RESET}     Jump to start/end of line\r\n"
    ));
    s.push_str(&format!(
        "  {CYAN}Opt+←/→{RESET}      Jump word-by-word (also Alt+B / Alt+F)\r\n"
    ));
    s.push_str("\r\n");
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Interactive mode{RESET}\r\n"));
    s.push_str(&format!(
        "  {CYAN}↑ / ↓{RESET}        Cycle through parameter history\r\n"
    ));
    s.push_str(&format!(
        "  {CYAN}Tab{RESET}          Cycle through completions\r\n"
    ));
    s.push_str(&format!(
        "  {CYAN}Ctrl+C{RESET}       Abort interactive mode\r\n"
    ));
    s.push_str("\r\n");
    s.push_str(&format!("{BOLD}{BRIGHT_WHITE}Examples{RESET}\r\n"));
    s.push_str(&format!(
        "  {GRAY}call sys.checksum hash \"hello world\"{RESET}\r\n"
    ));
    s.push_str(&format!("  {GRAY}call -i sys.checksum{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}sys.version{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}info sys.list{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}list sys{RESET}\r\n"));
    s.push_str(&format!("  {GRAY}search checksum{RESET}\r\n"));
    s
}

fn format_list(backend: &dyn CliCallBackend, namespace: Option<&str>) -> String {
    let all = backend.list_all();
    let filtered: Vec<&Value> = if let Some(ns) = namespace {
        all.iter()
            .filter(|t| {
                t.get("path")
                    .and_then(|p| p.as_str())
                    .map_or(false, |p| p.starts_with(ns))
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
            "{BOLD}{BRIGHT_WHITE}{}{RESET} {GRAY}({}){RESET}\r\n",
            ns,
            traits.len()
        ));
        for t in traits {
            let path = t.get("path").and_then(|p| p.as_str()).unwrap_or("");
            let name = path.rsplit('.').next().unwrap_or(path);
            let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
            let wasm = t
                .get("wasm_callable")
                .and_then(|w| w.as_bool())
                .unwrap_or(false);
            let badge = if wasm {
                format!("{GREEN}[WASM]{RESET}")
            } else {
                format!("{YELLOW}[REST]{RESET}")
            };
            out.push_str(&format!(
                "  {} {BLUE}{}{RESET}  {GRAY}{}{RESET}\r\n",
                badge, name, desc
            ));
        }
    }
    out.push_str(&format!("{GRAY}{} traits{RESET}", filtered.len()));
    out
}

fn format_system_status(backend: &dyn CliCallBackend) -> String {
    // Call sys.info with no args to get system status
    match backend.call("sys.info", &[]) {
        Ok(info) => format_trait_result("sys.info", &info)
            .unwrap_or_else(|| serde_json::to_string_pretty(&info).unwrap_or_default()),
        Err(e) if e.starts_with("REST:") => {
            // WASM can't dispatch sys.info locally — delegate to SDK cascade
            format!(
                "{GRAY}loading system status…{RESET}\r\n\
                 {REST_SENTINEL_START}{{\"p\":\"sys.info\",\"a\":[]}}{REST_SENTINEL_END}"
            )
        }
        Err(_) => {
            // Fallback: basic info from backend
            let paths = backend.all_paths();
            let mut out = String::new();
            out.push_str(&format!("{BOLD}{BRIGHT_WHITE}System Status{RESET}\r\n\r\n"));
            out.push_str(&format!("{BOLD}Traits{RESET}\r\n"));
            out.push_str(&format!(
                "  {GRAY}Total:{RESET}   {CYAN}{}{RESET}\r\n",
                paths.len()
            ));
            out.push_str(&format!(
                "  {GRAY}Version:{RESET} {CYAN}{}{RESET}\r\n",
                backend.version()
            ));
            out
        }
    }
}

/// Format a REST response for display in the WASM terminal.
/// Returns Some(formatted) if a formatter exists, None to fall back to JSON.
/// When result is null (REST failed), returns a local fallback if available.
pub fn format_rest_result(trait_path: &str, args: &[Value], result: &Value) -> Option<String> {
    if result.is_null() {
        match trait_path {
            "sys.info"
                if args.is_empty()
                    || args
                        .first()
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .is_empty() =>
            {
                Some(format_basic_status())
            }
            _ => None,
        }
    } else {
        format_trait_result(trait_path, result)
    }
}

/// Basic system status from WASM-local data (no server needed).
fn format_basic_status() -> String {
    let count = kernel_logic::platform::registry_count();
    let version_info =
        kernel_logic::platform::dispatch("sys.version", &[Value::String("system".into())])
            .unwrap_or_default();
    let version = version_info
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let mut out = String::new();
    out.push_str(&format!("{BOLD}{BRIGHT_WHITE}System Status{RESET}\n\n"));
    out.push_str(&format!("{BOLD}System{RESET}\n"));
    out.push_str(&format!(
        "  {GRAY}Runtime:{RESET} {CYAN}WASM (browser){RESET}\n"
    ));
    out.push_str(&format!("  {GRAY}Build:{RESET}   {CYAN}{version}{RESET}\n"));
    out.push_str(&format!("\n{BOLD}Traits{RESET}\n"));
    out.push_str(&format!("  {GRAY}Total:{RESET}   {CYAN}{count}{RESET}\n"));
    out.push_str(&format!(
        "\n{GRAY}Connect a helper for full system status{RESET}\n"
    ));
    out
}

fn format_info(backend: &dyn CliCallBackend, path: &str) -> String {
    let info = match backend.get_info(path) {
        Some(v) => v,
        None => return format!("{RED}Trait \"{}\" not found{RESET}", path),
    };

    if let Some(formatted) = format_trait_result("sys.info", &info) {
        return formatted;
    }

    serde_json::to_string_pretty(&info).unwrap_or_default()
}

fn format_search(backend: &dyn CliCallBackend, query: &str) -> String {
    let results = backend.search(query);
    if results.is_empty() {
        return format!("{YELLOW}No matches for \"{}\"{RESET}", query);
    }
    let mut out = String::new();
    for t in &results {
        let path = t.get("path").and_then(|p| p.as_str()).unwrap_or("");
        let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
        let wasm = t
            .get("wasm_callable")
            .and_then(|w| w.as_bool())
            .unwrap_or(false);
        let badge = if wasm {
            format!("{GREEN}[WASM]{RESET}")
        } else {
            format!("{YELLOW}[REST]{RESET}")
        };
        out.push_str(&format!(
            "{} {BLUE}{}{RESET}  {GRAY}{}{RESET}\r\n",
            badge, path, desc
        ));
    }
    out.push_str(&format!("{GRAY}{} matches{RESET}", results.len()));
    out
}

// ── Parsing ──

/// Parse a command line string into parts, respecting quoted strings.
pub fn parse_command(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for ch in line.chars() {
        match (ch, in_quote) {
            ('"', None) | ('\'', None) => in_quote = Some(ch),
            (c, Some(q)) if c == q => in_quote = None,
            (' ', None) => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
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

fn shell_builtin_commands() -> Vec<String> {
    [
        "help", "h", "?", "list", "info", "i", "call", "c", "search", "s", "version", "v", "clear", "cls",
        "echo", "pwd", "true", "false", "cat", "head", "tail", "grep", "wc", "sed", "test", "[", "find", "curl",
        "vi", "ee", "stat", "mkdir", "write", "tee", "rm", "ls", "cd", "chat",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn prev_char_boundary(s: &str, mut pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    pos -= 1;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn cursor_word_left(s: &str, pos: usize) -> usize {
    let mut cur = pos.min(s.len());
    while cur > 0 && !s.is_char_boundary(cur) {
        cur -= 1;
    }

    while cur > 0 {
        let p = prev_char_boundary(s, cur);
        let ch = s[p..].chars().next().unwrap_or(' ');
        if is_word_char(ch) {
            break;
        }
        cur = p;
    }

    while cur > 0 {
        let p = prev_char_boundary(s, cur);
        let ch = s[p..].chars().next().unwrap_or(' ');
        if !is_word_char(ch) {
            break;
        }
        cur = p;
    }

    cur
}

fn cursor_word_right(s: &str, pos: usize) -> usize {
    let mut cur = pos.min(s.len());
    while cur < s.len() && !s.is_char_boundary(cur) {
        cur += 1;
    }

    while cur < s.len() {
        let ch = s[cur..].chars().next().unwrap_or(' ');
        if is_word_char(ch) {
            break;
        }
        cur += ch.len_utf8();
    }

    while cur < s.len() {
        let ch = s[cur..].chars().next().unwrap_or(' ');
        if !is_word_char(ch) {
            break;
        }
        cur += ch.len_utf8();
    }

    cur
}

/// Get interactive mode parameter info for a trait.
pub fn interactive_params(path: &str, backend: &dyn CliCallBackend) -> Option<Value> {
    backend
        .get_info(path)
        .and_then(|info| info.get("params").cloned())
}

// ── Helpers ──

fn resolve_path(path: &str, backend: &dyn CliCallBackend) -> String {
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
    let req = if p.required {
        format!("{RED}*{RESET}")
    } else {
        " ".to_string()
    };
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

fn build_history_completions(history: &[String]) -> Vec<String> {
    let mut completions: Vec<String> = history.iter().rev().cloned().collect();
    let mut seen = std::collections::HashSet::new();
    completions.retain(|v| seen.insert(v.clone()));
    completions
}

fn build_tab_completions(default_val: &str, example_vals: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut completions = Vec::new();
    // Default first
    if !default_val.is_empty() && seen.insert(default_val.to_string()) {
        completions.push(default_val.to_string());
    }
    // Then all example values
    for v in example_vals {
        if !v.is_empty() && seen.insert(v.clone()) {
            completions.push(v.clone());
        }
    }
    completions
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackend;

    impl CliCallBackend for MockBackend {
        fn call(&self, _path: &str, _args: &[Value]) -> Result<Value, String> {
            Err("not implemented in tests".to_string())
        }
        fn list_all(&self) -> Vec<Value> {
            vec![]
        }
        fn get_info(&self, _path: &str) -> Option<Value> {
            None
        }
        fn search(&self, _query: &str) -> Vec<Value> {
            vec![]
        }
        fn all_paths(&self) -> Vec<String> {
            vec![]
        }
        fn version(&self) -> String {
            "test".to_string()
        }
    }

    impl CliHistoryBackend for MockBackend {}
    impl CliExamplesBackend for MockBackend {}

    fn test_vfs() -> RefCell<Box<dyn Vfs>> {
        RefCell::new(Box::new(MemVfs::default()))
    }

    #[test]
    fn pwd_and_cd_follow_cwd_state() {
        let backend = MockBackend;
        let shell = DefaultShell;
        let vfs = test_vfs();
        let mut cwd = "/".to_string();

        let out = exec_line("pwd", &backend, &shell, &vfs, &mut cwd);
        assert_eq!(strip_ansi(&out).trim(), "/");

        let out = exec_line("mkdir -p a/b", &backend, &shell, &vfs, &mut cwd);
        assert!(strip_ansi(&out).contains("created"));
        assert!(vfs.borrow().is_dir("/a"));
        assert!(vfs.borrow().is_dir("/a/b"));

        let out = exec_line("cd a/b", &backend, &shell, &vfs, &mut cwd);
        assert!(strip_ansi(&out).trim().is_empty());
        assert_eq!(cwd, "/a/b");

        let out = exec_line("pwd", &backend, &shell, &vfs, &mut cwd);
        assert_eq!(strip_ansi(&out).trim(), "/a/b");

        let out = exec_line("cd ..", &backend, &shell, &vfs, &mut cwd);
        assert!(strip_ansi(&out).trim().is_empty());
        assert_eq!(cwd, "/a");
    }

    #[test]
    fn mkdir_without_p_requires_parent_directory() {
        let backend = MockBackend;
        let shell = DefaultShell;
        let vfs = test_vfs();
        let mut cwd = "/".to_string();

        let out = exec_line("mkdir one/two", &backend, &shell, &vfs, &mut cwd);
        let plain = strip_ansi(&out);
        assert!(plain.contains("No such file or directory"));
        assert!(!vfs.borrow().is_dir("/one/two"));
    }

    #[test]
    fn logical_and_or_short_circuit() {
        let backend = MockBackend;
        let shell = DefaultShell;
        let vfs = test_vfs();
        let mut cwd = "/".to_string();

        let out = exec_line("false && write blocked.txt nope", &backend, &shell, &vfs, &mut cwd);
        assert!(strip_ansi(&out).trim().is_empty());
        assert!(vfs.borrow().read("/blocked.txt").is_none());

        let out = exec_line("false || write allowed.txt yup", &backend, &shell, &vfs, &mut cwd);
        assert!(strip_ansi(&out).contains("wrote"));
        assert_eq!(vfs.borrow().read("/allowed.txt").as_deref(), Some("yup"));
    }

    #[test]
    fn vi_and_ee_can_save_and_append() {
        let backend = MockBackend;
        let shell = DefaultShell;
        let vfs = test_vfs();
        let mut cwd = "/".to_string();

        let out = exec_line("echo first | vi notes.txt", &backend, &shell, &vfs, &mut cwd);
        assert!(strip_ansi(&out).contains("saved"));
        assert_eq!(vfs.borrow().read("/notes.txt").as_deref(), Some("first"));

        let out = exec_line("ee -a notes.txt second", &backend, &shell, &vfs, &mut cwd);
        assert!(strip_ansi(&out).contains("appended"));
        let body = vfs.borrow().read("/notes.txt").unwrap_or_default();
        assert!(body.starts_with("first"));
        assert!(body.contains("second"));
    }

    #[test]
    fn find_supports_type_and_mindepth() {
        let backend = MockBackend;
        let shell = DefaultShell;
        let vfs = test_vfs();
        let mut cwd = "/".to_string();

        let _ = exec_line("mkdir -p a/b", &backend, &shell, &vfs, &mut cwd);
        let _ = exec_line("write a/root.txt r", &backend, &shell, &vfs, &mut cwd);
        let _ = exec_line("write a/b/leaf.txt l", &backend, &shell, &vfs, &mut cwd);

        let out = exec_line("find a -type d -mindepth 1", &backend, &shell, &vfs, &mut cwd);
        let plain = strip_ansi(&out);
        let lines: Vec<&str> = plain.lines().collect();
        assert!(lines.iter().any(|l| *l == "/a/b"));
        assert!(!lines.iter().any(|l| *l == "/a"));

        let out = exec_line("find a -type f -maxdepth 1", &backend, &shell, &vfs, &mut cwd);
        let plain = strip_ansi(&out);
        assert!(plain.contains("/a/root.txt"));
        assert!(!plain.contains("/a/b/leaf.txt"));
    }
}

// ── Native dispatch entry point ──

pub fn cli_dispatch(_args: &[Value]) -> Value {
    Value::String("kernel.cli: use CliSession.feed() with a CliBackend".to_string())
}
