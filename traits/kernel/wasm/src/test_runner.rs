use serde_json::{json, Value};

/// WASM test runner — runs embedded .features.json tests via WASM dispatch.
///
/// Supports example-based tests only (shell command tests are skipped).
/// Features are loaded from compile-time embedded BUILTIN_FEATURES.
pub fn test_runner(args: &[Value]) -> Value {
    let pattern = args.first().and_then(|v| v.as_str()).unwrap_or("*").trim();
    let verbose = args.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

    // Discover all matching features from embedded data
    let traits = discover_traits(pattern);
    if traits.is_empty() {
        return json!({
            "ok": false,
            "error": format!("No traits with features match pattern '{}'", pattern),
            "pattern": pattern,
        });
    }

    let mut all_results = Vec::new();
    let mut total_ex_passed = 0u32;
    let mut total_ex_failed = 0u32;
    let mut total_cmd_skipped = 0u32;
    let mut total_skipped = 0u32;

    for (trait_path, features_json, params) in &traits {
        let features = match parse_features(features_json) {
            Some(f) => f,
            None => continue,
        };
        if features.is_empty() {
            total_skipped += 1;
            continue;
        }

        // Count command tests that will be skipped
        let cmd_count = count_command_tests(&features);
        total_cmd_skipped += cmd_count;

        let ex_results = run_example_tests(trait_path, &features, params, verbose);

        let ex_p = ex_results.iter().filter(|r| r["passed"].as_bool() == Some(true)).count() as u32;
        let ex_f = ex_results.iter().filter(|r| r["passed"].as_bool() != Some(true)).count() as u32;

        total_ex_passed += ex_p;
        total_ex_failed += ex_f;

        if ex_results.is_empty() && cmd_count == 0 {
            total_skipped += 1;
            continue;
        }

        let mut trait_result = json!({
            "trait": trait_path,
            "ok": ex_f == 0,
            "examples": { "passed": ex_p, "failed": ex_f },
            "commands": { "skipped": cmd_count, "note": "shell commands unavailable in WASM" },
        });

        if verbose {
            trait_result["details"] = Value::Array(ex_results);
        } else {
            let failures: Vec<Value> = ex_results.into_iter()
                .filter(|d| d["passed"].as_bool() != Some(true))
                .collect();
            if !failures.is_empty() {
                trait_result["failures"] = Value::Array(failures);
            }
        }

        all_results.push(trait_result);
    }

    json!({
        "ok": total_ex_failed == 0,
        "pattern": pattern,
        "runtime": "wasm",
        "summary": {
            "traits": all_results.len(),
            "examples": { "passed": total_ex_passed, "failed": total_ex_failed },
            "commands": { "skipped": total_cmd_skipped, "note": "shell commands unavailable in WASM" },
            "skipped": total_skipped,
            "total_passed": total_ex_passed,
            "total_failed": total_ex_failed,
        },
        "results": all_results,
    })
}

/// Discover traits matching a glob pattern from embedded BUILTIN_FEATURES.
/// Returns Vec<(trait_path, features_json_str, param_names)>.
fn discover_traits(pattern: &str) -> Vec<(String, &'static str, Vec<String>)> {
    let (ns_filter, name_filter) = if pattern.contains('.') {
        let parts: Vec<&str> = pattern.splitn(2, '.').collect();
        (parts[0], parts[1])
    } else {
        ("", pattern)
    };

    let reg = crate::get_registry();
    let mut results = Vec::new();

    for &(trait_path, features_json) in crate::BUILTIN_FEATURES {
        // Match against pattern
        let parts: Vec<&str> = trait_path.splitn(2, '.').collect();
        if parts.len() != 2 { continue; }
        let (ns, name) = (parts[0], parts[1]);

        if !ns_filter.is_empty() && ns_filter != "*" && ns != ns_filter { continue; }
        if name_filter != "*" && name != name_filter { continue; }

        // Get param names from registry to support object→positional mapping
        let param_names: Vec<String> = reg.get(trait_path)
            .map(|e| {
                e.params.iter()
                    .filter_map(|p| p.get("name").and_then(|v| v.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        results.push((trait_path.to_string(), features_json, param_names));
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

/// Parse features array from a JSON string.
fn parse_features(json_str: &str) -> Option<Vec<Value>> {
    let parsed: Value = serde_json::from_str(json_str).ok()?;
    parsed.get("features").and_then(|f| f.as_array()).cloned()
}

/// Count command tests in features (these will be skipped in WASM).
fn count_command_tests(features: &[Value]) -> u32 {
    let mut count = 0;
    for feature in features {
        if let Some(tests) = feature.get("tests").and_then(|t| t.as_array()) {
            count += tests.len() as u32;
        }
    }
    count
}

/// Run example-based tests via WASM dispatch.
fn run_example_tests(
    trait_path: &str,
    features: &[Value],
    param_names: &[String],
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
            let args = input_to_args(input, param_names);

            // Call trait via WASM dispatch
            let (output, error) = match crate::wasm_traits::dispatch(trait_path, &args) {
                Some(v) => {
                    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                        (v.clone(), Some(e.to_string()))
                    } else {
                        (v, None)
                    }
                }
                None => (Value::Null, Some(format!("Trait '{}' not WASM-callable", trait_path))),
            };

            let output_str = if let Some(ref e) = error {
                format!("{{\"ok\":false,\"error\":\"{}\"}}", e.replace('"', "\\\""))
            } else {
                serde_json::to_string(&output).unwrap_or_default()
            };

            let expected = example.get("output").cloned().unwrap_or(json!({}));
            let checks = run_checks(&expected, &output_str, &output, error.as_deref());
            let passed = if checks.is_empty() {
                error.is_none()
            } else {
                checks.iter().all(|c| c["ok"].as_bool() == Some(true))
            };

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
                entry["failing_checks"] = Value::Array(
                    checks.into_iter()
                        .filter(|c| c["ok"].as_bool() != Some(true))
                        .collect()
                );
            }

            results.push(entry);
        }
    }
    results
}

/// Convert object/array input to positional args using param names.
fn input_to_args(input: Option<&Value>, param_names: &[String]) -> Vec<Value> {
    match input {
        Some(Value::Array(arr)) => arr.clone(),
        Some(Value::Object(obj)) => {
            let mut last_used: i32 = -1;
            for (i, name) in param_names.iter().enumerate() {
                if obj.contains_key(name) {
                    last_used = i as i32;
                }
            }
            let mut args = Vec::new();
            for i in 0..=(last_used as usize) {
                if i < param_names.len() {
                    args.push(obj.get(&param_names[i]).cloned().unwrap_or(Value::Null));
                }
            }
            args
        }
        Some(v) => vec![v.clone()],
        None => vec![],
    }
}

/// Run contains/not_contains checks against output.
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
                "value": match needle {
                    Value::String(s) => s.clone(),
                    _ => serde_json::to_string(needle).unwrap_or_default(),
                },
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
                "value": match needle {
                    Value::String(s) => s.clone(),
                    _ => serde_json::to_string(needle).unwrap_or_default(),
                },
                "ok": ok,
            }));
        }
    }

    checks
}

fn object_contains(needle: &Value, haystack: &Value) -> bool {
    match (needle, haystack) {
        (Value::Object(n), Value::Object(h)) => {
            n.iter().all(|(k, v)| h.get(k).map_or(false, |hv| v == hv))
        }
        _ => needle == haystack,
    }
}

fn object_needle_match(needle: &Value, output: &Value) -> bool {
    if let Value::Array(arr) = output {
        arr.iter().any(|item| object_contains(needle, item))
    } else {
        object_contains(needle, output)
    }
}
