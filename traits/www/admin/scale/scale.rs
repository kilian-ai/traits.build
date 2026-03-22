use serde_json::Value;

// Include shared Fly API helpers
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/traits/www/admin/fly_api.rs"));

/// Scale Fly.io machines: 0 = stop all running machines, 1+ = start stopped machines.
pub fn scale(args: &[Value]) -> Value {
    let count = args.first()
        .and_then(|v| v.as_i64())
        .unwrap_or(1) as i32;

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

    let mut results = Vec::new();

    if count == 0 {
        // Cordon + stop all running machines.
        // Cordon removes machine from Fly proxy, preventing auto_start_machines
        // from restarting it when health checks or other requests arrive.
        for machine in machines_arr {
            let id = machine["id"].as_str().unwrap_or("unknown");
            let state = machine["state"].as_str().unwrap_or("unknown");
            if state == "started" || state == "starting" {
                // Cordon first: remove from proxy load balancer
                match api.post(&format!("/machines/{}/cordon", id), "{}") {
                    Ok(_) => results.push(format!("{}: cordoned", id)),
                    Err(e) => results.push(format!("{}: cordon failed (continuing): {}", id, e)),
                }
                // Then stop
                match api.post(&format!("/machines/{}/stop", id), "{}") {
                    Ok(_) => results.push(format!("{}: stopped", id)),
                    Err(e) => results.push(format!("{}: stop failed: {}", id, e)),
                }
            } else {
                results.push(format!("{}: already {}", id, state));
            }
        }
    } else {
        // Start stopped machines (up to `count`)
        let mut started = 0;
        for machine in machines_arr {
            if started >= count { break; }
            let id = machine["id"].as_str().unwrap_or("unknown");
            let state = machine["state"].as_str().unwrap_or("unknown");
            if state == "stopped" || state == "created" {
                // Uncordon first: add back to proxy load balancer
                match api.post(&format!("/machines/{}/uncordon", id), "{}") {
                    Ok(_) => results.push(format!("{}: uncordoned", id)),
                    Err(e) => results.push(format!("{}: uncordon failed (continuing): {}", id, e)),
                }
                // Then start
                match api.post(&format!("/machines/{}/start", id), "{}") {
                    Ok(_) => {
                        results.push(format!("{}: started", id));
                        started += 1;
                    }
                    Err(e) => results.push(format!("{}: start failed: {}", id, e)),
                }
            } else if state == "started" {
                // Uncordon in case it was cordoned from a previous stop
                let _ = api.post(&format!("/machines/{}/uncordon", id), "{}");
                results.push(format!("{}: already running (uncordoned)", id));
                started += 1;
            }
        }

        // If we still need more machines, create them by cloning the first machine's config
        if started < count {
            if let Some(template) = machines_arr.first() {
                let config = &template["config"];
                let region = template["region"].as_str().unwrap_or("ord");
                while started < count {
                    let body = serde_json::json!({
                        "region": region,
                        "config": config
                    });
                    match api.post("/machines", &body.to_string()) {
                        Ok(resp) => {
                            let parsed: Value = serde_json::from_str(&resp).unwrap_or_default();
                            let new_id = parsed["id"].as_str().unwrap_or("new");
                            results.push(format!("{}: created", new_id));
                            started += 1;
                        }
                        Err(e) => {
                            results.push(format!("create failed: {}", e));
                            break;
                        }
                    }
                }
            }
        }
    }

    serde_json::json!({
        "ok": true,
        "action": if count == 0 { "scale_down" } else { "scale_up" },
        "target": count,
        "results": results
    })
}
