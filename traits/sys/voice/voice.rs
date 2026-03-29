use serde_json::{json, Value};

/// sys.voice — Voice I/O chat service.
///
/// Starts an interactive voice chat loop that replaces terminal I/O:
/// - Listens via microphone → transcribes (llm.voice.listen)
/// - Sends transcribed text to ACP agent (llm.prompt.acp)
/// - Speaks response via TTS (llm.voice.speak)
/// - Repeat until silence/quit
///
/// Args: [agent?, model?, voice?, duration?]
pub fn voice(args: &[Value]) -> Value {
    let agent = args.first()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("opencode");

    let model = args.get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    let tts_voice = args.get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("nova");

    let duration = args.get(3)
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
        })
        .unwrap_or(15)
        .clamp(1, 30) as u32;

    // Verify API key is available before starting loop
    let has_key = kernel_logic::platform::secret_get("openai_api_key").is_some()
        || std::env::var("OPENAI_API_KEY").is_ok();
    if !has_key {
        return json!({"ok": false, "error": "OpenAI API key not found. Set via: traits call sys.secrets set openai_api_key <key>"});
    }

    // Verify audio tools are available
    if !which("rec") && !(cfg!(target_os = "macos") && which("ffmpeg")) {
        return json!({"ok": false, "error": "No audio recorder found. Install sox (brew install sox) or ffmpeg (brew install ffmpeg)."});
    }

    // Ensure ACP proxy is running
    let ensure_result = kernel_logic::platform::dispatch(
        "llm.prompt.acp.start",
        &[json!(agent)],
    );
    if let Some(r) = &ensure_result {
        if r.get("ok").and_then(|v| v.as_bool()) == Some(false) {
            return json!({"ok": false, "error": format!("Failed to start ACP proxy: {}", r.get("error").and_then(|e| e.as_str()).unwrap_or("unknown"))});
        }
    }

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".into());

    voice_loop(agent, model, tts_voice, duration, &cwd)
}

/// Main voice chat loop.
fn voice_loop(agent: &str, model: &str, tts_voice: &str, duration: u32, cwd: &str) -> Value {
    use std::io::Write;

    let mut turns = 0u32;

    eprintln!("\x1b[96m\x1b[1mVoice chat\x1b[0m \x1b[90m(agent: {agent}{model_info})\x1b[0m",
        model_info = if model.is_empty() { String::new() } else { format!(", model: {model}") });
    eprintln!("\x1b[90mSpeak after the 🎤 prompt. Say \"quit\" or \"exit\" to stop.\x1b[0m");
    eprintln!("\x1b[90mPress Ctrl+C to abort at any time.\x1b[0m\n");

    loop {
        // ── 1. Listen ──
        eprint!("\x1b[96m🎤 Listening…\x1b[0m ");
        std::io::stderr().flush().ok();

        let listen_result = kernel_logic::platform::dispatch(
            "llm.voice.listen",
            &[json!(null), json!(duration)],
        ).unwrap_or_else(|| json!({"ok": false, "error": "llm.voice.listen not available"}));

        if listen_result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err = listen_result.get("error").and_then(|e| e.as_str()).unwrap_or("listen failed");
            eprintln!("\n\x1b[31m✗ {err}\x1b[0m");
            if err.contains("API key") {
                return json!({"ok": false, "error": err, "turns": turns});
            }
            continue; // retry on transient errors
        }

        let text = listen_result.get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if text.is_empty() {
            eprintln!("\x1b[90m(no speech detected)\x1b[0m\n");
            continue;
        }

        eprintln!("\x1b[0m\x1b[92m\"{text}\"\x1b[0m");

        // Check for quit commands
        let lower = text.to_lowercase();
        if lower == "quit" || lower == "exit" || lower == "stop"
            || lower == "goodbye" || lower == "bye" {
            eprintln!("\n\x1b[90mEnding voice chat.\x1b[0m");
            break;
        }

        // ── 2. Send to ACP ──
        eprint!("\x1b[90mthinking…\x1b[0m ");
        std::io::stderr().flush().ok();

        let model_arg = if model.is_empty() { "" } else { model };
        let acp_result = kernel_logic::platform::dispatch(
            "llm.prompt.acp",
            &[json!(text), json!(agent), json!(cwd), json!("false"), json!(model_arg)],
        ).unwrap_or_else(|| json!({"ok": false, "error": "llm.prompt.acp not available"}));

        let response = if let Some(s) = acp_result.as_str() {
            s.to_string()
        } else if let Some(err) = acp_result.get("error").and_then(|e| e.as_str()) {
            eprintln!("\n\x1b[31m✗ {err}\x1b[0m\n");
            continue;
        } else {
            serde_json::to_string(&acp_result).unwrap_or_default()
        };

        // Clear "thinking…" and print response for visual feedback
        eprint!("\r\x1b[2K");
        eprintln!("\x1b[96m💬 {response}\x1b[0m\n");

        turns += 1;

        // ── 3. Speak response ──
        // Strip markdown-heavy content for cleaner speech
        let speak_text = clean_for_speech(&response);
        if !speak_text.is_empty() {
            kernel_logic::platform::dispatch(
                "llm.voice.speak",
                &[json!(speak_text), json!(tts_voice)],
            );
        }
    }

    json!({"ok": true, "turns": turns})
}

/// Strip markdown artifacts and code blocks to produce cleaner speech.
fn clean_for_speech(text: &str) -> String {
    let mut result = String::new();
    let mut in_code_block = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            if !in_code_block {
                result.push_str(" (code omitted) ");
            }
            continue;
        }
        if in_code_block {
            continue;
        }
        // Strip leading # for headers
        let cleaned = if trimmed.starts_with('#') {
            trimmed.trim_start_matches('#').trim()
        } else {
            trimmed
        };
        // Strip bold/italic markers
        let cleaned = cleaned.replace("**", "").replace("__", "");
        if !cleaned.is_empty() {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(&cleaned);
        }
    }

    result
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
