use serde_json::Value;

pub fn format_cli(result: &Value) -> String {
    let obj = match result.as_object() {
        Some(o) => o,
        None => return format!("{}\n", result),
    };

    // Trait detail mode (has "path" key) — format trait info + dispatch
    if obj.contains_key("path") {
        return format_trait_info(result);
    }

    // System status mode (has "system" key)
    format_system_status(result)
}

fn format_system_status(result: &Value) -> String {
    let mut out = String::new();
    out.push_str("\x1b[1m\x1b[97mSystem Status\x1b[0m\n\n");

    if let Some(sys) = result.get("system") {
        let os = sys.get("os").and_then(|v| v.as_str()).unwrap_or("unknown");
        let arch = sys.get("arch").and_then(|v| v.as_str()).unwrap_or("unknown");
        let build = sys.get("build_version").and_then(|v| v.as_str()).unwrap_or("?");
        out.push_str("\x1b[1mSystem\x1b[0m\n");
        out.push_str(&format!("  \x1b[90mOS:\x1b[0m      \x1b[36m{}/{}\x1b[0m\n", os, arch));
        out.push_str(&format!("  \x1b[90mBuild:\x1b[0m   \x1b[36m{}\x1b[0m\n", build));
    }

    if let Some(srv) = result.get("server") {
        let bind = srv.get("bind").and_then(|v| v.as_str()).unwrap_or("?");
        let port = srv.get("port").and_then(|v| v.as_str()).unwrap_or("?");
        let uptime = srv.get("uptime").and_then(|v| v.as_str()).unwrap_or("n/a");
        out.push_str("\n\x1b[1mServer\x1b[0m\n");
        if bind == "not running" {
            out.push_str("  \x1b[90mStatus:\x1b[0m  \x1b[33mnot running\x1b[0m\n");
        } else {
            out.push_str(&format!("  \x1b[90mListen:\x1b[0m  \x1b[32m{}:{}\x1b[0m\n", bind, port));
            out.push_str(&format!("  \x1b[90mUptime:\x1b[0m  \x1b[36m{}\x1b[0m\n", uptime));
        }
    }

    if let Some(traits) = result.get("traits") {
        let total = traits.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        out.push_str("\n\x1b[1mTraits\x1b[0m\n");
        out.push_str(&format!("  \x1b[90mTotal:\x1b[0m   \x1b[36m{}\x1b[0m\n", total));
    }

    if let Some(relay) = result.get("relay") {
        let enabled = relay.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        out.push_str("\n\x1b[1mRelay\x1b[0m\n");
        if enabled {
            let url = relay.get("url").and_then(|v| v.as_str()).unwrap_or("?");
            let code = relay.get("code").and_then(|v| v.as_str()).unwrap_or("");
            let connected = relay.get("client_connected").and_then(|v| v.as_bool()).unwrap_or(false);
            out.push_str(&format!("  \x1b[90mURL:\x1b[0m     \x1b[36m{}\x1b[0m\n", url));
            if !code.is_empty() {
                out.push_str(&format!("  \x1b[90mCode:\x1b[0m    \x1b[32m{}\x1b[0m\n", code));
            }
            let status = if connected {
                "\x1b[32mconnected\x1b[0m"
            } else {
                "\x1b[33mwaiting\x1b[0m"
            };
            out.push_str(&format!("  \x1b[90mClient:\x1b[0m  {}\n", status));
        } else {
            out.push_str("  \x1b[90mStatus:\x1b[0m  \x1b[33mdisabled\x1b[0m \x1b[90m(set via: traits config set sys.serve RELAY_URL <url>)\x1b[0m\n");
        }
    }

    out
}

fn format_trait_info(result: &Value) -> String {
    let mut out = String::new();
    let path = result.get("path").and_then(|v| v.as_str()).unwrap_or("?");
    let desc = result.get("description").and_then(|v| v.as_str()).unwrap_or("");
    let version = result.get("version").and_then(|v| v.as_str()).unwrap_or("?");
    let source = result.get("source").and_then(|v| v.as_str()).unwrap_or("?");

    out.push_str(&format!("\x1b[1m\x1b[97m{}\x1b[0m", path));
    out.push_str(&format!("  \x1b[90m{}\x1b[0m\n", version));
    if !desc.is_empty() {
        out.push_str(&format!("  {}\n", desc));
    }
    out.push_str(&format!("  \x1b[90mSource:\x1b[0m {}\n", source));

    // Dispatch info
    if let Some(dispatch) = result.get("dispatch") {
        let location = dispatch.get("location").and_then(|v| v.as_str()).unwrap_or("?");
        let browser = dispatch.get("browser_dispatch").and_then(|v| v.as_str()).unwrap_or("n/a");
        out.push_str(&format!("  \x1b[90mDispatch:\x1b[0m {}\n", location));
        out.push_str(&format!("  \x1b[90mBrowser:\x1b[0m  {}\n", browser));
    }

    // Params
    if let Some(params) = result.get("params").and_then(|v| v.as_array()) {
        if !params.is_empty() {
            out.push('\n');
            for p in params {
                let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let ptype = p.get("type").and_then(|v| v.as_str()).unwrap_or("any");
                let required = p.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
                let pdesc = p.get("description").and_then(|v| v.as_str()).unwrap_or("");
                let marker = if required { "\x1b[31m*\x1b[0m" } else { " " };
                out.push_str(&format!("  {} \x1b[36m{}\x1b[0m \x1b[90m({})\x1b[0m", marker, name, ptype));
                if !pdesc.is_empty() {
                    out.push_str(&format!("  {}", pdesc));
                }
                out.push('\n');
            }
        }
    }

    out
}
