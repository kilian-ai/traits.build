use serde_json::Value;

/// Pretty-print test runner results for CLI output.
pub fn format_cli(result: &Value) -> String {
    let mut out = String::new();

    let pattern = result["pattern"].as_str().unwrap_or("?");
    let ok = result["ok"].as_bool().unwrap_or(false);
    let summary = &result["summary"];

    // Header
    out.push_str(&format!("\n  {} Test Runner — pattern: {}\n", if ok { "✓" } else { "✗" }, pattern));
    out.push_str("  ─────────────────────────────────────\n");

    // Per-trait results
    if let Some(results) = result["results"].as_array() {
        for r in results {
            let name = r["trait"].as_str().unwrap_or("?");
            let trait_ok = r["ok"].as_bool().unwrap_or(false);
            let ex_p = r["examples"]["passed"].as_u64().unwrap_or(0);
            let ex_f = r["examples"]["failed"].as_u64().unwrap_or(0);
            let cmd_p = r["commands"]["passed"].as_u64().unwrap_or(0);
            let cmd_f = r["commands"]["failed"].as_u64().unwrap_or(0);
            let total = ex_p + ex_f + cmd_p + cmd_f;
            let passed = ex_p + cmd_p;

            let icon = if trait_ok { "\x1b[32m✓\x1b[0m" } else { "\x1b[31m✗\x1b[0m" };
            out.push_str(&format!("  {} {:<30} {}/{} passed", icon, name, passed, total));

            // Show breakdown if there are both types
            if (ex_p + ex_f) > 0 && (cmd_p + cmd_f) > 0 {
                out.push_str(&format!("  (examples: {}, commands: {})", ex_p, cmd_p));
            }
            out.push('\n');

            // Verbose: show individual test details with checkmarks
            if let Some(details) = r.get("details").and_then(|d| d.as_array()) {
                for d in details {
                    let feature = d["feature"].as_str().unwrap_or("?");
                    let d_ok = d["passed"].as_bool().unwrap_or(false);
                    let dtype = d.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    let icon = if d_ok { "\x1b[32m✓\x1b[0m" } else { "\x1b[31m✗\x1b[0m" };
                    let tag = if dtype.is_empty() { String::new() } else { format!(" \x1b[2m({})\x1b[0m", dtype) };
                    out.push_str(&format!("    {} {}{}\n", icon, feature, tag));

                    if !d_ok {
                        // Show failing checks for example tests
                        if let Some(checks) = d.get("checks").and_then(|c| c.as_array()) {
                            for ch in checks.iter().filter(|c| !c["ok"].as_bool().unwrap_or(true)) {
                                let ctype = ch["type"].as_str().unwrap_or("?");
                                let cval = ch["value"].as_str().unwrap_or("?");
                                out.push_str(&format!("      → {} failed: {}\n", ctype, cval));
                            }
                        }
                        // Show fail reasons for command tests
                        if let Some(expect) = d.get("expect").and_then(|e| e.as_str()) {
                            out.push_str(&format!("      expected: {}\n", expect));
                        }
                        if let Some(cmd) = d.get("command").and_then(|c| c.as_str()) {
                            out.push_str(&format!("      $ {}\n", cmd));
                        }
                    }
                }
            }

            // Show failures inline (non-verbose mode — no details array)
            let failures = r.get("failures").and_then(|f| f.as_array());
            if let Some(fails) = failures {
                for f in fails {
                    let test_name = f.get("test")
                        .or_else(|| f.get("feature"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let ftype = f["type"].as_str().unwrap_or("?");
                    out.push_str(&format!("    \x1b[31m✗\x1b[0m {} ({})\n", test_name, ftype));

                    // Failure reasons for command tests
                    if let Some(reasons) = f.get("failReasons").and_then(|r| r.as_array()) {
                        for reason in reasons {
                            if let Some(s) = reason.as_str() {
                                out.push_str(&format!("      → {}\n", s));
                            }
                        }
                    }
                    // Failing checks for example tests
                    if let Some(checks) = f.get("failing_checks").and_then(|c| c.as_array()) {
                        for check in checks {
                            let ctype = check["type"].as_str().unwrap_or("?");
                            let cval = check["value"].as_str().unwrap_or("?");
                            out.push_str(&format!("      → {} failed: {}\n", ctype, cval));
                        }
                    }
                    // Error message
                    if let Some(err) = f.get("error").and_then(|e| e.as_str()) {
                        out.push_str(&format!("      → {}\n", err));
                    }
                    // Command that was run
                    if let Some(cmd) = f.get("command").and_then(|c| c.as_str()) {
                        out.push_str(&format!("      $ {}\n", cmd));
                    }
                }
            }
        }
    }

    // Summary line
    out.push_str("  ─────────────────────────────────────\n");
    let tp = summary["total_passed"].as_u64().unwrap_or(0);
    let tf = summary["total_failed"].as_u64().unwrap_or(0);
    let traits = summary["traits"].as_u64().unwrap_or(0);
    let skipped = summary["skipped"].as_u64().unwrap_or(0);

    let color = if tf == 0 { "\x1b[32m" } else { "\x1b[31m" };
    out.push_str(&format!("  {}  {} passed, {} failed\x1b[0m", color, tp, tf));
    out.push_str(&format!("  ({} trait{}",  traits, if traits != 1 { "s" } else { "" }));
    if skipped > 0 {
        out.push_str(&format!(", {} skipped", skipped));
    }
    out.push_str(")\n\n");

    out
}
