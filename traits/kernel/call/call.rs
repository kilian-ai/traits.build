use serde_json::Value;

/// Trait entry point: call(trait_path, args)
///
/// Dispatches to another trait by dot-notation path.
/// Accepts both dot and underscore notation (e.g. skills.spotify.pause or skills_spotify_pause).
pub fn call(args: &[Value]) -> Value {
    let raw_path = args.first().and_then(|v| v.as_str()).unwrap_or("");
    // Normalize underscore notation to dot notation (LLMs sometimes use underscores)
    let trait_path = if raw_path.contains('_') && !raw_path.contains('.') {
        raw_path.replace('_', ".")
    } else {
        raw_path.to_string()
    };
    let trait_path = trait_path.as_str();
    let call_args = args
        .get(1)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if trait_path.is_empty() {
        return serde_json::json!({ "error": "trait_path is required" });
    }

    let result = kernel_logic::platform::dispatch(trait_path, &call_args);

    match result {
        Some(v) => v,
        None => serde_json::json!({
            "error": format!("Trait '{}' not found", trait_path)
        }),
    }
}
