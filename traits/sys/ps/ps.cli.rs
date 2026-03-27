use serde_json::Value;

pub fn format_cli(result: &Value) -> String {
    let obj = match result.as_object() {
        Some(o) => o,
        None => return format!("{}\n", result),
    };

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
