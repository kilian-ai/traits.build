use serde_json::{json, Value};
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Entry: test_runner(pattern, verbose?)
pub fn test_runner(args: &[Value]) -> Value {
    // Prevent recursive calls (test_runner testing itself)
    if RUNNING.swap(true, Ordering::SeqCst) {
        return json!({ "ok": true, "skipped": true, "reason": "recursive call prevented" });
    }
    let result = test_runner_inner(args);
    RUNNING.store(false, Ordering::SeqCst);
    result
}

fn test_runner_inner(args: &[Value]) -> Value {
    let pattern = args.first().and_then(|v| v.as_str()).unwrap_or("*").trim();
    let verbose = args.get(1).and_then(|v| v.as_bool()).unwrap_or(false);
    let skip_commands = args.get(2).and_then(|v| v.as_bool()).unwrap_or(false);

    // If pattern looks like a filesystem path, discover features.json from disk
    let traits = if pattern.contains('/') || pattern.starts_with('.') {
        discover_fs_features(pattern)
    } else {
        discover_traits(pattern)
    };
    if traits.is_empty() {
        return json!({
            "ok": false,
            "error": format!("No traits with features.json match pattern '{}'", pattern),
            "pattern": pattern,
        });
    }

    let mut all_results = Vec::new();
    let mut total_ex_passed = 0u32;
    let mut total_ex_failed = 0u32;
    let mut total_cmd_passed = 0u32;
    let mut total_cmd_failed = 0u32;
    let mut total_skipped = 0u32;

    for (trait_path, features_path, params) in &traits {
        let features = match load_features(features_path) {
            Some(f) => f,
            None => continue,
        };
        if features.is_empty() {
            total_skipped += 1;
            continue;
        }

        let ex_results = run_example_tests(trait_path, &features, params, verbose);
        let cmd_results = if skip_commands { vec![] } else { run_command_tests(&features, verbose) };

        let ex_p = ex_results.iter().filter(|r| r["passed"].as_bool() == Some(true)).count() as u32;
        let ex_f = ex_results.iter().filter(|r| r["passed"].as_bool() != Some(true)).count() as u32;
        let cmd_p = cmd_results.iter().filter(|r| r["passed"].as_bool() == Some(true)).count() as u32;
        let cmd_f = cmd_results.iter().filter(|r| r["passed"].as_bool() != Some(true)).count() as u32;

        total_ex_passed += ex_p;
        total_ex_failed += ex_f;
        total_cmd_passed += cmd_p;
        total_cmd_failed += cmd_f;

        if ex_results.is_empty() && cmd_results.is_empty() {
            total_skipped += 1;
            continue;
        }

        let mut trait_result = json!({
            "trait": trait_path,
            "ok": ex_f == 0 && cmd_f == 0,
            "examples": { "passed": ex_p, "failed": ex_f },
            "commands": { "passed": cmd_p, "failed": cmd_f },
        });

        let all_details: Vec<Value> = ex_results.into_iter().chain(cmd_results).collect();
        if verbose {
            trait_result["details"] = Value::Array(all_details);
        } else {
            let failures: Vec<Value> = all_details.into_iter().filter(|d| d["passed"].as_bool() != Some(true)).collect();
            if !failures.is_empty() {
                trait_result["failures"] = Value::Array(failures);
            }
        }

        all_results.push(trait_result);
    }

    let total_passed = total_ex_passed + total_cmd_passed;
    let total_failed = total_ex_failed + total_cmd_failed;

    json!({
        "ok": total_failed == 0,
        "pattern": pattern,
        "summary": {
            "traits": all_results.len(),
            "examples": { "passed": total_ex_passed, "failed": total_ex_failed },
            "commands": { "passed": total_cmd_passed, "failed": total_cmd_failed },
            "skipped": total_skipped,
            "total_passed": total_passed,
            "total_failed": total_failed,
        },
        "results": all_results,
    })
}

// ── Discover traits with .features.json ─────────────────────
// Returns Vec<(trait_path, features_json_path, params)>
fn discover_traits(pattern: &str) -> Vec<(String, String, Vec<(String, String)>)> {
    let registry = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return vec![],
    };

    let (ns_filter, name_filter) = if pattern.contains('.') {
        let parts: Vec<&str> = pattern.splitn(2, '.').collect();
        (parts[0], parts[1])
    } else {
        ("", pattern)
    };

    let mut results = Vec::new();
    for entry in registry.all() {
        // Match against pattern
        let parts: Vec<&str> = entry.path.splitn(2, '.').collect();
        if parts.len() != 2 { continue; }
        let (ns, name) = (parts[0], parts[1]);

        if !ns_filter.is_empty() && ns_filter != "*" && ns != ns_filter { continue; }
        if name_filter != "*" && name != name_filter { continue; }

        // Look for features.json next to the toml_path
        let toml_dir = match entry.toml_path.parent() {
            Some(d) => d,
            None => continue,
        };
        let features_path = toml_dir.join(format!("{}.features.json", name));
        if !features_path.exists() { continue; }

        let params: Vec<(String, String)> = entry.signature.params.iter()
            .map(|p| (p.name.clone(), format!("{:?}", p.param_type)))
            .collect();

        results.push((
            entry.path.clone(),
            features_path.to_string_lossy().to_string(),
            params,
        ));
    }
    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

// ── Discover features.json from filesystem glob / path ──────
// Pattern like './src/*', './src/cli.features.json', or './src/'
// Returns entries with empty params (no trait signature → example tests skipped).
fn discover_fs_features(pattern: &str) -> Vec<(String, String, Vec<(String, String)>)> {
    use std::path::{Path, PathBuf};

    let mut files: Vec<PathBuf> = Vec::new();

    // If the pattern literally names a .features.json file, use it directly
    let p = Path::new(pattern);
    if p.is_file() && pattern.ends_with(".features.json") {
        files.push(p.to_path_buf());
    } else if p.is_dir() {
        // Directory: find all *.features.json inside it (non-recursive)
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                let ep = e.path();
                if ep.to_string_lossy().ends_with(".features.json") {
                    files.push(ep);
                }
            }
        }
    } else {
        // Glob: expand by walking parent dir and matching the filename pattern.
        // e.g. './src/*' → parent=./src, pattern=*
        // e.g. './src/*.features.json' → parent=./src, pattern=*.features.json
        let parent = p.parent().unwrap_or(Path::new("."));
        let glob_part = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();

        if parent.is_dir() {
            collect_features_recursive(parent, &glob_part, &mut files);
        }
    }

    files.sort();
    files.iter().filter_map(|fp| {
        let fname = fp.file_stem()?.to_string_lossy().to_string(); // e.g. "cli.features"
        let label = fname.strip_suffix(".features").unwrap_or(&fname);
        // Build a display path: dir/name, e.g. "src/cli"
        let dir_name = fp.parent()
            .and_then(|d| d.file_name())
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_default();
        let display = if dir_name.is_empty() { label.to_string() } else { format!("{}/{}", dir_name, label) };
        Some((display, fp.to_string_lossy().to_string(), vec![]))
    }).collect()
}

/// Recursively collect *.features.json files under `dir`.
/// If `glob_part` is "*", collect all; otherwise match the prefix.
fn collect_features_recursive(dir: &std::path::Path, glob_part: &str, out: &mut Vec<std::path::PathBuf>) {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for e in rd.flatten() {
        let ep = e.path();
        if ep.is_dir() {
            collect_features_recursive(&ep, glob_part, out);
        } else if ep.to_string_lossy().ends_with(".features.json") {
            if glob_part == "*" || glob_part.is_empty() {
                out.push(ep);
            } else if let Some(fname) = ep.file_name() {
                let fname_s = fname.to_string_lossy();
                // Simple prefix glob: "cli*" matches "cli.features.json"
                let prefix = glob_part.trim_end_matches('*');
                if prefix.is_empty() || fname_s.starts_with(prefix) {
                    out.push(ep);
                }
            }
        }
    }
}

// ── Load features from JSON ─────────────────────────────────
fn load_features(path: &str) -> Option<Vec<Value>> {
    let text = fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&text).ok()?;
    parsed.get("features").and_then(|f| f.as_array()).cloned()
}

// ── Run example-based tests (internal dispatch) ─────────────
fn run_example_tests(
    trait_path: &str,
    features: &[Value],
    params: &[(String, String)],
    verbose: bool,
) -> Vec<Value> {
    let mut results = Vec::new();

    for feature in features {
        let feature_name = feature.get("name").and_then(|n| n.as_str()).unwrap_or("unnamed");
        let examples = match feature.get("examples").and_then(|e| e.as_array()) {
            Some(e) => e,
            None => continue,
        };

        for example in examples {
            let input = example.get("input");
            let args = input_to_args(input, params);

            // Call trait via internal dispatch
            let (output, error) = match crate::dispatcher::compiled::dispatch(trait_path, &args) {
                Some(v) => {
                    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                        (v.clone(), Some(e.to_string()))
                    } else {
                        (v, None)
                    }
                }
                None => (Value::Null, Some(format!("Trait '{}' not found in compiled dispatch", trait_path))),
            };

            let output_str = if let Some(ref e) = error {
                format!("{{\"ok\":false,\"error\":\"{}\"}}", e.replace('"', "\\\""))
            } else {
                serde_json::to_string(&output).unwrap_or_default()
            };

            let expected = example.get("output").cloned().unwrap_or(json!({}));
            let checks = run_checks(&expected, &output_str, &output, error.as_deref());
            let passed = if checks.is_empty() { error.is_none() } else { checks.iter().all(|c| c["ok"].as_bool() == Some(true)) };

            let mut entry = json!({
                "type": "example",
                "feature": feature_name,
                "passed": passed,
            });
            if let Some(ref e) = error {
                entry["error"] = json!(e);
            }

            if verbose {
                entry["input"] = input.cloned().unwrap_or(Value::Null);
                entry["output"] = output;
                entry["checks"] = Value::Array(checks);
            } else if !passed {
                entry["input"] = input.cloned().unwrap_or(Value::Null);
                entry["failing_checks"] = Value::Array(checks.into_iter().filter(|c| c["ok"].as_bool() != Some(true)).collect());
            }

            results.push(entry);
        }
    }
    results
}

// ── Map object/array input to positional args ───────────────
fn input_to_args(input: Option<&Value>, params: &[(String, String)]) -> Vec<Value> {
    match input {
        Some(Value::Array(arr)) => arr.clone(),
        Some(Value::Object(obj)) => {
            // Find last param index referenced in input
            let mut last_used: i32 = -1;
            for (i, (name, _)) in params.iter().enumerate() {
                if obj.contains_key(name) {
                    last_used = i as i32;
                }
            }
            let mut args = Vec::new();
            for i in 0..=(last_used as usize) {
                if i < params.len() {
                    let (name, _ptype) = &params[i];
                    args.push(obj.get(name).cloned().unwrap_or(Value::Null));
                }
            }
            args
        }
        Some(v) => vec![v.clone()],
        None => vec![],
    }
}

// ── Run contains/not_contains checks ────────────────────────
fn run_checks(expected: &Value, output_str: &str, output: &Value, error: Option<&str>) -> Vec<Value> {
    let mut checks = Vec::new();

    if let Some(contains) = expected.get("contains").and_then(|c| c.as_array()) {
        for needle in contains {
            let ok = match needle {
                Value::String(s) => output_str.contains(s.as_str()),
                _ => {
                    let target = if let Some(e) = error {
                        json!({"ok": false, "error": e})
                    } else {
                        output.clone()
                    };
                    object_needle_match(needle, &target)
                }
            };
            checks.push(json!({
                "type": "contains",
                "value": match needle { Value::String(s) => s.clone(), _ => serde_json::to_string(needle).unwrap_or_default() },
                "ok": ok,
            }));
        }
    }

    if let Some(not_contains) = expected.get("not_contains").and_then(|c| c.as_array()) {
        for needle in not_contains {
            let ok = match needle {
                Value::String(s) => !output_str.contains(s.as_str()),
                _ => {
                    let target = if let Some(e) = error {
                        json!({"ok": false, "error": e})
                    } else {
                        output.clone()
                    };
                    !object_needle_match(needle, &target)
                }
            };
            checks.push(json!({
                "type": "not_contains",
                "value": match needle { Value::String(s) => s.clone(), _ => serde_json::to_string(needle).unwrap_or_default() },
                "ok": ok,
            }));
        }
    }

    checks
}

// ── Deep equality ───────────────────────────────────────────
fn deep_equals(a: &Value, b: &Value) -> bool {
    a == b
}

// ── Partial object match (needle keys ⊂ haystack) ──────────
fn object_contains(needle: &Value, haystack: &Value) -> bool {
    match (needle, haystack) {
        (Value::Object(n), Value::Object(h)) => {
            n.iter().all(|(k, v)| h.get(k).map_or(false, |hv| deep_equals(v, hv)))
        }
        _ => deep_equals(needle, haystack),
    }
}

fn object_needle_match(needle: &Value, output: &Value) -> bool {
    if let Value::Array(arr) = output {
        arr.iter().any(|item| object_contains(needle, item))
    } else {
        object_contains(needle, output)
    }
}

// ── Run shell command tests ─────────────────────────────────
fn run_command_tests(features: &[Value], verbose: bool) -> Vec<Value> {
    let mut results = Vec::new();

    // Determine cwd — use TRAITS_DIR parent or CARGO_MANIFEST_DIR
    let cwd = crate::globals::TRAITS_DIR.get()
        .map(|p| p.parent().unwrap_or(p).to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    for feature in features {
        let feature_name = feature.get("name").and_then(|n| n.as_str()).unwrap_or("unnamed");
        let tests = match feature.get("tests").and_then(|t| t.as_array()) {
            Some(t) => t,
            None => continue,
        };

        for test in tests {
            let command = match test.get("command").and_then(|c| c.as_str()) {
                Some(c) => c,
                None => continue,
            };
            let test_name = test.get("name").and_then(|n| n.as_str()).unwrap_or(command);

            let result = Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&cwd)
                .output();

            let (stdout, stderr, exit_code) = match result {
                Ok(out) => (
                    String::from_utf8_lossy(&out.stdout).trim().to_string(),
                    String::from_utf8_lossy(&out.stderr).trim().to_string(),
                    out.status.code().unwrap_or(1),
                ),
                Err(e) => (String::new(), e.to_string(), 1),
            };

            let expect = test.get("expect").and_then(|e| e.as_str()).unwrap_or("");
            let checks_arr = test.get("checks").and_then(|c| c.as_array());
            let mut passed = true;
            let mut fail_reasons = Vec::new();
            let mut check_results: Vec<Value> = Vec::new();

            if let Some(checks) = checks_arr {
                // Structured checks format
                for check in checks {
                    let ctype = check.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    let expected = &check["expected"];
                    let ok = match ctype {
                        "exit_code" => {
                            let exp = expected.as_i64().unwrap_or(0) as i32;
                            exit_code == exp
                        }
                        "contains" => {
                            let s = expected.as_str().unwrap_or("");
                            stdout.contains(s) || stderr.contains(s)
                        }
                        "not_contains" => {
                            let s = expected.as_str().unwrap_or("");
                            !stdout.contains(s) && !stderr.contains(s)
                        }
                        "count_gte" => {
                            let threshold = expected.as_i64().unwrap_or(0);
                            let num: i64 = stdout.trim().parse().unwrap_or(0);
                            num >= threshold
                        }
                        _ => true,
                    };
                    if !ok {
                        passed = false;
                        fail_reasons.push(format!("{}: expected {:?}", ctype, expected));
                    }
                    check_results.push(json!({
                        "type": ctype,
                        "expected": expected,
                        "ok": ok,
                    }));
                }
            } else if !expect.is_empty() {
                // Legacy expect string format
                let exits_zero = regex_match(r"exits?\s+0\b", expect) && !regex_match(r"non.?zero", expect);
                if exits_zero && exit_code != 0 {
                    passed = false;
                    fail_reasons.push(format!("expected exit 0, got {}", exit_code));
                }

                if regex_match(r"exits?\s+non.?zero", expect) && exit_code == 0 {
                    passed = false;
                    fail_reasons.push("expected non-zero exit, got 0".to_string());
                }

                for cap in regex_find_all(r#"contains?\s+['"]([^'"]+)['"]"#, expect) {
                    if !stdout.contains(&cap) && !stderr.contains(&cap) {
                        passed = false;
                        fail_reasons.push(format!("output missing: \"{}\"", cap));
                    }
                }

                if let Some((op, threshold)) = parse_count_check(expect) {
                    let num: i64 = stdout.trim().parse().unwrap_or(0);
                    let ok = match op.as_str() {
                        ">=" => num >= threshold,
                        ">" => num > threshold,
                        "<=" => num <= threshold,
                        "<" => num < threshold,
                        "==" | "=" => num == threshold,
                        _ => true,
                    };
                    if !ok {
                        passed = false;
                        fail_reasons.push(format!("count {} not {} {}", num, op, threshold));
                    }
                }
            }

            let mut entry = json!({
                "type": "command",
                "feature": feature_name,
                "test": test_name,
                "passed": passed,
            });

            if verbose {
                entry["command"] = json!(command);
                entry["stdout"] = json!(&stdout[..std::cmp::min(stdout.len(), 500)]);
                entry["stderr"] = json!(&stderr[..std::cmp::min(stderr.len(), 500)]);
                entry["exitCode"] = json!(exit_code);
                if !check_results.is_empty() {
                    entry["checks"] = Value::Array(check_results);
                } else {
                    entry["expect"] = json!(expect);
                }
            } else if !passed {
                entry["command"] = json!(command);
                entry["failReasons"] = json!(fail_reasons);
            }

            results.push(entry);
        }
    }
    results
}

// ── Simple regex helpers (avoid pulling in regex crate) ─────
fn regex_match(pattern: &str, text: &str) -> bool {
    // Use simple string matching for the patterns we need
    match pattern {
        r"exits?\s+0\b" => {
            text.contains("exit 0") || text.contains("exits 0")
        }
        r"non.?zero" => {
            text.contains("non-zero") || text.contains("nonzero") || text.contains("non zero")
        }
        r"exits?\s+non.?zero" => {
            text.contains("exit non-zero") || text.contains("exits non-zero")
                || text.contains("exit nonzero") || text.contains("exits nonzero")
        }
        _ => text.contains(pattern),
    }
}

fn regex_find_all(pattern: &str, text: &str) -> Vec<String> {
    // For contains "X" patterns, parse manually
    if pattern.contains("contains") {
        let mut results = Vec::new();
        let lower = text.to_lowercase();
        let search_in = |prefix: &str| -> Vec<String> {
            let mut found = Vec::new();
            let mut pos = 0;
            while let Some(idx) = lower[pos..].find(prefix) {
                let start = pos + idx + prefix.len();
                // Skip whitespace
                let rest = &text[start..];
                let trimmed = rest.trim_start();
                if let Some(quote) = trimmed.chars().next() {
                    if quote == '"' || quote == '\'' {
                        if let Some(end) = trimmed[1..].find(quote) {
                            found.push(trimmed[1..1 + end].to_string());
                        }
                    }
                }
                pos = start;
            }
            found
        };
        results.extend(search_in("contains "));
        results.extend(search_in("contain "));
        results
    } else {
        vec![]
    }
}

fn parse_count_check(text: &str) -> Option<(String, i64)> {
    let lower = text.to_lowercase();
    if let Some(idx) = lower.find("count") {
        let rest = &text[idx + 5..].trim_start();
        // Parse operator
        let ops = [">=", "<=", "==", ">", "<", "="];
        for op in &ops {
            if rest.starts_with(op) {
                let num_str = rest[op.len()..].trim_start();
                if let Ok(n) = num_str.split_whitespace().next().unwrap_or("").parse::<i64>() {
                    return Some((op.to_string(), n));
                }
            }
        }
    }
    None
}
