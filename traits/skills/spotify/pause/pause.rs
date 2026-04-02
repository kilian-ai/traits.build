use serde_json::{json, Value};
use std::process::Command;

/// skills.spotify.pause — Pause Spotify playback.
pub fn pause(_args: &[Value]) -> Value {
    let result = run_osascript("tell application \"Spotify\" to pause");
    if !result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return result;
    }
    // Verify actual player state
    match get_player_state().as_str() {
        "paused" | "stopped" => json!({"ok": true, "status": "Playback paused"}),
        state => json!({"ok": false, "error": format!("Pause sent but player state is: {}", state)}),
    }
}

fn get_player_state() -> String {
    Command::new("osascript")
        .args(["-e", "tell application \"Spotify\" to player state as string"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
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
