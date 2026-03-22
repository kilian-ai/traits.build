use serde_json::Value;
use std::process::Command;

const PROJECT_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub fn fast_deploy(args: &[Value]) -> Value {
    let mode = args.first()
        .and_then(|v| v.as_str())
        .unwrap_or("build");

    let script = format!("{}/scripts/fast-deploy.sh", PROJECT_DIR);

    // Verify script exists
    if !std::path::Path::new(&script).exists() {
        return serde_json::json!({
            "ok": false,
            "error": format!("Script not found: {}", script)
        });
    }

    let mut cmd = Command::new("bash");
    cmd.arg(&script);
    if mode == "upload" {
        cmd.arg("--upload");
    }
    cmd.current_dir(PROJECT_DIR);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let combined = if stderr.is_empty() {
                stdout.clone()
            } else {
                format!("{}\n{}", stdout, stderr)
            };
            serde_json::json!({
                "ok": output.status.success(),
                "exit_code": output.status.code().unwrap_or(-1),
                "output": combined.trim_end()
            })
        }
        Err(e) => serde_json::json!({
            "ok": false,
            "error": format!("Failed to run script: {}", e)
        }),
    }
}
