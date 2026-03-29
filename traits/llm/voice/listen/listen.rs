use serde_json::{json, Value};

/// llm.voice.listen — Speech-to-text transcription using OpenAI Whisper API.
///
/// Records audio from the microphone (or reads from a file), then sends it
/// to OpenAI's `/v1/audio/transcriptions` endpoint.
///
/// Args: [file?, duration?, language?]
pub fn listen(args: &[Value]) -> Value {
    let file_arg = args.first()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let duration = args.get(1)
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
        })
        .unwrap_or(10)
        .clamp(1, 30) as u32;

    let language = args.get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    // Resolve API key
    let api_key = match resolve_api_key() {
        Some(k) => k,
        None => return json!({"ok": false, "error": "OpenAI API key not found. Set via: traits call sys.secrets set openai_api_key <key>"}),
    };

    // Get audio file path — either provided or record from mic
    let (audio_path, is_temp) = match file_arg {
        Some(path) => {
            if !std::path::Path::new(path).exists() {
                return json!({"ok": false, "error": format!("File not found: {}", path)});
            }
            (path.to_string(), false)
        }
        None => {
            match record_audio(duration) {
                Ok(path) => (path, true),
                Err(e) => return json!({"ok": false, "error": e}),
            }
        }
    };

    // Send to Whisper API
    let result = whisper_transcribe(&api_key, &audio_path, language);

    // Clean up temp recording
    if is_temp {
        let _ = std::fs::remove_file(&audio_path);
    }

    result
}

/// Resolve OpenAI API key from secrets or environment.
fn resolve_api_key() -> Option<String> {
    let key = kernel_logic::platform::secret_get("openai_api_key");
    if key.is_some() { return key; }
    std::env::var("OPENAI_API_KEY").ok()
}

/// Record audio from the microphone.
///
/// Uses `sox` (rec) if available, otherwise falls back to macOS-specific
/// tools. Returns path to the recorded WAV/M4A file.
fn record_audio(duration: u32) -> Result<String, String> {
    use std::process::Command;

    let out_path = format!("/tmp/traits_stt_{}.wav", std::process::id());
    let dur_str = duration.to_string();

    // Try sox/rec first (cross-platform, best quality)
    if which("rec") {
        let status = Command::new("rec")
            .args([
                "-q",           // suppress progress output
                &out_path,
                "rate", "16000",
                "channels", "1",
                "gain", "6",    // +6dB input gain for quiet mics
                "trim", "0", &dur_str,
                // Stop after 2.5s of silence (0.1% threshold = very sensitive).
                // No start-gate — recording begins immediately so quiet speech
                // is never clipped.
                "silence", "1", "0.0", "0%", "1", "2.5", "0.1%",
            ])
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("rec failed: {}", e))?;
        if !status.success() {
            return Err("Audio recording failed (rec)".into());
        }
        // Check if file has meaningful content (sox may exit 0 with tiny/empty file)
        let file_size = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
        if file_size < 1000 {
            let _ = std::fs::remove_file(&out_path);
            return Err("No speech detected. If your mic is working, check that your terminal app has Microphone permission in System Settings → Privacy & Security.".into());
        }
        return Ok(out_path);
    }

    // macOS fallback: use ffmpeg to record from default mic
    if cfg!(target_os = "macos") && which("ffmpeg") {
        let out_m4a = format!("/tmp/traits_stt_{}.m4a", std::process::id());
        let status = Command::new("ffmpeg")
            .args(["-y", "-f", "avfoundation", "-i", ":default",
                   "-t", &dur_str, "-ac", "1", "-ar", "16000",
                   "-loglevel", "error", &out_m4a])
            .status()
            .map_err(|e| format!("ffmpeg failed: {}", e))?;
        if !status.success() {
            return Err("Audio recording failed (ffmpeg)".into());
        }
        return Ok(out_m4a);
    }

    Err("No audio recorder found. Install sox (brew install sox) or ffmpeg (brew install ffmpeg).".into())
}

/// Transcribe audio via OpenAI Whisper API using curl multipart upload.
fn whisper_transcribe(api_key: &str, audio_path: &str, language: Option<&str>) -> Value {
    use std::process::Command;

    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "--fail-with-body",
              "-X", "POST",
              "https://api.openai.com/v1/audio/transcriptions",
              "-H", &format!("Authorization: Bearer {}", api_key),
              "-F", &format!("file=@{}", audio_path),
              "-F", "model=whisper-1",
              "-F", "response_format=json",
              "--connect-timeout", "15",
              "--max-time", "60"]);

    if let Some(lang) = language {
        cmd.args(["-F", &format!("language={}", lang)]);
    }

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => return json!({"ok": false, "error": format!("curl failed: {}", e)}),
    };

    let body = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        if let Ok(parsed) = serde_json::from_str::<Value>(&body) {
            if let Some(msg) = parsed.pointer("/error/message").and_then(|v| v.as_str()) {
                return json!({"ok": false, "error": msg});
            }
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return json!({"ok": false, "error": format!("Whisper API error: {}", stderr)});
    }

    match serde_json::from_str::<Value>(&body) {
        Ok(parsed) => {
            let text = parsed.get("text").and_then(|v| v.as_str()).unwrap_or("");
            json!({"ok": true, "text": text, "file": audio_path})
        }
        Err(e) => json!({"ok": false, "error": format!("Failed to parse response: {}", e)}),
    }
}

/// Check if a command is available in PATH.
fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
