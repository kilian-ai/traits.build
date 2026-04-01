use serde_json::{json, Value};

/// Voice session control — mute, unmute, toggle, start, stop, and query status.
///
/// Returns action descriptors for the JS bridge to execute via CustomEvent.
/// The SPA shell listens for `traits-voice-control` events and performs the actual
/// mute/unmute/stop operations on the WebRTC voice stream.
pub fn control(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("status");

    match action {
        "mute" | "unmute" | "toggle" | "start" | "stop" | "status" => {
            json!({
                "ok": true,
                "voice_control_action": action
            })
        }
        _ => json!({
            "ok": false,
            "error": format!("Unknown action: {}. Use: mute, unmute, toggle, start, stop, status", action)
        }),
    }
}
