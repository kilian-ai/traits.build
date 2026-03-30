use serde_json::{json, Value};
use std::sync::atomic::Ordering;

/// sys.voice.quit — gracefully end the active voice session.
///
/// The model can call this to stop the voice chat on its own.
/// Sets VOICE_RUNNING to false, which causes the event loop to exit cleanly.
pub fn voice_quit(_args: &[Value]) -> Value {
    crate::dispatcher::compiled::voice::VOICE_RUNNING.store(false, Ordering::SeqCst);
    json!({ "ok": true, "action": "quit", "message": "Voice session ending." })
}
