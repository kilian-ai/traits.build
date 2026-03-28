use serde_json::Value;

pub fn format_cli(result: &Value) -> String {
    let obj = match result.as_object() {
        Some(o) => o,
        None => return format!("{}\n", result),
    };

    // WASM runtime — show kernel state instead of process table
    if obj.get("runtime").and_then(|v| v.as_str()) == Some("wasm") {
        return format_wasm(obj);
    }

    // Native — show process table
    let processes = match obj.get("processes").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return format!("{}\n", result),
    };

    if processes.is_empty() {
        return "No background traits running.\n".to_string();
    }

    // Split into PID-file processes and in-memory registry tasks
    let mut pid_procs = Vec::new();
    let mut reg_tasks = Vec::new();
    for p in processes {
        if p.get("source").and_then(|v| v.as_str()) == Some("registry") {
            reg_tasks.push(p);
        } else {
            pid_procs.push(p);
        }
    }

    let mut out = String::new();

    // PID-file processes (OS-level background traits)
    if !pid_procs.is_empty() {
        out.push_str(&format!("{:<25} {:>7}  {:>6}  {:>10}  {}\n",
            "TRAIT", "PID", "MEM", "UPTIME", "STATUS"));
        out.push_str(&format!("{}\n", "─".repeat(65)));

        for p in &pid_procs {
            let name = p["trait"].as_str().unwrap_or("?");
            let pid = p["pid"].as_u64().unwrap_or(0);
            let alive = p["alive"].as_bool().unwrap_or(false);
            let uptime = p["uptime"].as_str().unwrap_or("-");
            let mem = p.get("memory_mb")
                .and_then(|v| v.as_f64())
                .map(|m| format!("{:.1}MB", m))
                .unwrap_or_else(|| "-".into());
            let status = if alive { "\x1b[32m●\x1b[0m running" } else { "\x1b[31m○\x1b[0m dead" };

            out.push_str(&format!("{:<25} {:>7}  {:>6}  {:>10}  {}\n",
                name, pid, mem, uptime, status));
        }
    }

    // In-memory registry tasks (services, workers)
    if !reg_tasks.is_empty() {
        if !pid_procs.is_empty() { out.push('\n'); }
        out.push_str(&format!("{:<20} {:<10} {:>10}  {:<18} {}\n",
            "SERVICE", "TYPE", "UPTIME", "STATUS", "DETAIL"));
        out.push_str(&format!("{}\n", "─".repeat(72)));

        for p in &reg_tasks {
            let name = p["name"].as_str().unwrap_or("?");
            let ttype = p["type"].as_str().unwrap_or("?");
            let status = p["status"].as_str().unwrap_or("?");
            let uptime = p["uptime"].as_str().unwrap_or("-");
            let detail = p["detail"].as_str().unwrap_or("");
            let status_colored = match status {
                "running" => format!("\x1b[32m●\x1b[0m {}", status),
                "idle" => format!("\x1b[33m○\x1b[0m {}", status),
                _ => format!("\x1b[31m○\x1b[0m {}", status),
            };
            out.push_str(&format!("{:<20} {:<10} {:>10}  {:<18} {}\n",
                name, ttype, uptime, status_colored, detail));
        }
    }

    out
}

fn format_wasm(obj: &serde_json::Map<String, Value>) -> String {
    let mut out = String::new();

    out.push_str("\x1b[1;97mWASM Runtime Status\x1b[0m\n");
    out.push_str(&format!("{}\n", "─".repeat(50)));

    if let Some(wasm) = obj.get("wasm").and_then(|v| v.as_object()) {
        let callable = wasm.get("callable").and_then(|v| v.as_u64()).unwrap_or(0);
        let registered = wasm.get("registered").and_then(|v| v.as_u64()).unwrap_or(0);
        let threading = wasm.get("threading").and_then(|v| v.as_str()).unwrap_or("unknown");
        let helper = wasm.get("helper_connected").and_then(|v| v.as_bool()).unwrap_or(false);

        out.push_str(&format!("  \x1b[36mRuntime\x1b[0m      wasm32 (browser)\n"));
        out.push_str(&format!("  \x1b[36mThreading\x1b[0m    {}\n", threading));
        out.push_str(&format!("  \x1b[36mCallable\x1b[0m     {} traits (WASM-local)\n", callable));
        out.push_str(&format!("  \x1b[36mRegistered\x1b[0m   {} traits (total)\n", registered));
        out.push_str(&format!("  \x1b[36mREST-only\x1b[0m    {} traits (need helper)\n", registered as i64 - callable as i64));
        out.push_str(&format!("  \x1b[36mHelper\x1b[0m       {}\n",
            if helper { "\x1b[32mconnected\x1b[0m" } else { "\x1b[33mnot connected\x1b[0m" }));

        out.push_str(&format!("\n\x1b[1;97mDispatch Cascade\x1b[0m\n"));
        out.push_str(&format!("{}\n", "─".repeat(50)));
        if let Some(cascade) = wasm.get("dispatch_cascade").and_then(|v| v.as_array()) {
            for step in cascade {
                if let Some(s) = step.as_str() {
                    out.push_str(&format!("  {}\n", s));
                }
            }
        }
    }

    // Browser tasks/services
    if let Some(procs) = obj.get("processes").and_then(|v| v.as_array()) {
        if !procs.is_empty() {
            out.push_str(&format!("\n\x1b[1;97mBackground Tasks\x1b[0m\n"));
            out.push_str(&format!("{}\n", "─".repeat(50)));
            out.push_str(&format!("  {:<16} {:<10} {:<10} {}\n",
                "NAME", "TYPE", "STATUS", "DETAIL"));

            for p in procs {
                let name = p["name"].as_str().unwrap_or("?");
                let ttype = p["type"].as_str().unwrap_or("?");
                let status = p["status"].as_str().unwrap_or("?");
                let detail = p["detail"].as_str().unwrap_or("");
                let status_colored = match status {
                    "running" => format!("\x1b[32m●\x1b[0m {}", status),
                    "idle" => format!("\x1b[33m○\x1b[0m {}", status),
                    _ => format!("\x1b[31m○\x1b[0m {}", status),
                };
                out.push_str(&format!("  {:<16} {:<10} {:<18} {}\n",
                    name, ttype, status_colored, detail));
            }
        }
    }

    if obj.get("processes").and_then(|v| v.as_array()).map_or(true, |a| a.is_empty()) {
        out.push_str(&format!("\n\x1b[33mNo background tasks registered.\x1b[0m\n"));
    }

    out
}
