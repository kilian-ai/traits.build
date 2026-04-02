use serde_json::{json, Value};
use std::process::Command;

/// skills.spotify.vol — Set Spotify volume (0–100).
pub fn vol(args: &[Value]) -> Value {
    let level = args
        .first()
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(-1);

    if level < 0 || level > 100 {
        return json!({"ok": false, "error": "Volume must be 0–100"});
    }

    let script = format!(
        "tell application \"Spotify\" to set sound volume to {}",
        level
    );
    let result = run_osascript(&script);
    if result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        json!({"ok": true, "status": format!("Volume set to {}", level)})
    } else { result }
}

fn run_osascript(script: &str) -> Value {
    match Command::new("osascript").args(["-e", script]).output() {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if out.status.success() {
                json!({"ok": true})
            } else {
                json!({"ok": false, "error": stderr})
            }
        }
        Err(e) => json!({"ok": false, "error": format!("osascript failed: {}", e)}),
    }
}
