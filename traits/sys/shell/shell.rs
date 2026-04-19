use serde_json::{json, Value};
#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;

#[cfg(target_arch = "wasm32")]
use crate::wasm_traits::cli::{CliCallBackend, CliExamplesBackend, CliHistoryBackend, CliSession};
#[cfg(target_arch = "wasm32")]
use js_sys::Date;
#[cfg(target_arch = "wasm32")]
use web_sys;

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

    if command.contains('\n') || command.contains('\r') {
        return json!({
            "ok": false,
            "error": "WASM sys.shell does not support raw multi-line commands. Use a single-line write/tee/ee command with literal \\n escapes, then verify with cat."
        });
    }

    let cwd = args.get(1).and_then(|v| v.as_str()).filter(|s| !s.is_empty());

    // Use a fresh CLI session per call to avoid stale cwd leaking between tool
    // invocations. The VFS is still shared via persisted traits.pvfs state, so
    // files remain visible in the primary terminal session.
    let mut session = CliSession::new();
    let backend = WasmShellBackend;

    if let Some(json) = shell_ls_get("traits.pvfs") {
        session.vfs_load(&json);
    }

    if let Some(dir) = cwd {
        let _ = session.feed(&format!("cd {}\r", dir), &backend);
    }
    let raw = session.feed(&format!("{}\r", command), &backend);

    let dump = session.vfs_dump();
    if !dump.is_empty() {
        shell_ls_set("traits.pvfs", &dump);
        shell_ls_set("traits.pvfs.ts", &format!("{:.0}", Date::now()));
    }

    if let Some(payload) = extract_js_payload(&raw) {
        return execute_js_payload(&payload);
    }

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
fn extract_js_payload(raw: &str) -> Option<Value> {
    let start = raw.find("[JS]")? + 4;
    let end = raw[start..].find("[/JS]")? + start;
    serde_json::from_str::<Value>(&raw[start..end]).ok()
}

#[cfg(target_arch = "wasm32")]
fn execute_js_payload(payload: &Value) -> Value {
    let code = payload
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let input = payload.get("input").cloned().unwrap_or_else(|| json!({}));

    let result = match kernel_logic::platform::dispatch("sys.js", &[json!(code), input]) {
        Some(v) => v,
        None => {
            return json!({
                "ok": false,
                "exit_code": 1,
                "stdout": "",
                "stderr": "sys.js trait not available",
                "note": "WASM shell JS dispatch bridge failed"
            })
        }
    };

    let stdout_lines: Vec<String> = result
        .get("stdout")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let stderr_lines: Vec<String> = result
        .get("stderr")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let mut stdout = stdout_lines.join("\n");
    if stdout.trim().is_empty() {
        if let Some(value) = result.get("result") {
            if !value.is_null() {
                stdout = if let Some(s) = value.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
                };
            }
        }
    }

    let stderr = stderr_lines.join("\n");
    let need_input = result
        .get("need_input")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if need_input {
        return json!({
            "ok": false,
            "exit_code": 1,
            "stdout": stdout,
            "stderr": stderr,
            "error": format!(
                "interactive JS input required: {}",
                result.get("prompt").and_then(|v| v.as_str()).unwrap_or("js stdin")
            ),
            "note": "WASM shell JS execution requires terminal interaction when stdin is insufficient"
        });
    }

    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let error = result
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    json!({
        "ok": ok,
        "exit_code": if ok { 0 } else { 1 },
        "stdout": stdout,
        "stderr": if stderr.is_empty() { error } else { &stderr },
        "note": "WASM shell executed JS via sys.js bridge",
    })
}

#[cfg(target_arch = "wasm32")]
fn shell_ls_get(key: &str) -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item(key).ok()?
}

#[cfg(target_arch = "wasm32")]
fn shell_ls_set(key: &str, value: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.set_item(key, value);
    }
}

#[cfg(target_arch = "wasm32")]
struct WasmShellBackend;

#[cfg(target_arch = "wasm32")]
impl CliCallBackend for WasmShellBackend {
    fn call(&self, path: &str, args: &[Value]) -> Result<Value, String> {
        kernel_logic::platform::dispatch(path, args)
            .ok_or_else(|| format!("Trait not found: {}", path))
    }

    fn list_all(&self) -> Vec<Value> {
        kernel_logic::platform::registry_all()
    }

    fn get_info(&self, path: &str) -> Option<Value> {
        kernel_logic::platform::registry_detail(path)
    }

    fn search(&self, query: &str) -> Vec<Value> {
        let q = query.to_lowercase();
        kernel_logic::platform::registry_all()
            .into_iter()
            .filter(|entry| {
                let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                let desc = entry
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                path.contains(&q) || desc.contains(&q)
            })
            .collect()
    }

    fn all_paths(&self) -> Vec<String> {
        kernel_logic::platform::registry_all()
            .into_iter()
            .filter_map(|entry| entry.get("path").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect()
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

#[cfg(target_arch = "wasm32")]
impl CliHistoryBackend for WasmShellBackend {}

#[cfg(target_arch = "wasm32")]
impl CliExamplesBackend for WasmShellBackend {}

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
