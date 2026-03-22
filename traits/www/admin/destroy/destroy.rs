use serde_json::Value;

// Include shared Fly API helpers
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/traits/www/admin/fly_api.rs"));

/// Destroy all Fly.io machines for the app. This is destructive — machines are deleted.
/// The app itself is preserved (can create new machines afterwards).
pub fn destroy(_args: &[Value]) -> Value {
    let api = match FlyApi::new() {
        Ok(a) => a,
        Err(e) => return serde_json::json!({"error": e}),
    };

    let machines = match api.list_machines() {
        Ok(m) => m,
        Err(e) => return serde_json::json!({"error": format!("Failed to list machines: {}", e)}),
    };

    let machines_arr = match machines.as_array() {
        Some(a) => a,
        None => return serde_json::json!({"error": "No machines array returned"}),
    };

    if machines_arr.is_empty() {
        return serde_json::json!({"ok": true, "action": "destroy", "message": "No machines to destroy"});
    }

    let mut results = Vec::new();
    for machine in machines_arr {
        let id = machine["id"].as_str().unwrap_or("unknown");
        let state = machine["state"].as_str().unwrap_or("unknown");

        // Stop first if running (required before delete)
        if state == "started" || state == "starting" {
            let _ = api.post(&format!("/machines/{}/stop", id), "{}");
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        // Force-delete the machine
        match api.delete(&format!("/machines/{}?force=true", id)) {
            Ok(_) => results.push(format!("{}: destroyed", id)),
            Err(e) => results.push(format!("{}: destroy failed: {}", id, e)),
        }
    }

    serde_json::json!({
        "ok": true,
        "action": "destroy",
        "machines_destroyed": results.len(),
        "results": results
    })
}
