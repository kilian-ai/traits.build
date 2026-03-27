use serde_json::Value;

pub fn format_cli(result: &Value) -> String {
    let obj = match result.as_object() {
        Some(o) => o,
        None => return format!("{}\n", result),
    };

    let runtime = obj.get("runtime").and_then(|v| v.as_str()).unwrap_or("unknown");

    if runtime == "wasm" {
        return format_wasm(obj);
    }

    // Fall through to native format for non-WASM results
    let processes = match obj.get("processes").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return format!("{}\n", result),
    };

    if processes.is_empty() {
        return "No background traits running.\n".to_string();
    }

    let mut out = String::new();
    out.push_str(&format!("{:<25} {:>7}  {:>6}  {:>10}  {}\n",
        "TRAIT", "PID", "MEM", "UPTIME", "STATUS"));
    out.push_str(&format!("{}\n", "─".repeat(65)));

    for p in processes {
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

        out.push_str(&format!("  \x1b[36mRuntime\x1b[0m      wasm32 (browser)\n"));
        out.push_str(&format!("  \x1b[36mThreading\x1b[0m    {}\n", threading));
        out.push_str(&format!("  \x1b[36mCallable\x1b[0m     {} traits (WASM-local)\n", callable));
        out.push_str(&format!("  \x1b[36mRegistered\x1b[0m   {} traits (total)\n", registered));
        out.push_str(&format!("  \x1b[36mREST-only\x1b[0m    {} traits (need helper)\n", registered as i64 - callable as i64));

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

    out.push_str(&format!("\n\x1b[33mOS Processes\x1b[0m  n/a (no threads in WASM)\n"));

    out
}
