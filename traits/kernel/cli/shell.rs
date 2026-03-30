// ── Shell abstraction layer ──
//
// This module defines the `Shell` trait — the single seam that separates
// line-parsing from the rest of the CLI kernel.  Today's implementation
// (`DefaultShell`) uses `shell-words` for POSIX-correct word splitting and
// handles `>` / `>>` redirection.
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
/// `pipe_next` is reserved for future pipeline support — today it is always
/// `None`.  Callers must handle it gracefully (ignore or warn "unsupported").
#[derive(Debug)]
pub struct ShellCommand {
    /// Argv-style token list after word-splitting and quote removal.
    pub args: Vec<String>,
    /// Optional output-redirection target.
    pub redirect: Option<Redirect>,
    /// Future: piped next command.
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
/// This is intentionally minimal — it does not evaluate variables, glob-
/// expand paths, or support pipelines.  Those are all left to future
/// implementations of the `Shell` trait.
pub struct DefaultShell;

impl Shell for DefaultShell {
    fn parse(&self, line: &str) -> ShellCommand {
        // POSIX word split (handles "", '', \ escaping)
        let mut args = match shell_words::split(line) {
            Ok(parts) => parts,
            Err(_) => {
                // Unclosed quote etc. — fall back to whitespace split
                line.split_whitespace().map(String::from).collect()
            }
        };

        let redirect = extract_redirect(&mut args);

        ShellCommand {
            args,
            redirect,
            pipe_next: None,
        }
    }
}

// ── Redirect extraction ──────────────────────────────────────────────────────

/// Scan `args` for the first `>` / `>>` token (with or without a space before
/// the filename) and remove it from `args`, returning the `Redirect`.
///
/// Handles all four forms:
///   cmd arg >> file        (tokens: ["cmd","arg",">>","file"])
///   cmd arg > file         (tokens: ["cmd","arg",">","file"])
///   cmd arg >>file         (tokens: ["cmd","arg",">>file"])
///   cmd arg >file          (tokens: ["cmd","arg",">file"])
fn extract_redirect(args: &mut Vec<String>) -> Option<Redirect> {
    let mut i = 0;
    while i < args.len() {
        // ">> file" or "> file" as separate tokens
        if (args[i] == ">>" || args[i] == ">") && i + 1 < args.len() {
            let append = args[i] == ">>";
            let file = args[i + 1].clone();
            args.drain(i..=i + 1);
            return Some(Redirect { file, append });
        }
        // ">>file" or ">file" attached
        if args[i].starts_with(">>") && args[i].len() > 2 {
            let file = args[i][2..].to_string();
            args.remove(i);
            return Some(Redirect { file, append: true });
        }
        if args[i].starts_with('>') && args[i].len() > 1 {
            let file = args[i][1..].to_string();
            args.remove(i);
            return Some(Redirect {
                file,
                append: false,
            });
        }
        i += 1;
    }
    None
}
