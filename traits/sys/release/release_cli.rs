use serde_json::Value;

pub fn format_cli(value: &Value) -> String {
    let mut out = String::new();

    let version = value.get("version").and_then(|v| v.as_str()).unwrap_or("?");
    let dry_run = value.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
    let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);

    if dry_run {
        out.push_str(&format!("=== Release Pipeline (dry run) — {} ===\n\n", version));
    } else {
        out.push_str(&format!("=== Release Pipeline — {} ===\n\n", version));
    }

    if let Some(steps) = value.get("steps").and_then(|v| v.as_array()) {
        for step in steps {
            let name = step.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let skipped = step.get("skipped").and_then(|v| v.as_bool()).unwrap_or(false);
            let step_dry = step.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
            let step_ok = step.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);

            if skipped {
                let reason = step.get("reason").and_then(|v| v.as_str()).unwrap_or("not selected");
                out.push_str(&format!("  ○ {:10} — skipped ({})\n", name, reason));
            } else if step_dry {
                let would_run = step.get("would_run").and_then(|v| v.as_str()).unwrap_or("?");
                out.push_str(&format!("  ◇ {:10} → {}\n", name, would_run));
            } else if step_ok {
                let output = step.get("output").and_then(|v| v.as_str()).unwrap_or("");
                let summary = output.lines().last().unwrap_or(output);
                out.push_str(&format!("  ✓ {:10}   {}\n", name, summary));
            } else {
                let error = step.get("error")
                    .or_else(|| step.get("output"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                out.push_str(&format!("  ✗ {:10}   {}\n", name, error));
            }
        }
    }

    out.push('\n');
    if ok {
        out.push_str(&format!("✓ Pipeline complete: {}\n", version));
    } else if dry_run {
        out.push_str("◇ Dry run complete — no changes made\n");
    } else {
        out.push_str("✗ Pipeline failed — see errors above\n");
    }

    out
}
