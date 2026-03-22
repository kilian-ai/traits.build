use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

const RUN_DIR: &str = ".run";

pub fn ps(_args: &[Value]) -> Value {
    let run_dir = Path::new(RUN_DIR);
    if !run_dir.is_dir() {
        return json!({ "ok": true, "processes": [] });
    }

    let entries = match fs::read_dir(run_dir) {
        Ok(rd) => rd,
        Err(_) => return json!({ "ok": true, "processes": [] }),
    };

    let mut processes = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let fname = match path.file_name().and_then(|f| f.to_str()) {
            Some(f) if f.ends_with(".pid") => f.to_string(),
            _ => continue,
        };
        let trait_path = fname.trim_end_matches(".pid").to_string();

        let pid_str = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let pid: u32 = match pid_str.trim().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let alive = unsafe { libc::kill(pid as i32, 0) == 0 };

        // PID file modification time as proxy for start time
        let uptime_secs = path.metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|mtime| SystemTime::now().duration_since(mtime).ok())
            .map(|d| d.as_secs_f64());

        // Read RSS memory from sysctl on macOS (or /proc on Linux)
        let memory_mb = get_rss_mb(pid);

        let mut proc_info = json!({
            "trait": trait_path,
            "pid": pid,
            "alive": alive,
        });

        if let Some(up) = uptime_secs {
            proc_info["uptime"] = json!(format_uptime(up));
            proc_info["uptime_secs"] = json!(up.round() as u64);
        }
        if let Some(mb) = memory_mb {
            proc_info["memory_mb"] = json!((mb * 100.0).round() / 100.0);
        }
        proc_info["pid_file"] = json!(path.to_string_lossy());

        processes.push(proc_info);
    }

    // Sort by trait path
    processes.sort_by(|a, b| {
        let ta = a["trait"].as_str().unwrap_or("");
        let tb = b["trait"].as_str().unwrap_or("");
        ta.cmp(tb)
    });

    json!({
        "ok": true,
        "count": processes.len(),
        "processes": processes,
    })
}

fn format_uptime(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

/// Get resident set size in MB for a process.
/// Uses sysctl on macOS, /proc on Linux.
fn get_rss_mb(pid: u32) -> Option<f64> {
    #[cfg(target_os = "macos")]
    {
        // Use ps command as a portable fallback
        let output = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let rss_kb: f64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;
        Some(rss_kb / 1024.0)
    }
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
                return Some(kb / 1024.0);
            }
        }
        None
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}
