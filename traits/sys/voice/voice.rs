use serde_json::{json, Value, Map};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Instant;

const VOICE_INSTRUCTIONS: &str = include_str!("realtime_instructions.md");

/// Global flag for SIGINT handling.
static VOICE_RUNNING: AtomicBool = AtomicBool::new(false);
/// Mute mic while model is speaking to prevent feedback.
static MIC_MUTED: AtomicBool = AtomicBool::new(false);
/// Set by playback thread when `play` process finishes all buffered audio.
static PLAYBACK_IDLE: AtomicBool = AtomicBool::new(true);

extern "C" fn sigint_handler(_: libc::c_int) {
    VOICE_RUNNING.store(false, Ordering::SeqCst);
}

/// sys.voice — Real-time voice chat via OpenAI Realtime API.
///
/// Opens a WebSocket to OpenAI's Realtime API for direct speech-to-speech
/// conversation. Mic audio is streamed continuously; the model responds
/// with audio directly. No intermediate STT/TTS pipeline.
///
/// Args: [voice?, model?, agent?, session_id?]
pub fn voice(args: &[Value]) -> Value {
    // Read persistent defaults from sys.config, then allow arg overrides
    let default_voice = read_voice_pref("voice").unwrap_or_else(|| "cedar".into());
    let default_model = read_voice_pref("model").unwrap_or_else(|| "gpt-4o-realtime-preview".into());
    let default_agent = read_voice_pref("agent").unwrap_or_default();

    let voice_name = args.first()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_voice);

    let model = args.get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_model);

    let agent = args.get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_agent);

    let session_id = args.get(3)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // Resolve API key
    let api_key = match resolve_api_key() {
        Some(k) => k,
        None => return json!({"ok": false, "error": "OpenAI API key not found. Set via: traits call sys.secrets set openai_api_key <key>"}),
    };

    // Verify sox is available (provides `rec` and `play`)
    if !which("rec") {
        return json!({"ok": false, "error": "sox not found. Install: brew install sox"});
    }

    // Build combined instructions: agent context + voice-specific tuning
    let instructions = build_instructions(agent, session_id.as_deref());

    match realtime_session(&api_key, model, voice_name, &instructions, session_id.as_deref()) {
        Ok(turns) => json!({"ok": true, "turns": turns}),
        Err(e) => json!({"ok": false, "error": e}),
    }
}

/// Build combined instructions from agent context + voice-specific tuning.
fn build_instructions(agent: &str, session_id: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();

    // 1. Agent context — tell the model who it's acting as
    if !agent.is_empty() {
        parts.push(format!(
            "You are operating as the \"{}\" coding agent on the traits.build platform. \
             The user is a developer who may ask about code, architecture, or technical topics. \
             Maintain awareness of this agent context in your responses.",
            agent
        ));
    }

    // 2. Conversation history — provide recent context from the chat session
    if let Some(sid) = session_id {
        if let Some(result) = kernel_logic::platform::dispatch(
            "sys.chat", &[json!("get"), json!(sid)],
        ) {
            if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                if let Some(messages) = result.pointer("/session/messages").and_then(|v| v.as_array()) {
                    // Include last few messages as context (not too many — voice is concise)
                    let recent: Vec<&Value> = messages.iter().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
                    if !recent.is_empty() {
                        let mut ctx = String::from("Recent conversation context (for continuity):\n");
                        for msg in &recent {
                            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("?");
                            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            // Truncate long messages for voice context
                            let short = if content.len() > 200 { &content[..200] } else { content };
                            ctx.push_str(&format!("  {}: {}\n", role, short));
                        }
                        parts.push(ctx);
                    }
                }
            }
        }
    }

    // 3. Voice-specific tuning (always last — most specific)
    parts.push(VOICE_INSTRUCTIONS.to_string());

    parts.join("\n\n")
}

fn resolve_api_key() -> Option<String> {
    kernel_logic::platform::secret_get("openai_api_key")
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
}

/// Read a voice preference from persistent config (sys.config sys.voice <key>).
fn read_voice_pref(key: &str) -> Option<String> {
    kernel_logic::platform::dispatch(
        "sys.config",
        &[json!("get"), json!("sys.voice"), json!(key)],
    )
    .and_then(|r| r.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()))
    .filter(|s| !s.is_empty())
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
    /// Close current player (end of response), signal PLAYBACK_IDLE when done
    FinishResponse,
    /// Shut down the playback thread
    Shutdown,
}

// ═══════════════════════════════════════════════════════════════════════════
// Main Realtime session
// ═══════════════════════════════════════════════════════════════════════════

fn realtime_session(api_key: &str, model: &str, voice_name: &str, instructions: &str, session_id: Option<&str>) -> Result<u32, String> {
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

    // ── Configure session with tools ──
    let tools = build_tools();
    let tool_count = tools.len();
    let mut session_config = json!({
        "instructions": instructions,
        "modalities": ["text", "audio"],
        "voice": voice_name,
        "input_audio_format": "pcm16",
        "output_audio_format": "pcm16",
        "input_audio_noise_reduction": {
            "type": "far_field"
        },
        "turn_detection": {
            "type": "server_vad",
            "threshold": 0.8,
            "prefix_padding_ms": 300,
            "silence_duration_ms": 800
        },
        "input_audio_transcription": {
            "model": "whisper-1"
        }
    });
    if !tools.is_empty() {
        session_config["tools"] = Value::Array(tools);
    }
    let session_update = json!({
        "type": "session.update",
        "session": session_config
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
    PLAYBACK_IDLE.store(true, Ordering::SeqCst);
    let prev_handler = unsafe {
        libc::signal(libc::SIGINT, sigint_handler as *const () as libc::sighandler_t)
    };

    // ── Print UI ──
    eprintln!("\x1b[96m\x1b[1mRealtime voice chat\x1b[0m \x1b[90m(model: {model}, voice: {voice_name}, {tool_count} tools)\x1b[0m");
    eprintln!("\x1b[90mSpeak naturally. Press Ctrl+C to stop.\x1b[0m\n");

    // ── Main event loop ──
    let mut turns = 0u32;
    let mut unmute_at: Option<Instant> = None;

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
                        PLAYBACK_IDLE.store(false, Ordering::Relaxed);
                        if let Some(delta) = ev.get("delta").and_then(|d| d.as_str()) {
                            if let Ok(pcm) = BASE64.decode(delta) {
                                play_tx.send(PlayCmd::Audio(pcm)).ok();
                            }
                        }
                    }

                    // ── Model finished sending audio — close player, wait for actual playback to finish ──
                    "response.audio.done" => {
                        // Tell playback thread to close stdin and wait for play to exit
                        play_tx.send(PlayCmd::FinishResponse).ok();
                        // Drain any mic chunks sent during playback
                        while mic_rx.try_recv().is_ok() {}
                        // Clear server input buffer
                        let clear_ev = json!({"type": "input_audio_buffer.clear"});
                        ws.send(Message::Text(clear_ev.to_string())).ok();
                    }

                    // ── User started speaking — interrupt playback, unmute ──
                    "input_audio_buffer.speech_started" => {
                        MIC_MUTED.store(false, Ordering::Relaxed);
                        PLAYBACK_IDLE.store(true, Ordering::Relaxed);
                        play_tx.send(PlayCmd::Flush).ok();
                    }

                    // ── User's transcribed speech ──
                    "conversation.item.input_audio_transcription.completed" => {
                        if let Some(transcript) = ev.get("transcript").and_then(|t| t.as_str()) {
                            let trimmed = transcript.trim();
                            if !trimmed.is_empty() {
                                eprintln!("\x1b[92m🎤 {trimmed}\x1b[0m");
                                // Persist user turn to session
                                if let Some(sid) = session_id {
                                    kernel_logic::platform::dispatch(
                                        "sys.chat",
                                        &[json!("append"), json!(sid), json!("user"), json!(trimmed)],
                                    );
                                }
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
                                // Persist assistant turn to session
                                if let Some(sid) = session_id {
                                    kernel_logic::platform::dispatch(
                                        "sys.chat",
                                        &[json!("append"), json!(sid), json!("assistant"), json!(trimmed)],
                                    );
                                }
                            }
                        }
                    }

                    // ── Session configured ──
                    "session.updated" => {
                        eprintln!("\x1b[90m✓ Session configured\x1b[0m");
                    }

                    // ── Function call — model wants to invoke a tool ──
                    "response.function_call_arguments.done" => {
                        let call_id = ev.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                        let func_name = ev.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let arguments = ev.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");

                        eprintln!("\x1b[93m⚡ {func_name}\x1b[0m");

                        // Dispatch the tool call
                        let result = dispatch_tool_call(func_name, arguments);

                        // Truncate very long results for voice context
                        let output = if result.len() > 2000 {
                            format!("{}…(truncated)", &result[..2000])
                        } else {
                            result
                        };

                        // If the model changed voice preferences, apply live
                        if func_name == "sys_voice_config" {
                            apply_live_config_change(arguments, session_id, &mut ws);
                        }

                        // Send function call output back to the model
                        let output_event = json!({
                            "type": "conversation.item.create",
                            "item": {
                                "type": "function_call_output",
                                "call_id": call_id,
                                "output": output
                            }
                        });
                        ws.send(Message::Text(output_event.to_string())).ok();

                        // Ask model to continue responding (with audio)
                        let continue_event = json!({
                            "type": "response.create"
                        });
                        ws.send(Message::Text(continue_event.to_string())).ok();
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
                    | "response.function_call_arguments.delta"
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

        // 2. Check if playback finished — unmute mic with settling delay
        //    The mic thread's read_exact may still contain speaker tail audio
        //    right after the play process exits. Wait 400ms to let room settle.
        if !PLAYBACK_IDLE.load(Ordering::Relaxed) {
            // Still playing — keep mic muted, reset settle timer
            unmute_at = None;
        } else if MIC_MUTED.load(Ordering::Relaxed) {
            if unmute_at.is_none() {
                // Playback just finished — start 400ms settling period
                while mic_rx.try_recv().is_ok() {}
                let clear_ev = json!({"type": "input_audio_buffer.clear"});
                ws.send(Message::Text(clear_ev.to_string())).ok();
                unmute_at = Some(Instant::now() + Duration::from_millis(400));
            } else if Instant::now() >= unmute_at.unwrap() {
                // Settling done — drain once more and unmute
                while mic_rx.try_recv().is_ok() {}
                let clear_ev = json!({"type": "input_audio_buffer.clear"});
                ws.send(Message::Text(clear_ev.to_string())).ok();
                MIC_MUTED.store(false, Ordering::Relaxed);
                unmute_at = None;
            } else {
                // Still settling — keep draining stale mic data
                while mic_rx.try_recv().is_ok() {}
            }
        }

        // 3. Send queued mic audio to server (drop chunks while muted)
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
    PLAYBACK_IDLE.store(true, Ordering::Relaxed);
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
                // Discard audio while muted (model is speaking)
                if MIC_MUTED.load(Ordering::Relaxed) {
                    continue;
                }
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
                PLAYBACK_IDLE.store(true, Ordering::SeqCst);
                // Drain any queued audio commands
                while let Ok(cmd) = rx.try_recv() {
                    if matches!(cmd, PlayCmd::Shutdown) { return; }
                }
            }
            PlayCmd::FinishResponse => {
                // Close stdin → player finishes buffered audio → wait for exit
                drop(stdin.take());
                if let Some(ref mut p) = player {
                    let _ = p.wait(); // blocks until all buffered audio plays out
                }
                player = None;
                // NOW signal that speakers are truly silent
                PLAYBACK_IDLE.store(true, Ordering::SeqCst);
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

// ═══════════════════════════════════════════════════════════════════════════
// Live config change — apply voice/agent changes mid-session via session.update
// ═══════════════════════════════════════════════════════════════════════════

/// After sys.voice.config set is called by the model, apply relevant changes
/// to the live WebSocket session. Voice changes take effect on next response.
/// Agent changes update the instructions immediately.
fn apply_live_config_change(
    arguments_json: &str,
    session_id: Option<&str>,
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
) {
    use tungstenite::Message;

    let args: Map<String, Value> = serde_json::from_str(arguments_json).unwrap_or_default();
    let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");
    let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
    let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("");

    if action != "set" || key.is_empty() || value.is_empty() {
        return;
    }

    match key {
        "voice" => {
            // Send session.update with new voice — takes effect on next response
            let update = json!({
                "type": "session.update",
                "session": { "voice": value }
            });
            ws.send(Message::Text(update.to_string())).ok();
            eprintln!("\x1b[90m✓ Voice changed to {value}\x1b[0m");
        }
        "agent" => {
            // Rebuild instructions with new agent and send session.update
            let instructions = build_instructions(value, session_id);
            let update = json!({
                "type": "session.update",
                "session": { "instructions": instructions }
            });
            ws.send(Message::Text(update.to_string())).ok();
            eprintln!("\x1b[90m✓ Agent changed to {value}\x1b[0m");
        }
        "model" => {
            // Model is fixed at WebSocket connect time — just inform
            eprintln!("\x1b[90m✓ Model set to {value} (takes effect next session)\x1b[0m");
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tool registration — expose traits as Realtime API function-calling tools
// ═══════════════════════════════════════════════════════════════════════════

/// Traits to exclude from voice tool calling (internal/dangerous/interactive).
const TOOL_EXCLUDE: &[&str] = &[
    "sys.voice", "sys.mcp", "sys.serve", "sys.cli", "sys.cli.native", "sys.cli.wasm",
    "sys.dylib_loader", "sys.reload", "sys.release", "sys.secrets",
    "kernel.main", "kernel.dispatcher", "kernel.globals", "kernel.registry",
    "kernel.config", "kernel.plugin_api", "kernel.cli",
    "www.admin", "www.admin.deploy", "www.admin.fast_deploy",
    "www.admin.scale", "www.admin.destroy", "www.admin.save_config",
];

/// Build OpenAI Realtime API tool definitions from the trait registry.
fn build_tools() -> Vec<Value> {
    let registry = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return Vec::new(),
    };

    let mut tools: Vec<Value> = Vec::new();
    let mut entries = registry.all();
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    for entry in &entries {
        if TOOL_EXCLUDE.contains(&entry.path.as_str()) {
            continue;
        }
        // Skip www.* traits (they return HTML, not useful for voice)
        if entry.path.starts_with("www.") {
            continue;
        }
        // Skip non-callable / library traits
        if entry.kind == "library" || entry.kind == "interface" {
            continue;
        }

        let tool_name = entry.path.replace('.', "_");
        let schema = build_tool_schema(&entry.signature);

        tools.push(json!({
            "type": "function",
            "name": tool_name,
            "description": entry.description,
            "parameters": schema
        }));
    }

    tools
}

/// Build JSON Schema parameters object from a trait's signature.
fn build_tool_schema(sig: &crate::types::TraitSignature) -> Value {
    let mut properties = Map::new();
    let mut required: Vec<Value> = Vec::new();

    for param in &sig.params {
        let mut prop = match trait_type_to_schema(&param.param_type) {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        if !param.description.is_empty() {
            prop.insert("description".to_string(), json!(param.description));
        }
        properties.insert(param.name.clone(), Value::Object(prop));
        if !param.optional {
            required.push(json!(param.name));
        }
    }

    let mut schema = Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_string(), Value::Array(required));
    }
    Value::Object(schema)
}

/// Map TraitType → JSON Schema type.
fn trait_type_to_schema(tt: &crate::types::TraitType) -> Value {
    match tt {
        crate::types::TraitType::Int => json!({"type": "integer"}),
        crate::types::TraitType::Float => json!({"type": "number"}),
        crate::types::TraitType::String => json!({"type": "string"}),
        crate::types::TraitType::Bool => json!({"type": "boolean"}),
        crate::types::TraitType::Bytes => json!({"type": "string"}),
        crate::types::TraitType::List(inner) => json!({
            "type": "array",
            "items": trait_type_to_schema(inner)
        }),
        crate::types::TraitType::Map(_k, v) => json!({
            "type": "object",
            "additionalProperties": trait_type_to_schema(v)
        }),
        crate::types::TraitType::Optional(inner) => trait_type_to_schema(inner),
        crate::types::TraitType::Any => json!({"type": "string"}),
        crate::types::TraitType::Handle => json!({"type": "string"}),
        crate::types::TraitType::Null => json!({"type": "string"}),
    }
}

/// Build ordered args array from function call arguments, matching param order.
fn build_args_from_call(
    sig: &crate::types::TraitSignature,
    arguments: &Map<String, Value>,
) -> Vec<Value> {
    sig.params.iter().map(|param| {
        arguments.get(&param.name).cloned().unwrap_or(Value::Null)
    }).collect()
}

/// Dispatch a tool call: look up trait, build args, call, return result string.
fn dispatch_tool_call(tool_name: &str, arguments_json: &str) -> String {
    let trait_path = tool_name.replace('_', ".");

    let registry = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return "Registry not available".to_string(),
    };

    let entry = match registry.get(&trait_path) {
        Some(e) => e,
        None => return format!("Unknown tool: {tool_name} (trait: {trait_path})"),
    };

    let arguments: Map<String, Value> = serde_json::from_str(arguments_json)
        .unwrap_or_default();
    let args = build_args_from_call(&entry.signature, &arguments);

    match crate::dispatcher::compiled::dispatch(&trait_path, &args) {
        Some(Value::String(s)) => s,
        Some(other) => serde_json::to_string_pretty(&other).unwrap_or_default(),
        None => format!("Dispatch failed for {trait_path}"),
    }
}
