use serde_json::{json, Value};
use std::process::Command;

/// skills.spotify.play — Play a track, album, artist, or playlist on Spotify.
/// If no query is given, resumes the current track.
pub fn play(args: &[Value]) -> Value {
    let query = args.first().and_then(|v| v.as_str()).unwrap_or("").trim().to_string();

    let result = if query.is_empty() {
        // Resume playback
        run_osascript("tell application \"Spotify\" to play")
    } else if query.starts_with("spotify:") {
        // If it looks like a Spotify URI, play it directly
        let script = format!(
            "tell application \"Spotify\" to play track \"{}\"",
            query.replace('"', "\\\"")
        );
        run_osascript(&script)
    } else {
        // Search: use the search URI scheme
        let search_uri = format!("spotify:search:{}", query.replace(' ', "%20"));
        let script = format!(
            "tell application \"Spotify\" to play track \"{}\"",
            search_uri.replace('"', "\\\"")
        );
        run_osascript(&script)
    };

    // Enrich successful results with a human-readable status
    if result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        if query.is_empty() {
            json!({"ok": true, "status": "Playback resumed"})
        } else {
            json!({"ok": true, "status": format!("Now playing: {}", query)})
        }
    } else {
        result
    }
}

fn run_osascript(script: &str) -> Value {
    match Command::new("osascript").args(["-e", script]).output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if out.status.success() {
                json!({"ok": true, "result": stdout})
            } else {
                json!({"ok": false, "error": stderr})
            }
        }
        Err(e) => json!({"ok": false, "error": format!("osascript failed: {}", e)}),
    }
}
