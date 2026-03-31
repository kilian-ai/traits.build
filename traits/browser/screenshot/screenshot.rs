use serde_json::{json, Value};
use std::process::Command;
use base64::Engine;

pub fn screenshot(args: &[Value]) -> Value {
    let url = match args.first().and_then(|v| v.as_str()) {
        Some(u) if u.starts_with("http://") || u.starts_with("https://") => u.to_string(),
        Some(u) => return json!({"ok": false, "error": format!("invalid URL: {}", u)}),
        None => return json!({"ok": false, "error": "url argument is required"}),
    };
    let width = args.get(1).and_then(|v| v.as_u64()).unwrap_or(1280);
    let height = args.get(2).and_then(|v| v.as_u64()).unwrap_or(800);
    let full_page = args.get(3).and_then(|v| v.as_bool()).unwrap_or(false);

    let chrome = match find_chrome_binary() {
        Some(c) => c,
        None => {
            return json!({
                "ok": false,
                "error": "Chrome not found. Install Google Chrome and try again."
            })
        }
    };

    // Write screenshot to a temp file
    let tmp_path = std::env::temp_dir().join(format!(
        "traits-ss-{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros()
    ));

    let window_size = if full_page {
        // Use very tall viewport to capture full page
        format!("{},10000", width)
    } else {
        format!("{},{}", width, height)
    };

    let screenshot_arg = format!("--screenshot={}", tmp_path.display());
    let window_arg = format!("--window-size={}", window_size);

    let out = Command::new(&chrome)
        .args([
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--disable-software-rasterizer",
            "--hide-scrollbars",
            &screenshot_arg,
            &window_arg,
            "--timeout=15000",
            &url,
        ])
        .output();

    match out {
        Ok(result) => {
            if tmp_path.exists() {
                match std::fs::read(&tmp_path) {
                    Ok(bytes) => {
                        let _ = std::fs::remove_file(&tmp_path);
                        let size = bytes.len();
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        json!({
                            "ok": true,
                            "url": url,
                            "image": format!("data:image/png;base64,{}", b64),
                            "width": width,
                            "height": height,
                            "bytes": size
                        })
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&tmp_path);
                        json!({"ok": false, "error": format!("failed to read screenshot: {}", e)})
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                json!({
                    "ok": false,
                    "error": format!(
                        "Chrome did not produce output (exit {:?}): {}",
                        result.status.code(),
                        stderr.trim()
                    )
                })
            }
        }
        Err(e) => json!({"ok": false, "error": format!("chrome launch failed: {}", e)}),
    }
}

fn find_chrome_binary() -> Option<String> {
    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/snap/bin/chromium",
        "/usr/local/bin/chromium",
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return Some(c.to_string());
        }
    }
    for name in &["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"] {
        if Command::new(name)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some((*name).to_string());
        }
    }
    None
}
