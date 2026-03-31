use serde_json::{json, Value};
use std::process::Command;

/// sys.shell — execute a shell command and return its output.
///
/// Runs the command via `sh -c` so pipes, redirects, and shell builtins work.
/// Returns stdout, stderr, and exit code. Capped at 60 seconds timeout.
pub fn shell(args: &[Value]) -> Value {
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
