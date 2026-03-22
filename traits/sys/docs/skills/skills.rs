use serde_json::{json, Value};
use std::collections::BTreeMap;

/// sys.docs.skills — Generate a SKILL.md file from the OpenAPI spec.
///
/// Actions:
///   generate [openapi_json] — return markdown string (default: uses live sys.openapi)
///   write    [openapi_json] — generate + write to .github/skills/traits.build/SKILL.md
pub fn skills(args: &[Value]) -> Value {
    let action = args.first()
        .and_then(|v| v.as_str())
        .unwrap_or("generate");

    let spec = match args.get(1) {
        Some(v) if v.is_object() => v.clone(),
        _ => crate::dispatcher::compiled::openapi::openapi(&[]),
    };

    let markdown = match generate_skill_md(&spec) {
        Ok(md) => md,
        Err(e) => return json!({"error": e}),
    };

    match action {
        "write" => {
            let traits_dir = crate::globals::TRAITS_DIR.get()
                .map(|p| p.clone())
                .unwrap_or_else(|| std::path::PathBuf::from("traits"));
            let workspace_root = traits_dir.parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            let skill_path = workspace_root
                .join(".github")
                .join("skills")
                .join("traits.build")
                .join("SKILL.md");

            if let Some(parent) = skill_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return json!({"error": format!("Failed to create directory: {}", e)});
                }
            }
            match std::fs::write(&skill_path, &markdown) {
                Ok(_) => json!({
                    "ok": true,
                    "action": "write",
                    "path": skill_path.display().to_string(),
                    "bytes": markdown.len()
                }),
                Err(e) => json!({"error": format!("Failed to write: {}", e)}),
            }
        }
        _ => Value::String(markdown),
    }
}

fn generate_skill_md(spec: &Value) -> Result<String, String> {
    let paths = spec.get("paths")
        .and_then(|p| p.as_object())
        .ok_or("OpenAPI spec missing 'paths'")?;

    let version = spec.pointer("/info/version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let server_url = spec.pointer("/servers/0/url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://traits.build");

    // Group traits by namespace/tag
    let mut by_tag: BTreeMap<String, Vec<TraitSkill>> = BTreeMap::new();

    for (path, methods) in paths {
        let post = match methods.get("post") {
            Some(p) => p,
            None => continue,
        };

        // Skip infrastructure endpoints
        let tags = post.get("tags")
            .and_then(|t| t.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
        if tags.contains(&"infrastructure") {
            continue;
        }

        let op_id = post.get("operationId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let trait_path = op_id.replace('_', ".");
        let mcp_tool = op_id.to_string();
        let description = post.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let summary = post.get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Extract params from examples
        let params = extract_params(post);
        let example_req = extract_example_request(post);
        let example_resp = extract_example_response(post);

        let tag = tags.first().map(|s| s.to_string()).unwrap_or_else(|| "other".to_string());

        let skill = TraitSkill {
            trait_path,
            mcp_tool,
            api_path: path.clone(),
            description,
            summary,
            params,
            example_req,
            example_resp,
        };

        by_tag.entry(tag).or_default().push(skill);
    }

    // Build markdown
    let mut md = String::new();

    md.push_str("---\n");
    md.push_str("name: traits.build\n");
    md.push_str("description: |\n");
    md.push_str("  Invoke traits.build API functions as MCP tools or REST calls.\n");
    md.push_str("  Every trait is available as an MCP tool (dot→underscore naming)\n");
    md.push_str("  and as a REST endpoint (POST /traits/{namespace}/{name}).\n");
    md.push_str("---\n\n");

    md.push_str("# traits.build Skills\n\n");
    md.push_str(&format!(
        "> Auto-generated from OpenAPI spec {} — [traits.build]({})\n\n",
        version, server_url
    ));

    md.push_str("## How to call traits\n\n");
    md.push_str("Every trait can be invoked two ways:\n\n");
    md.push_str("### MCP Tool Call\n\n");
    md.push_str("Tool names use underscore notation: `sys.checksum` → `mcp_traits-build_sys_checksum`\n\n");
    md.push_str("```json\n");
    md.push_str("{\n");
    md.push_str("  \"name\": \"mcp_traits-build_sys_checksum\",\n");
    md.push_str("  \"arguments\": { \"action\": \"hash\", \"data\": \"hello\" }\n");
    md.push_str("}\n");
    md.push_str("```\n\n");
    md.push_str("### REST API Call\n\n");
    md.push_str(&format!("```bash\ncurl -X POST {}/traits/sys/checksum \\\n", server_url));
    md.push_str("  -H 'Content-Type: application/json' \\\n");
    md.push_str("  -d '{\"args\": [\"hash\", \"hello\"]}'\n```\n\n");

    md.push_str("---\n\n");
    md.push_str("## Available Traits\n\n");

    for (tag, skills) in &by_tag {
        let tag_desc = match tag.as_str() {
            "sys" => "System utilities — registry, checksums, versioning, testing",
            "www" => "Web interface — landing page, admin dashboard, deployment",
            _ => "",
        };
        md.push_str(&format!("### {} — {}\n\n", tag, tag_desc));
        md.push_str("| MCP Tool | Trait Path | Description |\n");
        md.push_str("|----------|------------|-------------|\n");
        for skill in skills {
            md.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                skill.mcp_tool,
                skill.trait_path,
                skill.description.replace('\n', " ").replace('|', "\\|")
            ));
        }
        md.push_str("\n");

        // Detail each trait
        for skill in skills {
            md.push_str(&format!("#### `{}`\n\n", skill.trait_path));
            if !skill.description.is_empty() {
                md.push_str(&format!("{}\n\n", skill.description));
            }
            md.push_str(&format!("- **MCP tool:** `{}`\n", skill.mcp_tool));
            md.push_str(&format!("- **REST:** `POST {}`\n", skill.api_path));

            if !skill.params.is_empty() {
                md.push_str("\n**Parameters:**\n\n");
                md.push_str("| Name | Type | Required | Description |\n");
                md.push_str("|------|------|----------|-------------|\n");
                for p in &skill.params {
                    md.push_str(&format!(
                        "| `{}` | `{}` | {} | {} |\n",
                        p.name, p.param_type,
                        if p.required { "yes" } else { "no" },
                        p.description.replace('|', "\\|")
                    ));
                }
            }

            if let Some(ref req) = skill.example_req {
                md.push_str("\n**Example request:**\n\n");
                md.push_str(&format!("```json\n{}\n```\n", req));
            }
            if let Some(ref resp) = skill.example_resp {
                md.push_str("\n**Example response:**\n\n");
                md.push_str(&format!("```json\n{}\n```\n", resp));
            }
            md.push_str("\n");
        }
    }

    md.push_str("---\n\n");
    md.push_str(&format!(
        "*Generated by `sys.docs.skills` from traits.build {} — [API Docs]({}/docs/api)*\n",
        version, server_url
    ));

    Ok(md)
}

struct TraitSkill {
    trait_path: String,
    mcp_tool: String,
    api_path: String,
    description: String,
    #[allow(dead_code)]
    summary: String,
    params: Vec<ParamInfo>,
    example_req: Option<String>,
    example_resp: Option<String>,
}

struct ParamInfo {
    name: String,
    param_type: String,
    required: bool,
    description: String,
}

fn extract_params(post: &Value) -> Vec<ParamInfo> {
    // Try requestBody → content → application/json → schema → properties
    let schema = post.pointer("/requestBody/content/application/json/schema");
    let props = schema
        .and_then(|s| s.get("properties"))
        .and_then(|p| p.get("args"))
        .and_then(|a| a.get("description"))
        .and_then(|d| d.as_str());

    // Also try the object form schema
    let examples = post.pointer("/requestBody/content/application/json/examples");
    let named_example = examples
        .and_then(|e| e.get("named"))
        .and_then(|n| n.get("value"))
        .and_then(|v| v.get("args"))
        .and_then(|a| a.as_object());

    // Try to get schema properties from the description
    let desc = schema.and_then(|s| s.get("description")).and_then(|d| d.as_str());
    let _obj_schema = if let Some(d) = desc {
        // Parse param names from description like: Pass arguments either as positional array or as named object
        Some(d.to_string())
    } else {
        None
    };

    // Build params from the named example if available
    let mut params = Vec::new();

    // Try to get proper schema with required array
    let full_schema = post.pointer("/requestBody/content/application/json/schema/properties");
    if let Some(full) = full_schema {
        // This is the object-form schema with param properties
        if let Some(obj) = full.as_object() {
            let required_arr = post.pointer("/requestBody/content/application/json/schema/required")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();

            for (name, prop) in obj {
                if name == "args" { continue; }
                let param_type = prop.get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("any")
                    .to_string();
                let description = prop.get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let required = required_arr.contains(&name.as_str());
                params.push(ParamInfo { name: name.clone(), param_type, required, description });
            }
        }
    }

    // If no proper schema, try building from named example + args description
    if params.is_empty() {
        if let Some(named) = named_example {
            let args_desc = props.unwrap_or("");
            let param_names: Vec<&str> = if args_desc.starts_with("Positional arguments: [") {
                args_desc.trim_start_matches("Positional arguments: [")
                    .trim_end_matches(']')
                    .split(", ")
                    .collect()
            } else {
                Vec::new()
            };

            for (name, _val) in named {
                let desc_for_param = param_names.iter()
                    .find(|&&n| n == name.as_str())
                    .map(|_| "")
                    .unwrap_or("");
                params.push(ParamInfo {
                    name: name.clone(),
                    param_type: "any".to_string(),
                    required: true,
                    description: desc_for_param.to_string(),
                });
            }
        }
    }

    params
}

fn extract_example_request(post: &Value) -> Option<String> {
    let examples = post.pointer("/requestBody/content/application/json/examples");
    if let Some(positional) = examples.and_then(|e| e.get("positional")).and_then(|p| p.get("value")) {
        return Some(serde_json::to_string_pretty(positional).ok()?);
    }
    let example = post.pointer("/requestBody/content/application/json/example");
    example.map(|e| serde_json::to_string_pretty(e).unwrap_or_default())
}

fn extract_example_response(post: &Value) -> Option<String> {
    let example = post.pointer("/responses/200/content/application/json/example");
    example.map(|e| serde_json::to_string_pretty(e).unwrap_or_default())
}
