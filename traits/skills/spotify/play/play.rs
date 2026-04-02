use serde_json::{json, Value};
use std::process::Command;

/// Words that mean "just resume playback" rather than being a search query.
const RESUME_WORDS: &[&str] = &["", "spotify", "music", "resume", "unpause"];

/// skills.spotify.play — Play a track, album, artist, or playlist on Spotify.
/// If no query is given (or query is just "spotify"/"music"), resumes the current track.
pub fn play(args: &[Value]) -> Value {
    let raw_query = args.first().and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let query_lower = raw_query.to_lowercase();
    let is_resume = RESUME_WORDS.iter().any(|w| *w == query_lower);

    let result = if is_resume {
        // Resume playback
        run_osascript("tell application \"Spotify\" to play")
    } else if raw_query.starts_with("spotify:") {
        // If it looks like a Spotify URI, play it directly
        let script = format!(
            "tell application \"Spotify\" to play track \"{}\"",
            raw_query.replace('"', "\\\"")
        );
        run_osascript(&script)
    } else {
        // Search: use the search URI scheme
        let search_uri = format!("spotify:search:{}", raw_query.replace(' ', "%20"));
        let script = format!(
            "tell application \"Spotify\" to play track \"{}\"",
            search_uri.replace('"', "\\\"")
        );
        run_osascript(&script)
    };

    if !result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return result;
    }

    // For search-based playback, Spotify needs a moment to load
    if !is_resume && !raw_query.starts_with("spotify:") {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // Verify actual player state
    let state = get_player_state();
    if state == "playing" {
        if is_resume {
            json!({"ok": true, "status": "Playback resumed"})
        } else {
            json!({"ok": true, "status": format!("Now playing: {}", raw_query)})
        }
    } else {
        json!({"ok": false, "error": format!("Play sent but player state is: {}", state)})
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
