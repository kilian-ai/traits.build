use serde_json::{json, Value};
use std::process::Command;

/// skills.spotify.status — Get current Spotify playback status.
pub fn status(_args: &[Value]) -> Value {
    let script = r#"
tell application "Spotify"
    if it is running then
        set pState to player state as string
        set tName to name of current track
        set tArtist to artist of current track
        set tAlbum to album of current track
        set tDuration to duration of current track
        set pPos to player position
        set sVol to sound volume
        return pState & "|" & tName & "|" & tArtist & "|" & tAlbum & "|" & tDuration & "|" & pPos & "|" & sVol
    else
        return "not_running"
    end if
end tell
"#;

    match Command::new("osascript").args(["-e", script]).output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if !out.status.success() {
                return json!({"ok": false, "error": stderr});
            }
            if stdout == "not_running" {
                return json!({"ok": true, "running": false, "state": "not_running"});
            }
            let parts: Vec<&str> = stdout.splitn(7, '|').collect();
            if parts.len() >= 7 {
                json!({
                    "ok": true,
                    "running": true,
                    "state": parts[0],
                    "track": parts[1],
                    "artist": parts[2],
                    "album": parts[3],
                    "duration_ms": parts[4].parse::<i64>().unwrap_or(0),
                    "position_s": parts[5].parse::<f64>().unwrap_or(0.0).round() as i64,
                    "volume": parts[6].parse::<i64>().unwrap_or(0),
                })
            } else {
                json!({"ok": true, "raw": stdout})
            }
        }
        Err(e) => json!({"ok": false, "error": format!("osascript failed: {}", e)}),
    }
}
