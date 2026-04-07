use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Instant;

const VOICE_INSTRUCTIONS: &str = include_str!("realtime_instructions.md");

/// Global flag for SIGINT handling. Pub so sys.voice.quit can stop the session.
pub static VOICE_RUNNING: AtomicBool = AtomicBool::new(false);
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
    let default_model =
        read_voice_pref("model").unwrap_or_else(|| "gpt-4o-mini-realtime-preview".into());
    let default_agent = read_voice_pref("agent").unwrap_or_default();

    let voice_name = args
        .first()
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_voice);

    let model = args
        .get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_model);

    let agent = args
        .get(2)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_agent);

    let session_id = args
        .get(3)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // Resolve API key
    let api_key = match resolve_api_key() {
        Some(k) => k,
        None => {
            return json!({"ok": false, "error": "OpenAI API key not found. Set via: traits call sys.secrets set openai_api_key <key>"})
        }
    };

    // Verify sox is available (provides `rec` and `play`)
    if !which("rec") {
        return json!({"ok": false, "error": "sox not found. Install: brew install sox"});
    }

    // Build combined instructions via sys.voice.instruct build
    let instructions = build_instructions_via_trait(agent, session_id.as_deref());

    match realtime_session(
        &api_key,
        model,
        voice_name,
        &instructions,
        session_id.as_deref(),
    ) {
        Ok(turns) => json!({"ok": true, "turns": turns}),
        Err(e) => json!({"ok": false, "error": e}),
    }
}

/// Delegate instruction assembly to sys.voice.instruct build (single source of truth).
fn build_instructions_via_trait(agent: &str, session_id: Option<&str>) -> String {
    let sid = session_id.map(|s| json!(s)).unwrap_or(Value::Null);
    if let Some(result) = kernel_logic::platform::dispatch(
        "sys.voice.instruct",
        &[json!("build"), json!(agent), sid],
    ) {
        if let Some(s) = result.get("instructions").and_then(|v| v.as_str()) {
            return s.to_string();
        }
    }
    // Fallback: compiled-in default
    VOICE_INSTRUCTIONS.to_string()
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
    .and_then(|r| {
        r.get("value")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    })
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

fn realtime_session(
    api_key: &str,
    model: &str,
    voice_name: &str,
    instructions: &str,
    session_id: Option<&str>,
) -> Result<u32, String> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    use std::time::Duration;
    use tungstenite::client::IntoClientRequest;
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::{connect, Message};

    // ── Connect ──
    let url = format!("wss://api.openai.com/v1/realtime?model={}", model);
    eprintln!("\x1b[90mConnecting to {model}…\x1b[0m");

    let mut request = url
        .into_client_request()
        .map_err(|e| format!("Build request: {e}"))?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", api_key)
            .parse()
            .map_err(|e| format!("Auth header: {e}"))?,
    );
    request.headers_mut().insert(
        "OpenAI-Beta",
        "realtime=v1"
            .parse()
            .map_err(|e| format!("Beta header: {e}"))?,
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

    // ── Configure session with tools via sys.voice.tools (single source of truth) ──
    let tools: Vec<Value> = kernel_logic::platform::dispatch(
        "sys.voice.tools",
        &[json!("")],
    )
    .and_then(|r| r.get("tools").and_then(|v| v.as_array()).cloned())
    .unwrap_or_default();
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
            tls.get_ref()
                .set_read_timeout(Some(Duration::from_millis(20)))
                .ok();
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
        libc::signal(
            libc::SIGINT,
            sigint_handler as *const () as libc::sighandler_t,
        )
    };

    // ── Background tool channel ──
    let (bg_tool_tx, bg_tool_rx) = mpsc::channel::<BgToolMsg>();

    // ── Print UI ──
    eprintln!("\x1b[96m\x1b[1mRealtime voice chat\x1b[0m \x1b[90m(model: {model}, voice: {voice_name}, {tool_count} tools)\x1b[0m");
    eprintln!("\x1b[90mSpeak naturally. Press Ctrl+C to stop.\x1b[0m\n");

    // ── Main event loop ──
    let mut turns = 0u32;
    let mut unmute_at: Option<Instant> = None;
    let mut glow_proc: Option<std::process::Child> = None;

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
                                        &[
                                            json!("append"),
                                            json!(sid),
                                            json!("user"),
                                            json!(trimmed),
                                        ],
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
                                        &[
                                            json!("append"),
                                            json!(sid),
                                            json!("assistant"),
                                            json!(trimmed),
                                        ],
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
                        let arguments =
                            ev.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");

                        eprintln!("\x1b[93m⚡ {func_name}\x1b[0m");

                        if func_name == "llm_prompt_acp" {
                            // ── Background ACP dispatch — keep voice interactive ──
                            let bg_tx = bg_tool_tx.clone();
                            let args_owned = arguments.to_string();

                            // Immediately ack so the model can keep talking
                            ws.send(Message::Text(json!({
                                "type": "conversation.item.create",
                                "item": {
                                    "type": "function_call_output",
                                    "call_id": call_id,
                                    "output": "Coding agent started in background. The user can see streaming output in the terminal. I will inject the final result when it completes — you can continue the conversation."
                                }
                            }).to_string())).ok();
                            ws.send(Message::Text(
                                json!({"type": "response.create"}).to_string(),
                            ))
                            .ok();

                            eprintln!("\x1b[90m--- ACP agent working in background ---\x1b[0m");
                            std::thread::spawn(move || {
                                dispatch_acp_background(&args_owned, &bg_tx);
                            });
                        } else if func_name == "sys_voice_quit" {
                            // ── Voice quit — stop the session gracefully ──
                            eprintln!("\x1b[90mVoice quit requested by model\x1b[0m");
                            VOICE_RUNNING.store(false, Ordering::Relaxed);
                            ws.send(Message::Text(json!({
                                "type": "conversation.item.create",
                                "item": {
                                    "type": "function_call_output",
                                    "call_id": call_id,
                                    "output": r#"{"ok":true,"action":"quit","message":"Voice session ending."}"#
                                }
                            }).to_string())).ok();
                        } else {
                            // ── Synchronous dispatch for fast tools ──
                            let result = dispatch_tool_call(func_name, arguments);

                            // Truncate very long results for voice context
                            let output = if result.len() > 2000 {
                                let mut end = 2000;
                                while !result.is_char_boundary(end) {
                                    end -= 1;
                                }
                                format!("{}…(truncated)", &result[..end])
                            } else {
                                result
                            };

                            // If the model changed voice preferences, apply live
                            if func_name == "sys_voice_config" {
                                apply_live_config_change(arguments, session_id, &mut ws);
                            }

                            // If the model changed instructions, rebuild and update session
                            if func_name == "sys_voice_instruct" {
                                let new_instructions = build_instructions_via_trait(
                                    &read_voice_pref("agent").unwrap_or_default(),
                                    session_id,
                                );
                                ws.send(Message::Text(
                                    json!({
                                        "type": "session.update",
                                        "session": { "instructions": new_instructions }
                                    })
                                    .to_string(),
                                ))
                                .ok();
                            }

                            // If the model added/removed a memory note, rebuild and update session
                            if func_name == "sys_voice_memory" {
                                let new_instructions = build_instructions_via_trait(
                                    &read_voice_pref("agent").unwrap_or_default(),
                                    session_id,
                                );
                                ws.send(Message::Text(
                                    json!({
                                        "type": "session.update",
                                        "session": { "instructions": new_instructions }
                                    })
                                    .to_string(),
                                ))
                                .ok();
                            }

                            // Send function call output back to the model
                            ws.send(Message::Text(
                                json!({
                                    "type": "conversation.item.create",
                                    "item": {
                                        "type": "function_call_output",
                                        "call_id": call_id,
                                        "output": output
                                    }
                                })
                                .to_string(),
                            ))
                            .ok();

                            // Ask model to continue responding (with audio)
                            ws.send(Message::Text(
                                json!({"type": "response.create"}).to_string(),
                            ))
                            .ok();
                        }
                    }

                    // ── Error from server ──
                    "error" => {
                        let msg = ev
                            .pointer("/error/message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown error");
                        eprintln!("\x1b[31m✗ {msg}\x1b[0m");
                        if msg.contains("auth") || msg.contains("key") || msg.contains("quota") {
                            VOICE_RUNNING.store(false, Ordering::Relaxed);
                        }
                    }

                    // Lifecycle events we can ignore
                    "response.created"
                    | "response.done"
                    | "response.output_item.added"
                    | "response.output_item.done"
                    | "response.content_part.added"
                    | "response.content_part.done"
                    | "response.audio_transcript.delta"
                    | "response.function_call_arguments.delta"
                    | "input_audio_buffer.speech_stopped"
                    | "input_audio_buffer.committed"
                    | "conversation.item.created"
                    | "rate_limits.updated" => {}

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

        // 3. Check background tool results
        while let Ok(msg) = bg_tool_rx.try_recv() {
            match msg {
                BgToolMsg::Chunk(text) => {
                    // Lazy-start glow for markdown rendering
                    if glow_proc.is_none() {
                        glow_proc = std::process::Command::new("glow")
                            .args(["--style", "dark", "-"])
                            .stdin(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::inherit())
                            .stdout(std::process::Stdio::inherit())
                            .spawn()
                            .ok();
                    }
                    if let Some(ref mut proc) = glow_proc {
                        if let Some(ref mut stdin) = proc.stdin {
                            use std::io::Write;
                            let _ = stdin.write_all(text.as_bytes());
                            let _ = stdin.flush();
                        }
                    } else {
                        // Fallback if glow not available
                        eprint!("{}", text);
                    }
                }
                BgToolMsg::Done { result } => {
                    // Close glow stdin so it renders and exits
                    if let Some(mut proc) = glow_proc.take() {
                        drop(proc.stdin.take());
                        let _ = proc.wait();
                    }
                    eprintln!("\n\x1b[90m--- ACP agent done ---\x1b[0m");
                    let truncated = if result.len() > 2000 {
                        let mut end = 2000;
                        while !result.is_char_boundary(end) {
                            end -= 1;
                        }
                        format!("{}…(truncated)", &result[..end])
                    } else {
                        result
                    };
                    // Inject result into conversation so 4o sees the coding agent output
                    ws.send(Message::Text(json!({
                        "type": "conversation.item.create",
                        "item": {
                            "type": "message",
                            "role": "user",
                            "content": [{
                                "type": "input_text",
                                "text": format!("[Background coding agent completed]\n{}", truncated)
                            }]
                        }
                    }).to_string())).ok();
                    // Ask model to acknowledge/summarize the result with audio
                    ws.send(Message::Text(
                        json!({"type": "response.create"}).to_string(),
                    ))
                    .ok();
                }
            }
        }

        // 4. Send queued mic audio to server (drop chunks while muted)
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
            if sent > 10 {
                break;
            } // Don't block too long on sends
        }
    }

    // ── Cleanup ──
    VOICE_RUNNING.store(false, Ordering::Relaxed);
    MIC_MUTED.store(false, Ordering::Relaxed);
    PLAYBACK_IDLE.store(true, Ordering::Relaxed);
    unsafe {
        libc::signal(libc::SIGINT, prev_handler);
    }
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
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = match Command::new("rec")
        .args([
            "-q", // suppress progress
            "-t", "raw", // raw PCM output
            "-r", "24000", // 24 kHz (Realtime API requirement)
            "-c", "1", // mono
            "-e", "signed", // signed integer
            "-b", "16", // 16-bit
            "-",  // output to stdout
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
    use std::io::Write;
    use std::process::{Child, ChildStdin, Command, Stdio};

    let mut player: Option<Child> = None;
    let mut stdin: Option<ChildStdin> = None;

    fn start_player() -> Option<(Child, ChildStdin)> {
        let mut child = Command::new("play")
            .args([
                "-q", // suppress progress
                "-t", "raw", // raw PCM input
                "-r", "24000", // 24 kHz
                "-c", "1", // mono
                "-e", "signed", // signed integer
                "-b", "16", // 16-bit
                "-",  // read from stdin
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
                    if matches!(cmd, PlayCmd::Shutdown) {
                        return;
                    }
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
            let instructions = build_instructions_via_trait(value, session_id);
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
// Tool dispatch — args assembly + trait call
// Tool list building has moved to sys.voice.tools (WASM-callable, shared with
// the browser WebRTC session so both always use the same exclusion list and
// schema generation).
// ═══════════════════════════════════════════════════════════════════════════

/// Build ordered args array from function call arguments, matching param order.
fn build_args_from_call(
    sig: &crate::types::TraitSignature,
    arguments: &Map<String, Value>,
) -> Vec<Value> {
    sig.params
        .iter()
        .map(|param| arguments.get(&param.name).cloned().unwrap_or(Value::Null))
        .collect()
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

    let arguments: Map<String, Value> = serde_json::from_str(arguments_json).unwrap_or_default();
    let args = build_args_from_call(&entry.signature, &arguments);

    match crate::dispatcher::compiled::dispatch(&trait_path, &args) {
        Some(Value::String(s)) => s,
        Some(other) => serde_json::to_string_pretty(&other).unwrap_or_default(),
        None => format!("Dispatch failed for {trait_path}"),
    }
}

/// Messages from background ACP tool dispatch.
enum BgToolMsg {
    /// A streaming text chunk from the background tool.
    Chunk(String),
    /// The tool completed with full result.
    Done { result: String },
}

/// Run ACP dispatch in background thread, streaming chunks via channel.
fn dispatch_acp_background(arguments_json: &str, tx: &mpsc::Sender<BgToolMsg>) {
    let arguments: Map<String, Value> = serde_json::from_str(arguments_json).unwrap_or_default();

    // Build args array matching llm.prompt.acp signature: [prompt, agent?, cwd?, auto_approve?, model?, context?]
    let prompt = arguments.get("prompt").cloned().unwrap_or(Value::Null);
    let agent = arguments.get("agent").cloned().unwrap_or(Value::Null);
    let cwd = arguments.get("cwd").cloned().unwrap_or(Value::Null);
    let auto_approve = arguments
        .get("auto_approve")
        .cloned()
        .unwrap_or(Value::Null);
    let model = arguments.get("model").cloned().unwrap_or(Value::Null);
    let context = arguments.get("context").cloned().unwrap_or(Value::Null);
    let args = vec![prompt, agent, cwd, auto_approve, model, context];

    let mut full_response = String::new();
    let result = crate::dispatcher::compiled::acp::acp_proxy_dispatch_streaming(
        &args,
        &mut |chunk: &str| {
            full_response.push_str(chunk);
            tx.send(BgToolMsg::Chunk(chunk.to_string())).ok();
        },
    );

    let final_result = if !full_response.is_empty() {
        full_response
    } else if let Some(text) = result.get("text").and_then(|v| v.as_str()) {
        text.to_string()
    } else {
        serde_json::to_string_pretty(&result).unwrap_or_default()
    };

    tx.send(BgToolMsg::Done {
        result: final_result,
    })
    .ok();
}
