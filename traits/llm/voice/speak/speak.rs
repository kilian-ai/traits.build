use serde_json::{json, Value};

/// llm.voice.speak — Text-to-speech synthesis using OpenAI TTS API.
///
/// Sends text to OpenAI's `/v1/audio/speech` endpoint, saves the audio
/// to a temp file, and optionally plays it via the platform audio player.
///
/// Args: [text, voice?, model?, play?]
pub fn speak(args: &[Value]) -> Value {
    let text = match args.first().and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t,
        _ => return json!({"ok": false, "error": "text is required"}),
    };

    let voice = args.get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("nova");

    let model = args.get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("tts-1");

    let play = args.get(3)
        .and_then(|v| {
            if v.is_boolean() { v.as_bool() }
            else if let Some(s) = v.as_str() { Some(s != "false") }
            else { None }
        })
        .unwrap_or(true);

    // Resolve API key
    let api_key = match resolve_api_key() {
        Some(k) => k,
        None => return json!({"ok": false, "error": "OpenAI API key not found. Set via: traits call sys.secrets set openai_api_key <key>"}),
    };

    // Build request
    let body = json!({
        "model": model,
        "input": text,
        "voice": voice,
        "response_format": "mp3"
    });

    // Call OpenAI TTS API via curl (binary response)
    let out_path = format!("/tmp/traits_tts_{}.mp3", std::process::id());
    match tts_request(&api_key, &body, &out_path) {
        Ok(()) => {
            if play {
                play_audio(&out_path);
            }
            json!({"ok": true, "file": out_path})
        }
        Err(e) => json!({"ok": false, "error": e}),
    }
}

/// Resolve OpenAI API key from secrets or environment.
fn resolve_api_key() -> Option<String> {
    // Try secrets store first
    let key = kernel_logic::platform::secret_get("openai_api_key");
    if key.is_some() { return key; }
    // Fallback to environment
    std::env::var("OPENAI_API_KEY").ok()
}

/// Make TTS API call using curl, writing binary audio to file.
fn tts_request(api_key: &str, body: &Value, out_path: &str) -> Result<(), String> {
    use std::process::Command;

    let body_str = serde_json::to_string(body).map_err(|e| e.to_string())?;

    let output = Command::new("curl")
        .args(["-sS", "--fail-with-body",
               "-X", "POST",
               "https://api.openai.com/v1/audio/speech",
               "-H", "Content-Type: application/json",
               "-H", &format!("Authorization: Bearer {}", api_key),
               "-d", &body_str,
               "-o", out_path,
               "--connect-timeout", "15",
               "--max-time", "60"])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        // Try to read the output file for error details
        let stderr = String::from_utf8_lossy(&output.stderr);
        let body_err = std::fs::read_to_string(out_path).unwrap_or_default();
        let _ = std::fs::remove_file(out_path);
        if !body_err.is_empty() {
            if let Ok(parsed) = serde_json::from_str::<Value>(&body_err) {
                if let Some(msg) = parsed.pointer("/error/message").and_then(|v| v.as_str()) {
                    return Err(msg.to_string());
                }
            }
        }
        return Err(format!("TTS API error: {}", stderr));
    }

    // Verify the file exists and is not empty
    let meta = std::fs::metadata(out_path).map_err(|e| format!("Output file error: {}", e))?;
    if meta.len() == 0 {
        let _ = std::fs::remove_file(out_path);
        return Err("TTS API returned empty audio".into());
    }

    Ok(())
}

/// Play an audio file using the platform audio player.
fn play_audio(path: &str) {
    use std::process::Command;

    // macOS: afplay, Linux: aplay/paplay/mpv
    let (player, extra_args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("afplay", &[])
    } else if std::path::Path::new("/usr/bin/paplay").exists() {
        ("paplay", &[])
    } else if std::path::Path::new("/usr/bin/mpv").exists() {
        ("mpv", &["--no-video", "--really-quiet"])
    } else {
        // No player found — skip silently
        return;
    };

    let mut cmd = Command::new(player);
    cmd.args(extra_args);
    cmd.arg(path);
    let _ = cmd.status();
}
