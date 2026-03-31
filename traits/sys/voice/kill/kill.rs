use serde_json::{json, Value};
use std::process::Command;

/// sys.voice.kill — find and kill orphaned voice agent processes.
///
/// Scans for orphaned `rec` and `play` (sox) processes that match the voice
/// audio format signature (24kHz, 16-bit, signed, mono raw PCM) and kills them.
/// Also sets VOICE_RUNNING to false if it was still active.
///
/// Returns a report of processes found and killed.
pub fn voice_kill(_args: &[Value]) -> Value {
    // Stop the voice loop flag if active
    crate::dispatcher::compiled::voice::VOICE_RUNNING
        .store(false, std::sync::atomic::Ordering::SeqCst);

    let mut killed = Vec::new();
    let mut errors = Vec::new();

    // Find orphaned sox processes: rec and play with our audio signature
    let output = match Command::new("ps")
        .args(["-eo", "pid,args"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => return json!({"ok": false, "error": format!("Failed to list processes: {e}")}),
    };

    for line in output.lines() {
        let trimmed = line.trim();
        // Match: rec/play with our exact audio format signature
        let is_voice_proc = (trimmed.contains("rec ") || trimmed.contains("play "))
            && trimmed.contains("-r 24000")
            && trimmed.contains("-b 16")
            && trimmed.contains("signed");

        if !is_voice_proc {
            continue;
        }

        // Extract PID (first token)
        let pid_str = match trimmed.split_whitespace().next() {
            Some(p) => p,
            None => continue,
        };
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Don't kill ourselves
        let my_pid = std::process::id();
        if pid == my_pid {
            continue;
        }

        // Kill the process
        let cmd = trimmed[pid_str.len()..].trim().to_string();
        match Command::new("kill").arg(pid.to_string()).output() {
            Ok(o) if o.status.success() => {
                killed.push(json!({"pid": pid, "command": cmd}));
            }
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                // Process already gone is not an error
                if err.contains("No such process") {
                    killed.push(json!({"pid": pid, "command": cmd, "note": "already exited"}));
                } else {
                    errors.push(json!({"pid": pid, "error": err}));
                }
            }
            Err(e) => {
                errors.push(json!({"pid": pid, "error": format!("{e}")}));
            }
        }
    }

    let mut result = json!({
        "ok": true,
        "killed": killed.len(),
        "processes": killed,
    });
    if !errors.is_empty() {
        result["errors"] = json!(errors);
    }
    result
}
