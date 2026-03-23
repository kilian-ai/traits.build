use serde_json::{json, Value};
use std::process::Command;

const PROJECT_DIR: &str = env!("CARGO_MANIFEST_DIR");

/// Release pipeline: build → test → commit → push → tag → publish → deploy
///
/// Steps:
///   all      = build,test,commit,push,tag,publish,deploy
///   ci       = commit,push,tag
///   ship     = commit,push,tag,deploy
///   custom   = comma-separated subset, e.g. "build,test,commit"
///
/// Each step reports { name, ok, output } or { name, skipped: true }.
/// Pipeline halts on first failure (unless dry_run).
pub fn release(args: &[Value]) -> Value {
    let steps_str = args.first().and_then(|v| v.as_str()).unwrap_or("all");
    let message = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let dry_run = args.get(2).and_then(|v| v.as_bool()).unwrap_or(false);

    let steps = resolve_steps(steps_str);
    let version = read_version();
    let commit_msg = if message.is_empty() {
        format!("release: {}", version)
    } else {
        message.to_string()
    };

    let all_steps = ["build", "test", "commit", "push", "tag", "publish", "deploy"];
    let mut results: Vec<Value> = Vec::new();
    let mut failed = false;

    for &step_name in &all_steps {
        if !steps.contains(&step_name.to_string()) {
            results.push(json!({ "name": step_name, "skipped": true }));
            continue;
        }

        if failed {
            results.push(json!({ "name": step_name, "skipped": true, "reason": "previous step failed" }));
            continue;
        }

        if dry_run {
            results.push(json!({
                "name": step_name,
                "dry_run": true,
                "would_run": describe_step(step_name, &version, &commit_msg),
            }));
            continue;
        }

        let result = run_step(step_name, &version, &commit_msg);
        let ok = result["ok"].as_bool().unwrap_or(false);
        if !ok {
            failed = true;
        }
        results.push(result);
    }

    let all_ok = !failed;
    json!({
        "ok": all_ok,
        "version": version,
        "dry_run": dry_run,
        "steps": results,
    })
}

/// Resolve step presets or comma-separated list
fn resolve_steps(input: &str) -> Vec<String> {
    match input.trim() {
        "all" => vec!["build", "test", "commit", "push", "tag", "publish", "deploy"]
            .into_iter().map(String::from).collect(),
        "ci" => vec!["commit", "push", "tag"]
            .into_iter().map(String::from).collect(),
        "ship" => vec!["commit", "push", "tag", "deploy"]
            .into_iter().map(String::from).collect(),
        other => other.split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

/// Read current version from version.trait.toml
fn read_version() -> String {
    let toml_path = format!("{}/traits/sys/version/version.trait.toml", PROJECT_DIR);
    if let Ok(content) = std::fs::read_to_string(&toml_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("version") && trimmed.contains('=') {
                if let Some(val) = trimmed.split('=').nth(1) {
                    let v = val.trim().trim_matches('"').trim();
                    if !v.is_empty() {
                        return v.to_string();
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

/// Describe what a step would do (for dry_run)
fn describe_step(step: &str, version: &str, message: &str) -> String {
    match step {
        "build"   => "cargo build --release".to_string(),
        "test"    => "traits test_runner '*'".to_string(),
        "commit"  => format!("git add -A && git commit -m '{}'", message),
        "push"    => "git push origin main".to_string(),
        "tag"     => format!("git tag {} && git push origin {}", version, version),
        "publish" => "cargo publish".to_string(),
        "deploy"  => "scripts/fast-deploy.sh".to_string(),
        _         => format!("unknown step: {}", step),
    }
}

/// Execute a pipeline step, return JSON result
fn run_step(step: &str, version: &str, message: &str) -> Value {
    match step {
        "build"   => step_build(),
        "test"    => step_test(),
        "commit"  => step_commit(message),
        "push"    => step_push(),
        "tag"     => step_tag(version),
        "publish" => step_publish(),
        "deploy"  => step_deploy(),
        _ => json!({ "name": step, "ok": false, "error": format!("Unknown step: {}", step) }),
    }
}

// ── Step implementations ─────────────────────────────────────────────

fn step_build() -> Value {
    let result = run_cmd("bash", &["build.sh"]);
    json!({
        "name": "build",
        "ok": result.0,
        "output": truncate(&result.1, 500),
    })
}

fn step_test() -> Value {
    let binary = format!("{}/target/release/traits", PROJECT_DIR);
    let result = run_cmd(&binary, &["test_runner", "*"]);
    // Parse test output for summary
    let output = &result.1;
    let ok = result.0 && !output.contains("\"failed\":");
    json!({
        "name": "test",
        "ok": ok,
        "output": truncate(output, 1000),
    })
}

fn step_commit(message: &str) -> Value {
    // Check if there are any changes to commit
    let status = run_cmd("git", &["status", "--porcelain"]);
    if status.1.trim().is_empty() {
        return json!({
            "name": "commit",
            "ok": true,
            "output": "nothing to commit, working tree clean",
        });
    }

    // Stage all — including build-generated version bumps in .trait.toml and Cargo.toml
    let add = run_cmd("git", &["add", "-A"]);
    if !add.0 {
        return json!({ "name": "commit", "ok": false, "error": add.1 });
    }

    let commit = run_cmd("git", &["commit", "-m", message]);
    json!({
        "name": "commit",
        "ok": commit.0,
        "output": truncate(&commit.1, 300),
    })
}

fn step_push() -> Value {
    let result = run_cmd("git", &["push", "origin", "main"]);
    json!({
        "name": "push",
        "ok": result.0,
        "output": truncate(&result.1, 300),
    })
}

fn step_tag(version: &str) -> Value {
    // Delete existing tag if present (replace)
    let existing = run_cmd("git", &["rev-parse", version]);
    if existing.0 {
        let _ = run_cmd("git", &["tag", "-d", version]);
        let _ = run_cmd("git", &["push", "origin", &format!(":refs/tags/{}", version)]);
        // Try to delete GitHub release too
        let _ = run_cmd("gh", &["release", "delete", version, "--yes"]);
    }

    // Create tag
    let tag = run_cmd("git", &["tag", version]);
    if !tag.0 {
        return json!({ "name": "tag", "ok": false, "error": tag.1 });
    }

    // Push tag
    let push_tag = run_cmd("git", &["push", "origin", version]);
    if !push_tag.0 {
        return json!({ "name": "tag", "ok": false, "error": push_tag.1 });
    }

    // Create GitHub release with changelog
    let log_result = run_cmd("git", &["log", "--oneline", &format!("{}..HEAD", version)]);
    let changelog = if log_result.0 && !log_result.1.trim().is_empty() {
        log_result.1.clone()
    } else {
        // Fallback: last 10 commits
        let fallback = run_cmd("git", &["log", "--oneline", "-10"]);
        fallback.1
    };

    let notes = format!(
        "## traits.build {}\n\nPure Rust composable function kernel.\n\n### Changes\n\n{}",
        version, changelog
    );

    let gh = run_cmd("gh", &[
        "release", "create", version,
        "--title", version,
        "--notes", &notes,
    ]);

    json!({
        "name": "tag",
        "ok": true,
        "output": format!("Tagged {} (gh release: {})", version, if gh.0 { "created" } else { "skipped" }),
    })
}

fn step_publish() -> Value {
    let result = run_cmd("cargo", &["publish", "--allow-dirty"]);
    json!({
        "name": "publish",
        "ok": result.0,
        "output": truncate(&result.1, 500),
    })
}

fn step_deploy() -> Value {
    // Use dispatch to call www.admin.fast_deploy if available
    let deploy_args = vec![json!("build")];
    match crate::dispatcher::compiled::dispatch("www.admin.fast_deploy", &deploy_args) {
        Some(result) => {
            let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            json!({
                "name": "deploy",
                "ok": ok,
                "output": truncate(
                    &result.get("output").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    500
                ),
            })
        }
        None => {
            // Fallback: run script directly
            let script = format!("{}/scripts/fast-deploy.sh", PROJECT_DIR);
            let result = run_cmd("bash", &[&script]);
            json!({
                "name": "deploy",
                "ok": result.0,
                "output": truncate(&result.1, 500),
            })
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Run a command in the project directory, return (success, combined_output)
fn run_cmd(program: &str, args: &[&str]) -> (bool, String) {
    match Command::new(program)
        .args(args)
        .current_dir(PROJECT_DIR)
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = if stderr.is_empty() {
                stdout.to_string()
            } else if stdout.is_empty() {
                stderr.to_string()
            } else {
                format!("{}\n{}", stdout, stderr)
            };
            (output.status.success(), combined)
        }
        Err(e) => (false, format!("Failed to run {}: {}", program, e)),
    }
}

/// Truncate string to max_len, appending "..." if truncated
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
