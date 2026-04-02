use serde_json::{json, Value};

/// llm.agent.docs — Return raw markdown content from all project documentation.
///
/// WASM-compatible: all docs are embedded at compile time via include_str!.
///
/// Args: [slug?]
///   slug: Optional doc slug (e.g. "architecture", "cli"). If omitted, returns all docs.
///         Use "list" to get just the index of available docs.
///         Use "agent" to get the agent instructions file.
pub fn agent_docs(args: &[Value]) -> Value {
    let slug = args.first().and_then(|v| v.as_str()).unwrap_or("");

    match slug {
        "list" => {
            let index: Vec<Value> = DOCS.iter().map(|d| json!({
                "slug": d.slug,
                "title": d.title,
                "category": d.category,
            })).collect();
            json!({ "ok": true, "docs": index })
        }
        "agent" => {
            json!({
                "ok": true,
                "slug": "agent",
                "title": "Agent Instructions",
                "content": include_str!("../../../../.github/agents/traits.build.agent.md"),
            })
        }
        "" | "all" => {
            let all: Vec<Value> = DOCS.iter().map(|d| json!({
                "slug": d.slug,
                "title": d.title,
                "category": d.category,
                "content": d.markdown,
            })).collect();
            json!({ "ok": true, "docs": all })
        }
        _ => {
            // Find specific doc by slug
            for d in DOCS {
                if d.slug == slug {
                    return json!({
                        "ok": true,
                        "slug": d.slug,
                        "title": d.title,
                        "category": d.category,
                        "content": d.markdown,
                    });
                }
            }
            json!({ "ok": false, "error": format!("Doc '{}' not found. Use 'list' to see available docs.", slug) })
        }
    }
}

struct Doc {
    slug: &'static str,
    title: &'static str,
    markdown: &'static str,
    category: &'static str,
}

const DOCS: &[Doc] = &[
    Doc { slug: "intro",              title: "Overview",            markdown: include_str!("../../../../docs/intro.md"),                category: "" },
    Doc { slug: "getting-started",    title: "Getting Started",     markdown: include_str!("../../../../docs/getting-started.md"),      category: "" },
    Doc { slug: "architecture",       title: "Architecture",        markdown: include_str!("../../../../docs/architecture.md"),         category: "Core Concepts" },
    Doc { slug: "trait-definition",   title: "Trait Definition",    markdown: include_str!("../../../../docs/trait-definition.md"),     category: "Core Concepts" },
    Doc { slug: "interfaces",         title: "Interfaces",          markdown: include_str!("../../../../docs/interfaces.md"),           category: "Core Concepts" },
    Doc { slug: "type-system",        title: "Type System",         markdown: include_str!("../../../../docs/type-system.md"),          category: "Core Concepts" },
    Doc { slug: "platform-abstraction", title: "Platform Abstraction", markdown: include_str!("../../../../docs/platform-abstraction.md"), category: "Core Concepts" },
    Doc { slug: "rest-api",           title: "REST API",            markdown: include_str!("../../../../docs/rest-api.md"),             category: "Reference" },
    Doc { slug: "cli",                title: "CLI Reference",       markdown: include_str!("../../../../docs/cli.md"),                  category: "Reference" },
    Doc { slug: "creating-traits",    title: "Creating Traits",     markdown: include_str!("../../../../docs/creating-traits.md"),      category: "Guides" },
    Doc { slug: "deployment",         title: "Deployment",          markdown: include_str!("../../../../docs/deployment.md"),           category: "Guides" },
    Doc { slug: "deploy",             title: "Deploy Reference",    markdown: include_str!("../../../../docs/deploy.md"),               category: "Guides" },
    Doc { slug: "release",            title: "Release Process",     markdown: include_str!("../../../../docs/release.md"),              category: "Guides" },
];
