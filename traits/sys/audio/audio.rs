use serde_json::{json, Value};

/// sys.audio — WebAudio API bridge for generating and playing sounds in the browser.
///
/// Returns action descriptors that the JS bridge in the SDK executes via the WebAudio API.
/// Supports tone generation (oscillators), noise, sequences, drum patterns, and effects.
///
/// Actions:
///   tone       — Play a tone: frequency (Hz), duration (s), waveform, volume
///   sequence   — Play a sequence of notes: array of {freq, dur, wave} objects
///   drum       — Play a drum pattern: kick/snare/hihat with BPM and pattern string
///   noise      — Generate noise: white/pink/brown, duration, volume
///   chord      — Play a chord: array of frequencies simultaneously
///   sweep      — Frequency sweep: start_freq → end_freq over duration
///   stop       — Stop all playing audio
///   status     — Check if AudioContext is active
pub fn audio(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("status");

    match action {
        "tone" => {
            let freq = args.get(1).and_then(|v| v.as_f64()).unwrap_or(440.0);
            let duration = args.get(2).and_then(|v| v.as_f64()).unwrap_or(0.5);
            let waveform = args.get(3).and_then(|v| v.as_str()).unwrap_or("sine");
            let volume = args.get(4).and_then(|v| v.as_f64()).unwrap_or(0.3);
            // Clamp values to safe ranges
            let freq = freq.clamp(20.0, 20000.0);
            let duration = duration.clamp(0.01, 30.0);
            let volume = volume.clamp(0.0, 1.0);
            let waveform = match waveform {
                "sine" | "square" | "sawtooth" | "triangle" => waveform,
                _ => "sine",
            };
            json!({
                "ok": true,
                "audio_action": "tone",
                "freq": freq,
                "duration": duration,
                "waveform": waveform,
                "volume": volume
            })
        }
        "sequence" => {
            // args[1] = JSON string or array of note objects [{freq, dur, wave}]
            let notes = args.get(1).cloned().unwrap_or(json!([]));
            let notes = if let Some(s) = notes.as_str() {
                serde_json::from_str(s).unwrap_or(json!([]))
            } else {
                notes
            };
            let tempo = args.get(2).and_then(|v| v.as_f64()).unwrap_or(120.0).clamp(20.0, 300.0);
            let volume = args.get(3).and_then(|v| v.as_f64()).unwrap_or(0.3).clamp(0.0, 1.0);
            json!({
                "ok": true,
                "audio_action": "sequence",
                "notes": notes,
                "tempo": tempo,
                "volume": volume
            })
        }
        "drum" => {
            // args[1] = pattern string like "k..s..k.ks..s..." (k=kick, s=snare, h=hihat, .=rest)
            let pattern = args.get(1).and_then(|v| v.as_str()).unwrap_or("k..s..k.ks..s...");
            let bpm = args.get(2).and_then(|v| v.as_f64()).unwrap_or(120.0).clamp(20.0, 300.0);
            let loops = args.get(3).and_then(|v| v.as_u64()).unwrap_or(2).min(16);
            let volume = args.get(4).and_then(|v| v.as_f64()).unwrap_or(0.4).clamp(0.0, 1.0);
            json!({
                "ok": true,
                "audio_action": "drum",
                "pattern": pattern,
                "bpm": bpm,
                "loops": loops,
                "volume": volume
            })
        }
        "noise" => {
            let noise_type = args.get(1).and_then(|v| v.as_str()).unwrap_or("white");
            let duration = args.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0).clamp(0.01, 30.0);
            let volume = args.get(3).and_then(|v| v.as_f64()).unwrap_or(0.2).clamp(0.0, 1.0);
            let noise_type = match noise_type {
                "white" | "pink" | "brown" => noise_type,
                _ => "white",
            };
            json!({
                "ok": true,
                "audio_action": "noise",
                "noise_type": noise_type,
                "duration": duration,
                "volume": volume
            })
        }
        "chord" => {
            let freqs = args.get(1).cloned().unwrap_or(json!([]));
            let freqs = if let Some(s) = freqs.as_str() {
                serde_json::from_str(s).unwrap_or(json!([]))
            } else {
                freqs
            };
            let duration = args.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0).clamp(0.01, 30.0);
            let waveform = args.get(3).and_then(|v| v.as_str()).unwrap_or("sine");
            let volume = args.get(4).and_then(|v| v.as_f64()).unwrap_or(0.3).clamp(0.0, 1.0);
            let waveform = match waveform {
                "sine" | "square" | "sawtooth" | "triangle" => waveform,
                _ => "sine",
            };
            json!({
                "ok": true,
                "audio_action": "chord",
                "frequencies": freqs,
                "duration": duration,
                "waveform": waveform,
                "volume": volume
            })
        }
        "sweep" => {
            let start_freq = args.get(1).and_then(|v| v.as_f64()).unwrap_or(200.0).clamp(20.0, 20000.0);
            let end_freq = args.get(2).and_then(|v| v.as_f64()).unwrap_or(2000.0).clamp(20.0, 20000.0);
            let duration = args.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0).clamp(0.01, 30.0);
            let waveform = args.get(4).and_then(|v| v.as_str()).unwrap_or("sine");
            let volume = args.get(5).and_then(|v| v.as_f64()).unwrap_or(0.3).clamp(0.0, 1.0);
            let waveform = match waveform {
                "sine" | "square" | "sawtooth" | "triangle" => waveform,
                _ => "sine",
            };
            json!({
                "ok": true,
                "audio_action": "sweep",
                "start_freq": start_freq,
                "end_freq": end_freq,
                "duration": duration,
                "waveform": waveform,
                "volume": volume
            })
        }
        "stop" => {
            json!({
                "ok": true,
                "audio_action": "stop"
            })
        }
        "status" => {
            json!({
                "ok": true,
                "audio_action": "status"
            })
        }
        _ => json!({"ok": false, "error": format!("Unknown action: {}. Use: tone, sequence, drum, noise, chord, sweep, stop, status", action)}),
    }
}
