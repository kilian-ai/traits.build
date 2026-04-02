use serde_json::{json, Value};

/// llm.agent.skills — Return raw content from all SKILL.md files.
///
/// WASM-compatible: all skills are embedded at compile time via include_str!.
///
/// Args: [name?]
///   name: Optional skill name (e.g. "traits.build", "secrets", "src2doc").
///         If omitted, returns all skills.
///         Use "list" to get just the index.
pub fn agent_skills(args: &[Value]) -> Value {
    let name = args.first().and_then(|v| v.as_str()).unwrap_or("");

    match name {
        "list" => {
            let index: Vec<Value> = SKILLS.iter().map(|s| json!({
                "name": s.name,
                "description": s.description,
            })).collect();
            json!({ "ok": true, "skills": index })
        }
        "" | "all" => {
            let all: Vec<Value> = SKILLS.iter().map(|s| json!({
                "name": s.name,
                "description": s.description,
                "content": s.markdown,
            })).collect();
            json!({ "ok": true, "skills": all })
        }
        _ => {
            for s in SKILLS {
                if s.name == name {
                    return json!({
                        "ok": true,
                        "name": s.name,
                        "description": s.description,
                        "content": s.markdown,
                    });
                }
            }
            json!({ "ok": false, "error": format!("Skill '{}' not found. Use 'list' to see available skills.", name) })
        }
    }
}

struct Skill {
    name: &'static str,
    description: &'static str,
    markdown: &'static str,
}

const SKILLS: &[Skill] = &[
    Skill {
        name: "traits.build",
        description: "Invoke traits.build API functions as MCP tools or REST calls",
        markdown: include_str!("../../../../.github/skills/traits.build/SKILL.md"),
    },
    Skill {
        name: "secrets",
        description: "How to use the sys.secrets trait for managing secrets",
        markdown: include_str!("../../../../.github/skills/secrets/SKILL.md"),
    },
    Skill {
        name: "src2doc",
        description: "Generate 1:1 mapped documentation for source files",
        markdown: include_str!("../../../../.github/skills/src2doc/SKILL.md"),
    },
];
