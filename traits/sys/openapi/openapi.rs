use serde_json::{json, Value};

/// sys.openapi — Generate an OpenAPI 3.0 specification from the trait registry.
///
/// Reads all registered traits, maps their signatures to OpenAPI paths,
/// and returns a complete spec object suitable for Redoc/Swagger UI.
/// Examples are generated live by calling each safe trait via compiled dispatch.
pub fn openapi(_args: &[Value]) -> Value {
    let reg = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return json!({"error": "Registry not initialized"}),
    };

    let all = reg.all();
    let mut paths = serde_json::Map::new();
    let mut tag_set = std::collections::BTreeSet::new();

    // Generate live examples by calling safe traits
    let live_examples = generate_live_examples(&all);

    for entry in &all {
        // Skip kernel internals from the public API docs
        if entry.path.starts_with("kernel.") {
            continue;
        }

        let namespace = entry.path.split('.').next().unwrap_or("other");
        tag_set.insert(namespace.to_string());

        // Build the API path: sys.checksum -> /traits/sys/checksum
        let api_path = format!("/traits/{}", entry.path.replace('.', "/"));

        // Build request body schema from params
        let mut param_properties = serde_json::Map::new();
        let mut required_params = Vec::new();

        for p in entry.signature.params.iter() {
            let schema = trait_type_to_schema(&p.param_type);
            let mut prop = schema.clone();
            if !p.description.is_empty() {
                prop.as_object_mut().unwrap()
                    .insert("description".into(), Value::String(p.description.clone()));
            }
            param_properties.insert(p.name.clone(), prop);

            if !p.optional {
                required_params.push(Value::String(p.name.clone()));
            }
        }

        // Get live example for this trait (request args + response)
        let live = live_examples.get(entry.path.as_str());

        // Request body supports both array and object form
        let request_body = if entry.signature.params.is_empty() {
            json!({
                "required": false,
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "properties": {
                                "args": {
                                    "type": "array",
                                    "items": {},
                                    "description": "No arguments required"
                                }
                            }
                        },
                        "example": { "args": [] }
                    }
                }
            })
        } else {
            let mut object_schema = json!({
                "type": "object",
                "properties": param_properties,
            });
            if !required_params.is_empty() {
                object_schema.as_object_mut().unwrap()
                    .insert("required".into(), Value::Array(required_params));
            }

            let args_desc = format!("Positional arguments: [{}]",
                entry.signature.params.iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "));

            let schema_desc = format!(
                "Pass arguments either as positional array (`args: [...]`) or as named object (`args: {{{}}}`).",
                entry.signature.params.iter()
                    .map(|p| format!("\"{}\": ...", p.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            // Use live example args or fall back to type-based defaults
            let positional_example: Vec<Value> = if let Some(ex) = live {
                ex.request_args.clone()
            } else {
                entry.signature.params.iter()
                    .map(|p| example_value(&p.param_type))
                    .collect()
            };

            let named_example: serde_json::Map<String, Value> = entry.signature.params.iter()
                .zip(positional_example.iter())
                .map(|(p, v)| (p.name.clone(), v.clone()))
                .collect();

            json!({
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "properties": {
                                "args": {
                                    "type": "array",
                                    "items": {},
                                    "description": args_desc
                                }
                            },
                            "description": schema_desc
                        },
                        "examples": {
                            "positional": {
                                "summary": "Positional array form",
                                "value": { "args": positional_example }
                            },
                            "named": {
                                "summary": "Named object form",
                                "value": { "args": named_example }
                            }
                        }
                    }
                }
            })
        };

        let return_schema = trait_type_to_schema(&entry.signature.returns.return_type);

        // Build 200 response with real example if available
        let response_200 = if let Some(ex) = live {
            json!({
                "description": entry.signature.returns.description,
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "properties": {
                                "result": return_schema,
                                "error": { "type": "string", "nullable": true }
                            }
                        },
                        "example": { "result": ex.response }
                    }
                }
            })
        } else {
            json!({
                "description": entry.signature.returns.description,
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "properties": {
                                "result": return_schema,
                                "error": { "type": "string", "nullable": true }
                            }
                        }
                    }
                }
            })
        };

        let mut operation = json!({
            "summary": format!("POST {}", api_path),
            "description": entry.description,
            "operationId": entry.path.replace('.', "_"),
            "tags": [namespace],
            "requestBody": request_body,
            "responses": {
                "200": response_200,
                "404": {
                    "description": "Trait not found",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                },
                "400": {
                    "description": "Bad request (argument count or type mismatch)",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                }
            }
        });

        // Mark background/streaming traits
        if entry.background || entry.stream {
            let mut desc = entry.description.clone();
            if entry.background {
                desc.push_str(" *(background trait)*");
            }
            if entry.stream {
                desc.push_str(" *(supports `?stream=1` for SSE)*");
            }
            operation.as_object_mut().unwrap()
                .insert("description".into(), Value::String(desc));
        }

        paths.insert(api_path, json!({ "post": operation }));
    }

    // Build tags array with descriptions
    let tags: Vec<Value> = tag_set.iter().map(|ns| {
        let desc = match ns.as_str() {
            "sys" => "System utilities — registry, checksums, versioning, testing",
            "www" => "Web interface — landing page, admin dashboard, deployment",
            _ => "",
        };
        json!({ "name": ns, "description": desc })
    }).collect();

    // Add standard endpoints (non-trait)
    let mut standard_paths = serde_json::Map::new();
    standard_paths.insert("/health".into(), json!({
        "get": {
            "summary": "Health check",
            "operationId": "health_check",
            "tags": ["infrastructure"],
            "responses": {
                "200": {
                    "description": "Server health status",
                    "content": {
                        "application/json": {
                            "schema": {
                                "type": "object",
                                "properties": {
                                    "status": { "type": "string", "example": "healthy" },
                                    "version": { "type": "string", "example": "v260322" },
                                    "trait_count": { "type": "integer" },
                                    "namespace_count": { "type": "integer" },
                                    "uptime_human": { "type": "string" },
                                    "uptime_seconds": { "type": "integer" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }));
    standard_paths.insert("/metrics".into(), json!({
        "get": {
            "summary": "Prometheus-compatible metrics",
            "operationId": "metrics",
            "tags": ["infrastructure"],
            "responses": {
                "200": {
                    "description": "Prometheus text format metrics",
                    "content": {
                        "text/plain": {
                            "schema": { "type": "string" }
                        }
                    }
                }
            }
        }
    }));
    standard_paths.insert("/traits".into(), json!({
        "get": {
            "summary": "List all traits (tree view)",
            "operationId": "list_traits",
            "tags": ["infrastructure"],
            "responses": {
                "200": {
                    "description": "Hierarchical tree of all registered traits",
                    "content": {
                        "application/json": {
                            "schema": { "type": "object" }
                        }
                    }
                }
            }
        }
    }));
    standard_paths.insert("/traits/{path}".into(), json!({
        "get": {
            "summary": "Get trait info by path",
            "operationId": "get_trait_info",
            "tags": ["infrastructure"],
            "parameters": [{
                "name": "path",
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": "Dot-notation trait path (e.g. sys/checksum)"
            }],
            "responses": {
                "200": {
                    "description": "Full trait definition including signature, params, returns",
                    "content": {
                        "application/json": {
                            "schema": { "type": "object" }
                        }
                    }
                },
                "404": {
                    "description": "Trait not found",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                }
            }
        }
    }));

    // Merge standard + trait paths
    for (k, v) in paths {
        standard_paths.insert(k, v);
    }

    // Build the full OpenAPI spec
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "traits.build API",
            "description": "REST API for the traits.build composable function kernel.\n\nEvery trait is callable via `POST /traits/{namespace}/{name}` with a JSON body `{\"args\": [...]}`. Arguments can be passed as a positional array or as a named object.\n\n**Live instance:** [https://traits.build](https://traits.build)",
            "version": env!("TRAITS_BUILD_VERSION"),
            "contact": {
                "name": "GitHub",
                "url": "https://github.com/kilian-ai/traits.build"
            },
            "license": {
                "name": "MIT"
            }
        },
        "servers": [
            {
                "url": "https://traits.build",
                "description": "Production (Fly.io)"
            },
            {
                "url": "http://127.0.0.1:8090",
                "description": "Local development"
            }
        ],
        "tags": tags.into_iter().chain(std::iter::once(json!({
            "name": "infrastructure",
            "description": "Server health, metrics, and trait introspection endpoints"
        }))).collect::<Vec<_>>(),
        "paths": standard_paths,
        "components": {
            "schemas": {
                "ErrorResponse": {
                    "type": "object",
                    "properties": {
                        "result": { "nullable": true },
                        "error": { "type": "string" }
                    }
                }
            }
        }
    })
}

/// Map a TraitType to an OpenAPI/JSON Schema type object.
fn trait_type_to_schema(t: &crate::types::TraitType) -> Value {
    use crate::types::TraitType;
    match t {
        TraitType::Int => json!({ "type": "integer", "format": "int64" }),
        TraitType::Float => json!({ "type": "number", "format": "double" }),
        TraitType::String => json!({ "type": "string" }),
        TraitType::Bool => json!({ "type": "boolean" }),
        TraitType::Bytes => json!({ "type": "string", "format": "binary" }),
        TraitType::Null => json!({ "nullable": true }),
        TraitType::List(inner) => json!({
            "type": "array",
            "items": trait_type_to_schema(inner)
        }),
        TraitType::Map(_, v) => json!({
            "type": "object",
            "additionalProperties": trait_type_to_schema(v)
        }),
        TraitType::Optional(inner) => {
            let mut s = trait_type_to_schema(inner);
            if let Some(obj) = s.as_object_mut() {
                obj.insert("nullable".into(), Value::Bool(true));
            }
            s
        }
        TraitType::Any => json!({}),
        TraitType::Handle => json!({ "type": "string", "description": "Opaque handle reference" }),
    }
}

/// Generate an example value for a TraitType (fallback when live dispatch isn't available).
fn example_value(t: &crate::types::TraitType) -> Value {
    use crate::types::TraitType;
    match t {
        TraitType::Int => json!(42),
        TraitType::Float => json!(3.14),
        TraitType::String => json!("example"),
        TraitType::Bool => json!(true),
        TraitType::Bytes => json!("deadbeef"),
        TraitType::Null => Value::Null,
        TraitType::List(inner) => json!([example_value(inner)]),
        TraitType::Map(_, v) => json!({ "key": example_value(v) }),
        TraitType::Optional(inner) => example_value(inner),
        TraitType::Any => json!("value"),
        TraitType::Handle => json!("hdl:example"),
    }
}

/// A live example: the args we called with + the response we got back.
struct LiveExample {
    request_args: Vec<Value>,
    response: Value,
}

/// Known-safe example args for each trait that can be called live.
fn safe_example_args(path: &str) -> Option<Vec<Value>> {
    match path {
        "sys.checksum"      => Some(vec![json!("hash"), json!("hello world")]),
        "sys.version"       => Some(vec![json!("system")]),
        "sys.list"          => Some(vec![json!("sys")]),
        "sys.info"          => Some(vec![json!("sys.checksum")]),
        "sys.registry"      => Some(vec![json!("namespaces")]),
        "sys.ps"            => Some(vec![]),
        "sys.cli"           => Some(vec![]),
        _ => None,
    }
}

/// Static examples for traits that are unsafe to call live or return non-JSON.
fn static_example(path: &str) -> Option<LiveExample> {
    match path {
        "sys.snapshot" => Some(LiveExample {
            request_args: vec![json!("sys.version")],
            response: json!({"ok": true, "trait": "sys.version", "old_version": "v260321", "new_version": "v260322"}),
        }),
        "sys.test_runner" => Some(LiveExample {
            request_args: vec![json!("sys.checksum")],
            response: json!({"ok": true, "pattern": "sys.checksum", "total": 1, "passed": 1, "failed": 0, "details": [{"trait": "sys.checksum", "passed": 3, "failed": 0}]}),
        }),
        "sys.openapi" => Some(LiveExample {
            request_args: vec![],
            response: json!({"openapi": "3.0.3", "info": {"title": "traits.build API", "version": "v260322"}, "paths": {"...": "..."}}),
        }),
        "www.traits.build" => Some(LiveExample {
            request_args: vec![],
            response: json!("<!DOCTYPE html><html>... (landing page HTML)"),
        }),
        "www.admin" => Some(LiveExample {
            request_args: vec![],
            response: json!("<!DOCTYPE html><html>... (admin dashboard HTML)"),
        }),
        "www.docs.api" => Some(LiveExample {
            request_args: vec![],
            response: json!("<!DOCTYPE html><html>... (API documentation HTML)"),
        }),
        "www.admin.deploy" => Some(LiveExample {
            request_args: vec![json!("status")],
            response: json!({"ok": true, "mode": "status", "machines": [{"id": "abc123", "state": "started", "region": "iad"}]}),
        }),
        "www.admin.fast_deploy" => Some(LiveExample {
            request_args: vec![json!("status")],
            response: json!({"ok": true, "status": "deployed"}),
        }),
        "www.admin.scale" => Some(LiveExample {
            request_args: vec![json!(1)],
            response: json!({"ok": true, "target": 1, "machines": [{"id": "abc123", "state": "started"}]}),
        }),
        "www.admin.destroy" => Some(LiveExample {
            request_args: vec![],
            response: json!({"ok": true, "destroyed": 1}),
        }),
        _ => None,
    }
}

/// Generate live examples by actually calling safe traits via compiled dispatch.
/// Falls back to static examples for unsafe traits.
fn generate_live_examples(all: &[crate::registry::TraitEntry]) -> std::collections::HashMap<&str, LiveExample> {
    let mut map = std::collections::HashMap::new();
    for entry in all {
        if entry.path.starts_with("kernel.") { continue; }

        // Try live dispatch first (safe traits only)
        if let Some(args) = safe_example_args(&entry.path) {
            let json_args: Vec<Value> = args.iter().cloned().collect();
            if let Some(result) = crate::dispatcher::compiled::dispatch(&entry.path, &json_args) {
                let truncated = truncate_example(result);
                map.insert(entry.path.as_str(), LiveExample {
                    request_args: args,
                    response: truncated,
                });
                continue;
            }
        }

        // Fall back to static examples for unsafe/HTML traits
        if let Some(ex) = static_example(&entry.path) {
            map.insert(entry.path.as_str(), ex);
        }
    }
    map
}

/// Truncate large example responses to keep the OpenAPI spec readable.
/// - Arrays: keep first 3 items, append "..." marker
/// - Maps: keep first 5 keys
/// - Strings: truncate at 200 chars
fn truncate_example(v: Value) -> Value {
    match v {
        Value::Array(arr) if arr.len() > 3 => {
            let mut truncated: Vec<Value> = arr.into_iter().take(3).map(truncate_example).collect();
            truncated.push(json!("... (truncated)"));
            Value::Array(truncated)
        }
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (k, val) in map.into_iter().take(8) {
                result.insert(k, truncate_example(val));
            }
            Value::Object(result)
        }
        Value::String(s) if s.len() > 200 => {
            Value::String(format!("{}... (truncated)", &s[..200]))
        }
        other => other,
    }
}
