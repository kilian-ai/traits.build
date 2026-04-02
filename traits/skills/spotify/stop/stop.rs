use serde_json::{json, Value};
use std::process::Command;

/// skills.spotify.stop — Stop Spotify playback (pause + rewind to start).
pub fn stop(_args: &[Value]) -> Value {
    let result = run_osascript("tell application \"Spotify\"\npause\nset player position to 0\nend tell");
    if result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        json!({"ok": true, "status": "Playback stopped"})
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
