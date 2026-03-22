use serde_json::{json, Value};

/// sys.openapi — Generate an OpenAPI 3.0 specification from the trait registry.
///
/// Reads all registered traits, maps their signatures to OpenAPI paths,
/// and returns a complete spec object suitable for Redoc/Swagger UI.
pub fn openapi(_args: &[Value]) -> Value {
    let reg = match crate::globals::REGISTRY.get() {
        Some(r) => r,
        None => return json!({"error": "Registry not initialized"}),
    };

    let all = reg.all();
    let mut paths = serde_json::Map::new();
    let mut tag_set = std::collections::BTreeSet::new();

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
        let mut param_items = Vec::new();
        let mut required_params = Vec::new();

        for (i, p) in entry.signature.params.iter().enumerate() {
            let schema = trait_type_to_schema(&p.param_type);
            let mut prop = schema.clone();
            if !p.description.is_empty() {
                prop.as_object_mut().unwrap()
                    .insert("description".into(), Value::String(p.description.clone()));
            }
            param_properties.insert(p.name.clone(), prop);

            // For the args array form
            param_items.push(json!({
                "description": format!("args[{}]: {} — {}", i, p.name, p.description),
            }));

            if !p.optional {
                required_params.push(Value::String(p.name.clone()));
            }
        }

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
                        }
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

            let positional_example: Vec<Value> = entry.signature.params.iter()
                .map(|p| example_value(&p.param_type))
                .collect();

            let named_example: serde_json::Map<String, Value> = entry.signature.params.iter()
                .map(|p| (p.name.clone(), example_value(&p.param_type)))
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

        let mut operation = json!({
            "summary": entry.description,
            "operationId": entry.path.replace('.', "_"),
            "tags": [namespace],
            "requestBody": request_body,
            "responses": {
                "200": {
                    "description": entry.signature.returns.description,
                    "content": {
                        "application/json": {
                            "schema": {
                                "type": "object",
                                "properties": {
                                    "result": return_schema,
                                    "error": {
                                        "type": "string",
                                        "nullable": true
                                    }
                                }
                            }
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
                .insert("summary".into(), Value::String(desc));
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
            "description": "REST API for the traits.build composable function kernel.\n\nEvery trait is callable via `POST /traits/{namespace}/{name}` with a JSON body `{\"args\": [...]}`. Arguments can be passed as a positional array or as a named object.\n\n**Live instance:** [https://polygrait-api.fly.dev](https://polygrait-api.fly.dev)",
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
                "url": "https://polygrait-api.fly.dev",
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

/// Generate an example value for a TraitType (used in request examples).
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
