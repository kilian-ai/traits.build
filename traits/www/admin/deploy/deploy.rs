use serde_json::Value;

// Include shared Fly API helpers
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/traits/www/admin/fly_api.rs"));

/// Deploy: restart existing machines, or create a new one if none exist.
/// If first arg is "status", returns machine info without changing anything.
/// If app has 0 machines, creates one from the latest image in the registry.
pub fn deploy(args: &[Value]) -> Value {
    let mode = args.first()
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let api = match FlyApi::new() {
        Ok(a) => a,
        Err(e) => return Value::String(format!("Error: {}", e)),
    };

    let machines = match api.list_machines() {
        Ok(m) => m,
        Err(e) => return Value::String(format!("Failed to list machines: {}", e)),
    };

    let machines_arr = match machines.as_array() {
        Some(a) => a,
        None => return Value::String("No machines found".to_string()),
    };

    // Status mode: return machine info without changing anything
    if mode == "status" {
        let infos: Vec<Value> = machines_arr.iter().map(|m| {
            serde_json::json!({
                "id": m["id"].as_str().unwrap_or("unknown"),
                "state": m["state"].as_str().unwrap_or("unknown"),
                "region": m["region"].as_str().unwrap_or("unknown"),
                "image": m.pointer("/config/image")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
            })
        }).collect();
        return serde_json::json!({
            "ok": true,
            "machines": infos
        });
    }

    if machines_arr.is_empty() {
        // No machines — create one from the latest image in Fly's registry
        return create_first_machine(&api);
    }

    let mut results = Vec::new();
    for machine in machines_arr {
        let id = machine["id"].as_str().unwrap_or("unknown");
        let state = machine["state"].as_str().unwrap_or("unknown");

        // Stop then start = restart (forces image re-pull if updated in registry)
        if state == "started" {
            match api.post(&format!("/machines/{}/stop", id), "{}") {
                Ok(_) => results.push(format!("{}: stopped", id)),
                Err(e) => results.push(format!("{}: stop failed: {}", id, e)),
            }
            // Wait briefly for stop
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        // Uncordon in case it was cordoned from a previous scale-to-0
        let _ = api.post(&format!("/machines/{}/uncordon", id), "{}");

        match api.post(&format!("/machines/{}/start", id), "{}") {
            Ok(_) => results.push(format!("{}: started", id)),
            Err(e) => results.push(format!("{}: start failed: {}", id, e)),
        }
    }

    serde_json::json!({
        "ok": true,
        "action": "deploy",
        "machines": machines_arr.len(),
        "results": results
    })
}

/// Create the first machine for the app using Fly Machines API.
fn create_first_machine(api: &FlyApi) -> Value {
    let image = format!("registry.fly.io/{}:deployment-latest", fly_app());
    let body = serde_json::json!({
        "region": "iad",
        "config": {
            "image": image,
            "env": {
                "TRAITS_PORT": "8090",
                "RUST_LOG": "info"
            },
            "services": [{
                "ports": [
                    {"port": 80, "handlers": ["http"]},
                    {"port": 443, "handlers": ["tls", "http"]}
                ],
                "protocol": "tcp",
                "internal_port": 8090,
                "autostop": "stop",
                "autostart": true
            }],
            "guest": {
                "cpu_kind": "shared",
                "cpus": 1,
                "memory_mb": 512
            }
        }
    });

    match api.post("/machines", &body.to_string()) {
        Ok(resp) => {
            let parsed: Value = serde_json::from_str(&resp).unwrap_or(Value::String(resp));
            serde_json::json!({
                "ok": true,
                "action": "create_machine",
                "machine": parsed
            })
        }
        Err(e) => serde_json::json!({
            "ok": false,
            "error": format!("Failed to create machine: {}. Run 'fly deploy' locally first to push the image.", e)
        })
    }
}
