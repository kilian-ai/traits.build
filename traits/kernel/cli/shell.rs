// ── Shell abstraction layer ──
//
// This module defines the `Shell` trait — the single seam that separates
// line-parsing from the rest of the CLI kernel.  Today's implementation
// (`DefaultShell`) uses `shell-words` for POSIX-correct word splitting and
// handles redirection and pipelines.
//
// When a full shell interpreter is available (e.g. a POSIX engine compiled
// to WASM), swap it in at the `CliSession` level:
//
//   session.set_shell(Box::new(MyFullShell::new()));
//
// `exec_line` never needs to change — it receives `Arc<dyn Shell>` and only
// calls `shell.parse(line)`.

/// A parsed command ready for execution.
///
/// `pipe_next` holds pipeline continuation when present.
#[derive(Debug)]
pub struct ShellCommand {
    /// Argv-style token list after word-splitting and quote removal.
    pub args: Vec<String>,
    /// Optional input-redirection source (`< file`).
    pub stdin_from: Option<String>,
    /// Optional output-redirection target.
    pub redirect: Option<Redirect>,
    /// Piped next command (`cmd1 | cmd2`).
    pub pipe_next: Option<Box<ShellCommand>>,
}

/// Output redirection descriptor (`> file` or `>> file`).
#[derive(Debug, Clone)]
pub struct Redirect {
    pub file: String,
    pub append: bool,
}

impl ShellCommand {
    /// Convenience: first token (the command name), or empty string.
    pub fn cmd(&self) -> &str {
        self.args.first().map(String::as_str).unwrap_or("")
    }

    /// Tokens after the command name.
    pub fn rest(&self) -> &[String] {
        if self.args.is_empty() {
            &[]
        } else {
            &self.args[1..]
        }
    }
}

// ── Shell trait ──────────────────────────────────────────────────────────────

/// The shell abstraction boundary.
///
/// Implementations must be object-safe so they can be stored as
/// `Box<dyn Shell>` in `CliSession`.
pub trait Shell {
    /// Parse a raw command line into a `ShellCommand`.
    ///
    /// Parsing must be infallible — on any error return a `ShellCommand` with
    /// `args` containing the un-split raw line so the user sees *something*.
    fn parse(&self, line: &str) -> ShellCommand;
}

// ── DefaultShell — shell-words + redirect detection ─────────────────────────

/// Default implementation: POSIX word-splitting via `shell-words` plus
/// recognition of `>` / `>>` redirection operators.
///
/// This is intentionally minimal — it does not evaluate variables or glob-
/// expand paths. Logical chaining (`&&`, `||`) is handled in `exec_line`.
pub struct DefaultShell;

impl Shell for DefaultShell {
    fn parse(&self, line: &str) -> ShellCommand {
        // POSIX word split (handles "", '', \ escaping)
        let args = match shell_words::split(line) {
            Ok(parts) => parts,
            Err(_) => {
                // Unclosed quote etc. — fall back to whitespace split
                line.split_whitespace().map(String::from).collect()
            }
        };

        parse_pipeline(&args)
    }
}

// ── Redirect extraction ──────────────────────────────────────────────────────

fn parse_pipeline(tokens: &[String]) -> ShellCommand {
    if let Some(pipe_idx) = tokens.iter().position(|t| t == "|") {
        let left = parse_segment(&tokens[..pipe_idx]);
        let right = parse_pipeline(&tokens[pipe_idx + 1..]);
        ShellCommand {
            args: left.args,
            stdin_from: left.stdin_from,
            redirect: left.redirect,
            pipe_next: Some(Box::new(right)),
        }
    } else {
        parse_segment(tokens)
    }
}

fn parse_segment(tokens: &[String]) -> ShellCommand {
    let mut args = tokens.to_vec();
    let (stdin_from, redirect) = extract_redirects(&mut args);
    ShellCommand {
        args,
        stdin_from,
        redirect,
        pipe_next: None,
    }
}

/// Scan `args` for the first `>` / `>>` token (with or without a space before
/// the filename) and remove it from `args`, returning the `Redirect`.
///
/// Handles all four forms:
///   cmd arg >> file        (tokens: ["cmd","arg",">>","file"])
///   cmd arg > file         (tokens: ["cmd","arg",">","file"])
///   cmd arg >>file         (tokens: ["cmd","arg",">>file"])
///   cmd arg >file          (tokens: ["cmd","arg",">file"])
fn extract_redirects(args: &mut Vec<String>) -> (Option<String>, Option<Redirect>) {
    let mut stdin_from: Option<String> = None;
    let mut redirect: Option<Redirect> = None;
    let mut i = 0;
    while i < args.len() {
        // ">> file" or "> file" as separate tokens
        if (args[i] == ">>" || args[i] == ">") && i + 1 < args.len() {
            let append = args[i] == ">>";
            let file = args[i + 1].clone();
            args.drain(i..=i + 1);
            redirect = Some(Redirect { file, append });
            continue;
        }
        // "< file" as separate tokens
        if args[i] == "<" && i + 1 < args.len() {
            let file = args[i + 1].clone();
            args.drain(i..=i + 1);
            stdin_from = Some(file);
            continue;
        }
        // ">>file" or ">file" attached
        if args[i].starts_with(">>") && args[i].len() > 2 {
            let file = args[i][2..].to_string();
            args.remove(i);
            redirect = Some(Redirect { file, append: true });
            continue;
        }
        if args[i].starts_with('>') && args[i].len() > 1 {
            let file = args[i][1..].to_string();
            args.remove(i);
            redirect = Some(Redirect {
                file,
                append: false,
            });
            continue;
        }
        // "<file" attached
        if args[i].starts_with('<') && args[i].len() > 1 {
            let file = args[i][1..].to_string();
            args.remove(i);
            stdin_from = Some(file);
            continue;
        }
        i += 1;
    }
    (stdin_from, redirect)
}
