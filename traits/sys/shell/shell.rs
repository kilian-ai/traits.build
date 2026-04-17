use serde_json::{json, Value};
#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;

/// sys.shell — execute a shell command and return its output.
///
/// Runs the command via `sh -c` so pipes, redirects, and shell builtins work.
/// Returns stdout, stderr, and exit code. Capped at 60 seconds timeout.
pub fn shell(args: &[Value]) -> Value {
    #[cfg(target_arch = "wasm32")]
    {
        return shell_wasm(args);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        return shell_native(args);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn shell_native(args: &[Value]) -> Value {
    let command = match args.first().and_then(|v| v.as_str()) {
        Some(c) if !c.is_empty() => c,
        _ => return json!({"ok": false, "error": "Missing required parameter: command"}),
    };

    let cwd = args.get(1).and_then(|v| v.as_str()).filter(|s| !s.is_empty());
    let timeout_secs = args
        .get(2)
        .and_then(|v| v.as_u64())
        .unwrap_or(60)
        .min(300); // hard cap at 5 minutes

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    // Use wait_with_output with a thread-based timeout
    let result = std::thread::scope(|s| {
        let handle = s.spawn(|| cmd.output());
        // Wait for the thread to finish within the timeout
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            if handle.is_finished() {
                return handle.join().ok();
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    match result {
        Some(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = output.status.code().unwrap_or(-1);

            // Truncate to avoid blowing up JSON
            let max = 8000;
            let stdout_str = if stdout.len() > max {
                format!("{}…(truncated, {} bytes total)", &stdout[..max], stdout.len())
            } else {
                stdout.to_string()
            };
            let stderr_str = if stderr.len() > max {
                format!("{}…(truncated, {} bytes total)", &stderr[..max], stderr.len())
            } else {
                stderr.to_string()
            };

            json!({
                "ok": code == 0,
                "exit_code": code,
                "stdout": stdout_str.trim_end(),
                "stderr": stderr_str.trim_end(),
            })
        }
        Some(Err(e)) => json!({"ok": false, "error": format!("Failed to execute: {}", e)}),
        None => json!({"ok": false, "error": format!("Command timed out after {}s", timeout_secs)}),
    }
}

#[cfg(target_arch = "wasm32")]
fn shell_wasm(args: &[Value]) -> Value {
    let command = match args.first().and_then(|v| v.as_str()) {
        Some(c) if !c.is_empty() => c,
        _ => return json!({"ok": false, "error": "Missing required parameter: command"}),
    };

    let cwd = args.get(1).and_then(|v| v.as_str()).filter(|s| !s.is_empty());

    // In WASM we execute through the shared CLI session engine.
    // This keeps behavior aligned with the terminal shell builtins (ls/cd/cat/find/...)
    // and works without native process spawning.
    if let Some(dir) = cwd {
        let _ = crate::cli_input(&format!("cd {}\r", dir));
    }
    let raw = crate::cli_input(&format!("{}\r", command));
    let cleaned = clean_cli_output(&raw);

    let looks_error = cleaned.contains("Error:")
        || cleaned.contains("Unknown command:")
        || cleaned.contains("no such ")
        || cleaned.contains("Usage:")
        || cleaned.contains("failed");

    json!({
        "ok": !looks_error,
        "exit_code": if looks_error { 1 } else { 0 },
        "stdout": cleaned,
        "stderr": "",
        "note": "WASM shell executed via kernel.cli session engine",
    })
}

#[cfg(target_arch = "wasm32")]
fn clean_cli_output(raw: &str) -> String {
    let mut out = String::new();
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            i += 1;
            if i < bytes.len() && bytes[i] == b'[' {
                i += 1;
                while i < bytes.len() {
                    let b = bytes[i];
                    if (0x40..=0x7e).contains(&b) {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }

    let cleaned_lines: Vec<String> = out
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.trim().is_empty())
        .filter(|l| l.trim_start() != "traits")
        .filter(|l| !l.trim_start().starts_with("thinking…"))
        .collect();

    cleaned_lines.join("\n")
}
