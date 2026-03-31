use serde_json::{json, Value};
use std::process::Command;

/// skills.spotify.next — Skip to the next track.
pub fn next(_args: &[Value]) -> Value {
    run_osascript("tell application \"Spotify\" to next track")
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
