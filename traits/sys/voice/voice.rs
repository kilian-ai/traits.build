use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

const INSTRUCTIONS: &str = include_str!("realtime_instructions.md");

/// Global flag for SIGINT handling.
static VOICE_RUNNING: AtomicBool = AtomicBool::new(false);
/// Mute mic while model is speaking to prevent feedback.
static MIC_MUTED: AtomicBool = AtomicBool::new(false);

extern "C" fn sigint_handler(_: libc::c_int) {
    VOICE_RUNNING.store(false, Ordering::SeqCst);
}

/// sys.voice — Real-time voice chat via OpenAI Realtime API.
///
/// Opens a WebSocket to OpenAI's Realtime API for direct speech-to-speech
/// conversation. Mic audio is streamed continuously; the model responds
/// with audio directly. No intermediate STT/TTS pipeline.
///
/// Args: [voice?, model?]
pub fn voice(args: &[Value]) -> Value {
    let voice_name = args.first()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("cedar");

    let model = args.get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("gpt-4o-realtime-preview");

    // Resolve API key
    let api_key = match resolve_api_key() {
        Some(k) => k,
        None => return json!({"ok": false, "error": "OpenAI API key not found. Set via: traits call sys.secrets set openai_api_key <key>"}),
    };

    // Verify sox is available (provides `rec` and `play`)
    if !which("rec") {
        return json!({"ok": false, "error": "sox not found. Install: brew install sox"});
    }

    match realtime_session(&api_key, model, voice_name) {
        Ok(turns) => json!({"ok": true, "turns": turns}),
        Err(e) => json!({"ok": false, "error": e}),
    }
}

fn resolve_api_key() -> Option<String> {
    kernel_logic::platform::secret_get("openai_api_key")
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
}

fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ═══════════════════════════════════════════════════════════════════════════
// Playback thread commands
// ═══════════════════════════════════════════════════════════════════════════

enum PlayCmd {
    /// PCM audio data to write to speaker
    Audio(Vec<u8>),
    /// Interrupt: kill current playback, drain queue
    Flush,
    /// Shut down the playback thread
    Shutdown,
}

// ═══════════════════════════════════════════════════════════════════════════
// Main Realtime session
// ═══════════════════════════════════════════════════════════════════════════

fn realtime_session(api_key: &str, model: &str, voice_name: &str) -> Result<u32, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use tungstenite::{connect, Message};
    use tungstenite::client::IntoClientRequest;
    use tungstenite::stream::MaybeTlsStream;
    use std::time::Duration;

    // ── Connect ──
    let url = format!("wss://api.openai.com/v1/realtime?model={}", model);
    eprintln!("\x1b[90mConnecting to {model}…\x1b[0m");

    let mut request = url.into_client_request().map_err(|e| format!("Build request: {e}"))?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", api_key).parse().map_err(|e| format!("Auth header: {e}"))?
    );
    request.headers_mut().insert(
        "OpenAI-Beta",
        "realtime=v1".parse().map_err(|e| format!("Beta header: {e}"))?
    );

    let (mut ws, _) = connect(request).map_err(|e| format!("WebSocket connect: {e}"))?;

    // ── Wait for session.created ──
    let mut session_ready = false;
    for _ in 0..100 {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let ev: Value = serde_json::from_str(&text).unwrap_or_default();
                if ev.get("type").and_then(|t| t.as_str()) == Some("session.created") {
                    session_ready = true;
                    break;
                }
            }
            _ => {}
        }
    }
    if !session_ready {
        return Err("Timeout waiting for session.created".into());
    }

    // ── Configure session ──
    let session_update = json!({
        "type": "session.update",
        "session": {
            "instructions": INSTRUCTIONS,
            "modalities": ["text", "audio"],
            "voice": voice_name,
            "input_audio_format": "pcm16",
            "output_audio_format": "pcm16",
            "turn_detection": {
                "type": "server_vad",
                "threshold": 0.5,
                "prefix_padding_ms": 300,
                "silence_duration_ms": 500
            },
            "input_audio_transcription": {
                "model": "whisper-1"
            }
        }
    });

    ws.send(Message::Text(session_update.to_string()))
        .map_err(|e| format!("Send session.update: {e}"))?;

    // ── Set read timeout for non-blocking interleave ──
    match ws.get_ref() {
        MaybeTlsStream::NativeTls(tls) => {
            tls.get_ref().set_read_timeout(Some(Duration::from_millis(20))).ok();
        }
        MaybeTlsStream::Plain(tcp) => {
            tcp.set_read_timeout(Some(Duration::from_millis(20))).ok();
        }
        _ => {}
    }

    // ── Start mic capture thread ──
    let (mic_tx, mic_rx) = mpsc::channel::<Vec<u8>>();
    let mic_handle = std::thread::spawn(move || {
        mic_capture_loop(mic_tx);
    });

    // ── Start playback thread ──
    let (play_tx, play_rx) = mpsc::channel::<PlayCmd>();
    let play_handle = std::thread::spawn(move || {
        playback_loop(play_rx);
    });

    // ── Install SIGINT handler ──
    VOICE_RUNNING.store(true, Ordering::SeqCst);
    let prev_handler = unsafe {
        libc::signal(libc::SIGINT, sigint_handler as *const () as libc::sighandler_t)
    };

    // ── Print UI ──
    eprintln!("\x1b[96m\x1b[1mRealtime voice chat\x1b[0m \x1b[90m(model: {model}, voice: {voice_name})\x1b[0m");
    eprintln!("\x1b[90mSpeak naturally. Press Ctrl+C to stop.\x1b[0m\n");

    // ── Main event loop ──
    let mut turns = 0u32;

    while VOICE_RUNNING.load(Ordering::Relaxed) {
        // 1. Read server events
        match ws.read() {
            Ok(Message::Text(text)) => {
                let ev: Value = serde_json::from_str(&text).unwrap_or_default();
                let event_type = ev.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match event_type {
                    // ── Audio output from model ──
                    "response.audio.delta" => {
                        MIC_MUTED.store(true, Ordering::Relaxed);
                        if let Some(delta) = ev.get("delta").and_then(|d| d.as_str()) {
                            if let Ok(pcm) = BASE64.decode(delta) {
                                play_tx.send(PlayCmd::Audio(pcm)).ok();
                            }
                        }
                    }

                    // ── Model finished speaking — unmute mic ──
                    "response.audio.done" => {
                        MIC_MUTED.store(false, Ordering::Relaxed);
                    }

                    // ── User started speaking — interrupt playback, unmute ──
                    "input_audio_buffer.speech_started" => {
                        MIC_MUTED.store(false, Ordering::Relaxed);
                        play_tx.send(PlayCmd::Flush).ok();
                    }

                    // ── User's transcribed speech ──
                    "conversation.item.input_audio_transcription.completed" => {
                        if let Some(transcript) = ev.get("transcript").and_then(|t| t.as_str()) {
                            let trimmed = transcript.trim();
                            if !trimmed.is_empty() {
                                eprintln!("\x1b[92m🎤 {trimmed}\x1b[0m");
                            }
                        }
                        turns += 1;
                    }

                    // ── Model's response transcript ──
                    "response.audio_transcript.done" => {
                        if let Some(transcript) = ev.get("transcript").and_then(|t| t.as_str()) {
                            let trimmed = transcript.trim();
                            if !trimmed.is_empty() {
                                eprintln!("\x1b[96m💬 {trimmed}\x1b[0m");
                            }
                        }
                    }

                    // ── Session configured ──
                    "session.updated" => {
                        eprintln!("\x1b[90m✓ Session configured\x1b[0m");
                    }

                    // ── Error from server ──
                    "error" => {
                        let msg = ev.pointer("/error/message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown error");
                        eprintln!("\x1b[31m✗ {msg}\x1b[0m");
                        if msg.contains("auth") || msg.contains("key") || msg.contains("quota") {
                            VOICE_RUNNING.store(false, Ordering::Relaxed);
                        }
                    }

                    // Lifecycle events we can ignore
                    "response.created" | "response.done"
                    | "response.output_item.added" | "response.output_item.done"
                    | "response.content_part.added" | "response.content_part.done"
                    | "response.audio_transcript.delta"
                    | "input_audio_buffer.speech_stopped" | "input_audio_buffer.committed"
                    | "conversation.item.created" | "rate_limits.updated" => {}

                    _ => {
                        // Uncomment to debug:
                        // eprintln!("\x1b[90m[{event_type}]\x1b[0m");
                    }
                }
            }
            Ok(Message::Close(_)) => {
                eprintln!("\x1b[90mSession closed by server.\x1b[0m");
                break;
            }
            // Read timeout — no message, that's fine
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                // Connection reset or other fatal error
                let msg = e.to_string();
                if !msg.contains("Connection reset") {
                    eprintln!("\x1b[31m✗ WebSocket: {msg}\x1b[0m");
                }
                break;
            }
            _ => {}
        }

        // 2. Send queued mic audio to server (drop chunks while muted)
        let mut sent = 0;
        while let Ok(chunk) = mic_rx.try_recv() {
            if MIC_MUTED.load(Ordering::Relaxed) {
                continue; // drop mic data while model is speaking
            }
            let b64 = BASE64.encode(&chunk);
            let event = json!({
                "type": "input_audio_buffer.append",
                "audio": b64
            });
            if ws.send(Message::Text(event.to_string())).is_err() {
                VOICE_RUNNING.store(false, Ordering::Relaxed);
                break;
            }
            sent += 1;
            if sent > 10 { break; } // Don't block too long on sends
        }
    }

    // ── Cleanup ──
    VOICE_RUNNING.store(false, Ordering::Relaxed);
    MIC_MUTED.store(false, Ordering::Relaxed);
    unsafe { libc::signal(libc::SIGINT, prev_handler); }
    let _ = ws.close(None);
    play_tx.send(PlayCmd::Shutdown).ok();
    drop(play_tx);
    drop(mic_rx);
    let _ = mic_handle.join();
    let _ = play_handle.join();

    eprintln!("\n\x1b[90mVoice chat ended.\x1b[0m");
    Ok(turns)
}

// ═══════════════════════════════════════════════════════════════════════════
// Mic capture thread — records PCM 24kHz 16-bit mono, sends chunks
// ═══════════════════════════════════════════════════════════════════════════

fn mic_capture_loop(tx: mpsc::Sender<Vec<u8>>) {
    use std::process::{Command, Stdio};
    use std::io::Read;

    let mut child = match Command::new("rec")
        .args([
            "-q",            // suppress progress
            "-t", "raw",     // raw PCM output
            "-r", "24000",   // 24 kHz (Realtime API requirement)
            "-c", "1",       // mono
            "-e", "signed",  // signed integer
            "-b", "16",      // 16-bit
            "-",             // output to stdout
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[31m✗ Failed to start mic: {e}\x1b[0m");
            return;
        }
    };

    let stdout = child.stdout.take().unwrap();
    let mut reader = std::io::BufReader::new(stdout);
    // 100ms of audio at 24kHz 16-bit mono = 24000 * 2 * 0.1 = 4800 bytes
    let mut buf = vec![0u8; 4800];

    while VOICE_RUNNING.load(Ordering::Relaxed) {
        match reader.read_exact(&mut buf) {
            Ok(()) => {
                if tx.send(buf.clone()).is_err() {
                    break; // receiver dropped
                }
            }
            Err(_) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}

// ═══════════════════════════════════════════════════════════════════════════
// Audio playback thread — receives PCM chunks from server, plays via sox
// ═══════════════════════════════════════════════════════════════════════════

fn playback_loop(rx: mpsc::Receiver<PlayCmd>) {
    use std::process::{Command, Stdio, Child, ChildStdin};
    use std::io::Write;

    let mut player: Option<Child> = None;
    let mut stdin: Option<ChildStdin> = None;

    fn start_player() -> Option<(Child, ChildStdin)> {
        let mut child = Command::new("play")
            .args([
                "-q",            // suppress progress
                "-t", "raw",     // raw PCM input
                "-r", "24000",   // 24 kHz
                "-c", "1",       // mono
                "-e", "signed",  // signed integer
                "-b", "16",      // 16-bit
                "-",             // read from stdin
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let s = child.stdin.take()?;
        Some((child, s))
    }

    while let Ok(cmd) = rx.recv() {
        match cmd {
            PlayCmd::Audio(pcm) => {
                // Lazily start player on first audio chunk
                if stdin.is_none() {
                    if let Some((p, s)) = start_player() {
                        player = Some(p);
                        stdin = Some(s);
                    }
                }
                if let Some(ref mut s) = stdin {
                    if s.write_all(&pcm).is_err() {
                        // Player died — restart
                        if let Some(ref mut p) = player {
                            let _ = p.wait();
                        }
                        if let Some((p, s2)) = start_player() {
                            player = Some(p);
                            stdin = Some(s2);
                            stdin.as_mut().unwrap().write_all(&pcm).ok();
                        }
                    }
                }
            }
            PlayCmd::Flush => {
                // Kill current playback immediately (interruption)
                drop(stdin.take());
                if let Some(ref mut p) = player {
                    let _ = p.kill();
                    let _ = p.wait();
                }
                player = None;
                // Drain any queued audio commands
                while let Ok(cmd) = rx.try_recv() {
                    if matches!(cmd, PlayCmd::Shutdown) { return; }
                }
            }
            PlayCmd::Shutdown => {
                drop(stdin.take());
                if let Some(ref mut p) = player {
                    let _ = p.wait();
                }
                break;
            }
        }
    }
}
