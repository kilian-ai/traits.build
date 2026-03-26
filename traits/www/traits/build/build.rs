use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashSet;
use maud::{html, DOCTYPE, PreEscaped};

pub fn website(_args: &[Value]) -> Value {
    #[cfg(not(target_arch = "wasm32"))]
    let (trait_count, ns_count) = match crate::globals::REGISTRY.get() {
        Some(reg) => {
            let all = reg.all();
            let namespaces: HashSet<&str> = all.iter()
                .filter_map(|e| e.path.split('.').next())
                .collect();
            (all.len(), namespaces.len())
        }
        None => (0, 0),
    };
    #[cfg(target_arch = "wasm32")]
    let (trait_count, ns_count) = {
        let reg = crate::get_registry();
        let all = reg.all();
        let namespaces: std::collections::HashSet<&str> = all.iter()
            .filter_map(|e| e.path.split('.').next())
            .collect();
        (all.len(), namespaces.len())
    };

    #[cfg(not(target_arch = "wasm32"))]
    let binary_size = std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0))
        .unwrap_or_else(|| "? MB".to_string());
    #[cfg(target_arch = "wasm32")]
    let binary_size = "WASM".to_string();

    // Count WASM-callable traits
    #[cfg(not(target_arch = "wasm32"))]
    let wasm_callable = trait_count;
    #[cfg(target_arch = "wasm32")]
    let wasm_callable = trait_count;

    let code_how_it_works = format!(r##"<span class="cm"># 1. Define a trait (TOML + Rust source)</span>
traits/sys/checksum/checksum.trait.toml
traits/sys/checksum/checksum.rs

<span class="cm"># 2. build.rs discovers it automatically</span>
<span class="cm">#    - Embeds the TOML via include_str!</span>
<span class="cm">#    - Generates mod declarations</span>
<span class="cm">#    - Creates dispatch_compiled() match arms</span>
<span class="cm">#    - Validates checksums, bumps versions</span>

<span class="cm"># 3. cargo build produces a single binary</span>
<span class="kw">$</span> cargo build --release
<span class="cm">#    target/release/traits ({binary_size})</span>

<span class="cm"># 4. Run it anywhere</span>
<span class="kw">$</span> ./traits serve --port 8090
<span class="cm">#    {trait_count} traits loaded, 0 workers, 0 dependencies</span>"##);

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "traits.build \u{2014} composable function kernel" }
                style { (PreEscaped(CSS)) }
            }
            body {
                // Hero
                div.hero {
                    div.pill { "open source \u{00B7} pure Rust \u{00B7} runs in browser via WASM \u{00B7} AI-ready" }
                    h1 { span { "traits" } ".build" }
                    p.sub { "Typed, composable functions compiled into a single Rust binary and a WASM browser kernel. Define traits in TOML, call them via CLI, REST, MCP, or directly in the browser. No server required." }
                    div.cta {
                        a.btn.btn-primary href="#browser-native" { "Try in Browser" }
                        a.btn.btn-outline href="/playground" { "Playground" }
                        a.btn.btn-outline href="/docs" { "Docs" }
                        a.btn.btn-outline href="/docs/api" { "API" }
                        a.btn.btn-outline href="https://github.com/kilian-ai/traits.build" { "GitHub" }
                    }
                }

                // Stats
                div.stats {
                    div.stat { div.num { (trait_count) } div.label { "compiled traits" } }
                    div.stat { div.num { (wasm_callable) } div.label { "WASM-callable" } }
                    div.stat { div.num { (ns_count) } div.label { "namespaces" } }
                    div.stat { div.num { (&binary_size) } div.label { "binary size" } }
                }

                // Browser-native
                h2.section-title id="browser-native" { "Browser-Native" }
                p.section-sub { "The same kernel runs in the browser via WASM \u{2014} no server required" }
                section {
                    div.features {
                        div.card { div.icon { "\u{1F30D}" } h3 { "WASM kernel" } p { "The entire trait registry compiles to WebAssembly via wasm-pack. " (wasm_callable) " traits callable directly in the browser at native speed." } }
                        div.card { div.icon { "\u{26A1}" } h3 { "3-tier dispatch" } p { "SDK tries WASM local \u{2192} localhost helper \u{2192} server REST. Falls through automatically. Most traits resolve in <1ms via WASM." } }
                        div.card { div.icon { "\u{1F4BB}" } h3 { "Terminal in browser" } p { "Full xterm.js terminal running the traits CLI via WASM. Type " code { "list" } ", " code { "call sys.checksum hash hello" } " \u{2014} runs locally." } }
                        div.card { div.icon { "\u{1F3AE}" } h3 { "Interactive playground" } p { "Search, select, and call any trait with a visual form. Parameters auto-generated from TOML signatures. Results display inline." } }
                        div.card { div.icon { "\u{1F50C}" } h3 { "Auto-connect helper" } p { "Run " code { "./traits serve" } " locally and the page auto-discovers it within 1 second. Unlocks privileged traits (deploy, secrets, file I/O)." } }
                        div.card { div.icon { "\u{1F9E9}" } h3 { "Trait Components (TC)" } p { "Declarative " code { "data-trait" } " attributes on HTML elements. TC auto-calls traits on mount or click, binds results to DOM \u{2014} reactive UI without a framework." } }
                    }
                }

                // Architecture
                h2.section-title { "Architecture" }
                p.section-sub { "The kernel is traits all the way down" }
                section {
                    div.arch {
                        div.arch-box { h4 { "CLI & HTTP" } p { "sys.cli parses args, kernel.serve starts actix-web. Every sys.* trait is a direct subcommand." } }
                        div.arch-box { h4 { "Dispatcher" } p { "Resolves paths, validates & coerces arguments, dispatches to compiled Rust trait functions." } }
                        div.arch-box { h4 { "Registry" } p { "Concurrent DashMap of .trait.toml definitions. Loads from disk + compiled builtins. Hot-reloadable." } }
                        div.arch-box { h4 { "Interface System" } p { "provides / requires / bindings. Per-call overrides, global bindings, or auto-discovery." } }
                        div.arch-box { h4 { "Plugin Loader" } p { "kernel.dylib_loader discovers cdylib .dylib plugins at startup for dynamic trait extensions." } }
                        div.arch-box { h4 { "Type System" } p { "int, float, string, bool, bytes, any, list<T>, map<K,V>, T? \u{2014} validated at dispatch time." } }
                    }
                }

                // Features
                h2.section-title { "Why Traits?" }
                p.section-sub { "Functions as the universal building block" }
                section {
                    div.features {
                        div.card { div.icon { "\u{1F680}" } h3 { "Single binary, zero runtime" } p { "Every trait compiles directly into the binary via build.rs. No containers, no workers, no runtime dependencies. One executable does everything." } }
                        div.card { div.icon { "\u{2699}\u{FE0F}" } h3 { "Self-describing kernel" } p { "Registry, dispatcher, config, CLI, and HTTP server are all traits. Call " code { "kernel.main" } " to see every module, interface, and bootstrap step." } }
                        div.card { div.icon { "\u{1F50C}" } h3 { "CLI + REST + MCP + WASM" } p { "Every trait is callable via CLI, REST API, MCP tool protocol, or WASM in the browser. One trait, four surfaces." } }
                        div.card { div.icon { "\u{1F310}" } h3 { "Static SPA on GitHub Pages" } p { "The site is a self-contained HTML file with embedded WASM kernel. Hosted on GitHub Pages \u{2014} no server process needed for most traits." } }
                        div.card { div.icon { "\u{1F517}" } h3 { "Interface wiring" } p { "Traits declare " code { "provides" } ", " code { "requires" } ", and " code { "bindings" } " in TOML. Resolution: per-call overrides \u{2192} global bindings \u{2192} auto-discover." } }
                        div.card { div.icon { "\u{1F9E9}" } h3 { "Build-time codegen" } p { "build.rs discovers all .trait.toml files, generates dispatch tables, embeds definitions, auto-bumps versions and checksums." } }
                        div.card { div.icon { "\u{1F4E6}" } h3 { "cdylib plugins" } p { "Extend at runtime with .dylib shared libraries. The kernel discovers and loads them at startup via kernel.dylib_loader." } }
                    }
                }

                // AI-Ready
                h2.section-title { "AI-Ready by default" }
                p.section-sub { "Every trait is a tool an AI agent can discover, call, and test" }
                section {
                    div.features {
                        div.card { div.icon { "\u{1F916}" } h3 { "MCP server export" } p { "Every compiled trait can be exported as an MCP tool. Agents discover traits via the registry, read signatures from TOML, and call them over stdio." } }
                        div.card { div.icon { "\u{1F9E0}" } h3 { "Agent file included" } p { "The repo ships with a " code { ".github/agents/" } " file that teaches AI agents the project structure, build system, conventions, and how to add new traits." } }
                        div.card { div.icon { "\u{1F4CB}" } h3 { "Self-testing traits" } p { "Every trait has a " code { ".features.json" } " with features, examples, and test commands. Agents can run " code { "traits test_runner '*'" } " to validate changes." } }
                        div.card { div.icon { "\u{1F4D6}" } h3 { "Auto-generated docs" } p { "OpenAPI spec is generated live from the trait registry with real response examples. No hand-written API docs \u{2014} the kernel documents itself." } }
                        div.card { div.icon { "\u{2699}\u{FE0F}" } h3 { "TOML is the spec" } p { "Each " code { ".trait.toml" } " declares the full contract: signature, types, interfaces, dependencies, and wiring. Machine-readable by design." } }
                        div.card { div.icon { "\u{1F6E0}\u{FE0F}" } h3 { "Workspace-ready" } p { "VS Code workspace ships with build/test/serve tasks, MCP server config, and editor settings. Open and go." } }
                        div.card { div.icon { "\u{1F4DD}" } h3 { "SKILL.md generation" } p { "Run " code { "sys.docs.skills" } " to auto-generate a " code { "SKILL.md" } " from the live OpenAPI spec. Teach any AI agent every available trait, its parameters, and how to call it \u{2014} as MCP tools or REST." } }
                    }
                }

                // Secrets
                h2.section-title { "Secure Secrets Handling" }
                p.section-sub { "Protect sensitive data with a clean, developer-friendly design" }
                section {
                    div.features style="grid-template-columns:repeat(auto-fit,minmax(220px,1fr))" {
                        div.card { div.icon { "\u{1F512}" } h3 { "Separation" } p { "Data vs secrets are fully isolated. Secrets live in an encrypted store, never in config files or source code." } }
                        div.card { div.icon { "\u{1F3AF}" } h3 { "Explicit Access" } p { "No hidden flows. Traits declare which secrets they need via " code { "SecretContext::resolve()" } ". Access is intentional and auditable." } }
                        div.card { div.icon { "\u{23F3}" } h3 { "Short Lifetime" } p { "Secrets exist in memory only during execution. " code { "zeroize-on-drop" } " clears values immediately, masked " code { "Debug" } " prevents logging." } }
                        div.card { div.icon { "\u{1F510}" } h3 { "Encrypted at Rest" } p { "AES-GCM encryption with OS-backed key storage. Double encryption: individual values + entire store file." } }
                    }
                    div.code-block style="max-width:100%;padding:0;margin-top:1.5rem" {
                        pre { (PreEscaped(CODE_SECRETS)) }
                    }
                }

                // Built-in traits
                h2.section-title id="built-in-traits" { "Built-in traits" }
                p.section-sub { (trait_count) " traits across kernel, sys, and www \u{2014} all compiled in" }
                section {
                    table.trait-table {
                        tr { th { "Trait" } th { "What it does" } }
                        @for &(path, desc) in TRAITS {
                            tr { td { (path) } td { (desc) } }
                        }
                    }
                }

                // Interface system
                h2.section-title { "Interface system" }
                p.section-sub { "Declare dependencies as typed contracts, not hard-coded paths" }
                section {
                    div.code-block style="max-width:100%;padding:0" {
                        pre { (PreEscaped(CODE_INTERFACES)) }
                    }
                    p style="text-align:center;color:var(--muted);margin:1.5rem 0 0.5rem" { "Resolution chain" }
                    div.iface-flow {
                        span.iface-step { "per-call overrides" }
                        span.iface-arrow { "\u{2192}" }
                        span.iface-step { "global bindings" }
                        span.iface-arrow { "\u{2192}" }
                        span.iface-step { "caller [bindings]" }
                        span.iface-arrow { "\u{2192}" }
                        span.iface-step { "auto-discover" }
                    }
                }

                // Trait definition format
                h2.section-title { "Trait definition format" }
                p.section-sub { ".trait.toml \u{2014} one file defines everything" }
                section {
                    div.code-block style="max-width:100%;padding:0" {
                        pre { (PreEscaped(CODE_TRAIT_DEF)) }
                    }
                    div.type-grid {
                        @for ty in TYPES {
                            div.type-chip { (ty) }
                        }
                    }
                }

                // Quick start
                h2.section-title { "Quick start" }
                p.section-sub { "Four ways to talk to every trait" }
                section {
                    div.code-block style="max-width:100%;padding:0;margin-bottom:2rem" {
                        pre { (PreEscaped(CODE_CLI)) }
                    }
                    div.code-block style="max-width:100%;padding:0;margin-bottom:2rem" {
                        pre { (PreEscaped(CODE_REST)) }
                    }
                    div.code-block style="max-width:100%;padding:0" {
                        pre { (PreEscaped(CODE_BROWSER)) }
                    }
                }

                // How it works
                h2.section-title { "How it works" }
                p.section-sub { "build.rs discovers traits, generates dispatch, compiles everything in" }
                section {
                    div.code-block style="max-width:100%;padding:0" {
                        pre { (PreEscaped(&code_how_it_works)) }
                    }
                }

                footer {
                    p {
                        "traits.build \u{2014} pure Rust kernel + WASM browser runtime. \u{00B7} "
                        a href="/playground" { "Playground" } " \u{00B7} "
                        a href="/docs" { "Docs" } " \u{00B7} "
                        a href="/docs/api" { "API" } " \u{00B7} "
                        a href="https://github.com/kilian-ai/traits.build" { "GitHub" } " \u{00B7} "
                        a href="/admin" { "Admin" } " \u{00B7} "
                        a href="/wasm" { "WASM" }
                    }
                }
            }
        }
    };
    Value::String(markup.into_string())
}

const TRAITS: &[(&str, &str)] = &[
    ("kernel.main", "Entry point, bootstrap, compiled module list, interface introspection"),
    ("kernel.dispatcher", "Path resolution, argument validation & coercion, compiled dispatch"),
    ("kernel.registry", "Load .trait.toml definitions, DashMap lookup, interface resolution"),
    ("kernel.config", "traits.toml parsing, env var overrides, runtime config"),
    ("kernel.serve", "Start the actix-web HTTP server with CORS & SSE streaming"),
    ("kernel.dylib_loader", "Discover and load cdylib .dylib plugins at startup"),
    ("kernel.types", "TraitValue, TraitType, type parsing (list<map<string,int>>)"),
    ("kernel.globals", "OnceLock statics: REGISTRY, CONFIG, TRAITS_DIR, HANDLES"),
    ("kernel.call", "Call any trait by dot-notation path (inter-trait dispatch)"),
    ("kernel.plugin_api", "C ABI export macro for cdylib trait plugins"),
    ("kernel.reload", "Hot-reload the trait registry from disk"),
    ("sys.cli", "Clap parsing, subcommand dispatch, arg coercion, pipe support"),
    ("sys.registry", "Read API: list, info, tree, namespaces, count, search"),
    ("sys.checksum", "SHA-256 checksums for strings, I/O pairs, and signatures"),
    ("sys.version", "Generate YYMMDD date-format versions"),
    ("sys.snapshot", "Snapshot a trait version (YYMMDD or YYMMDD.HHMMSS)"),
    ("sys.test_runner", "Run .features.json tests \u{2014} dispatch examples + shell commands"),
    ("sys.list", "List all registered traits"),
    ("sys.info", "Show detailed trait metadata and signatures"),
    ("sys.ps", "List running background traits with process details"),
    ("sys.openapi", "Generate OpenAPI 3.0 spec with live examples from the registry"),
    ("sys.mcp", "MCP stdio server \u{2014} JSON-RPC 2.0 over stdin/stdout"),
    ("sys.secrets", "Encrypted secrets store \u{2014} set, get, delete, list"),
    ("sys.llm", "Unified LLM inference router \u{2014} provider-agnostic"),
    ("sys.docs.skills", "Generate SKILL.md from OpenAPI \u{2014} teach AI agents every trait"),
    ("www.traits.build", "This landing page \u{2014} stats pulled live from registry"),
    ("www.static", "SPA shell with WASM kernel + SDK + TC system"),
    ("www.playground", "Interactive trait playground \u{2014} search, call, inspect"),
    ("www.terminal", "Browser terminal via xterm.js + WASM CLI"),
    ("www.wasm", "WASM kernel internals and diagnostics page"),
    ("www.docs", "Single-page documentation with all guides rendered from markdown"),
    ("www.docs.api", "Serve Redoc API documentation page (OpenAPI + Redoc)"),
    ("www.admin", "Admin dashboard with deployment controls (Basic Auth)"),
    ("www.admin.spa", "Browser-only admin \u{2014} no auth required, SPA mode"),
    ("www.admin.deploy", "Deploy to Fly.io"),
    ("www.admin.fast_deploy", "Fast deploy: Docker build + sftp upload + restart"),
    ("www.admin.scale", "Scale Fly.io machines up or down"),
    ("www.admin.destroy", "Destroy Fly.io machines"),
    ("www.admin.save_config", "Save deploy configuration to traits.toml"),
    ("www.chat_logs", "Chat history viewer \u{2014} browse indexed conversations"),
    ("www.llm.openai", "OpenAI chat interface with streaming responses"),
];

const TYPES: &[&str] = &[
    "int", "float", "string", "bool", "bytes",
    "any", "list<T>", "map<K,V>", "T? (optional)", "handle",
];

const CSS: &str = r##"
*{margin:0;padding:0;box-sizing:border-box}
:root{--bg:#0a0a0f;--fg:#e8e6e3;--accent:#6c63ff;--accent2:#00d4aa;--muted:#6b7280;--card:#12121a;--border:#1e1e2e}
body{font-family:system-ui,-apple-system,sans-serif;background:var(--bg);color:var(--fg);line-height:1.6;overflow-x:hidden}
a{color:var(--accent);text-decoration:none}
a:hover{text-decoration:underline}

.hero{min-height:90vh;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;padding:2rem;position:relative}
.hero::before{content:'';position:absolute;inset:0;background:radial-gradient(ellipse at 50% 0%,rgba(108,99,255,0.12) 0%,transparent 60%);pointer-events:none}
.hero h1{font-size:clamp(2.5rem,6vw,4.5rem);font-weight:800;letter-spacing:-0.03em;margin-bottom:0.5rem}
.hero h1 span{background:linear-gradient(135deg,var(--accent),var(--accent2));-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}
.hero .sub{font-size:clamp(1.1rem,2.5vw,1.5rem);color:var(--muted);max-width:640px;margin:0 auto 2rem}
.pill{display:inline-block;padding:0.35rem 1rem;border-radius:999px;font-size:0.85rem;border:1px solid var(--border);color:var(--muted);margin-bottom:1.5rem}
.cta{display:inline-flex;gap:1rem;flex-wrap:wrap;justify-content:center}
.btn{padding:0.75rem 2rem;border-radius:8px;font-weight:600;font-size:1rem;transition:all 0.2s}
.btn-primary{background:var(--accent);color:#fff}
.btn-primary:hover{background:#5a52e0;text-decoration:none}
.btn-outline{border:1px solid var(--border);color:var(--fg)}
.btn-outline:hover{border-color:var(--accent);text-decoration:none}

section{max-width:1100px;margin:0 auto;padding:4rem 2rem}
.features{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:1.5rem}
.card{background:var(--card);border:1px solid var(--border);border-radius:12px;padding:1.75rem;transition:border-color 0.2s}
.card:hover{border-color:var(--accent)}
.card .icon{font-size:1.5rem;margin-bottom:0.75rem}
.card h3{font-size:1.15rem;margin-bottom:0.5rem}
.card p{color:var(--muted);font-size:0.95rem}

.code-block{max-width:760px;margin:0 auto;padding:0 2rem 2rem}
.code-block pre{background:var(--card);border:1px solid var(--border);border-radius:12px;padding:1.5rem;overflow-x:auto;font-family:'SF Mono',Consolas,monospace;font-size:0.9rem;line-height:1.7}
.code-block .cm{color:var(--muted)}
.code-block .kw{color:var(--accent)}
.code-block .s{color:var(--accent2)}
.code-block .dim{color:#555}

.section-title{text-align:center;font-size:clamp(1.5rem,3vw,2.25rem);font-weight:700;margin-bottom:0.5rem;padding-top:2rem}
.section-sub{text-align:center;color:var(--muted);margin-bottom:2.5rem;font-size:1.05rem}

.arch{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:1rem;margin-bottom:2rem}
.arch-box{background:var(--card);border:1px solid var(--border);border-radius:10px;padding:1.25rem;text-align:center}
.arch-box h4{font-size:0.95rem;margin-bottom:0.4rem;color:var(--accent2)}
.arch-box p{color:var(--muted);font-size:0.85rem}

.type-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:0.75rem;margin-top:1rem}
.type-chip{background:var(--card);border:1px solid var(--border);border-radius:8px;padding:0.6rem 1rem;font-family:'SF Mono',Consolas,monospace;font-size:0.85rem;color:var(--accent2)}

.trait-table{width:100%;border-collapse:collapse;margin-top:1rem}
.trait-table th{text-align:left;padding:0.6rem 1rem;color:var(--accent);font-weight:600;font-size:0.85rem;text-transform:uppercase;letter-spacing:0.05em;border-bottom:1px solid var(--border)}
.trait-table td{padding:0.6rem 1rem;border-bottom:1px solid var(--border);font-size:0.9rem}
.trait-table td:first-child{font-family:'SF Mono',Consolas,monospace;color:var(--accent2);white-space:nowrap}
.trait-table td:nth-child(2){color:var(--muted)}
.trait-table tr:hover{background:rgba(108,99,255,0.04)}

.iface-flow{display:flex;flex-wrap:wrap;gap:0.75rem;align-items:center;justify-content:center;margin:1.5rem 0;font-family:'SF Mono',Consolas,monospace;font-size:0.85rem}
.iface-step{background:var(--card);border:1px solid var(--border);border-radius:8px;padding:0.5rem 1rem}
.iface-arrow{color:var(--muted)}

.stats{display:flex;gap:2rem;justify-content:center;flex-wrap:wrap;margin:2rem 0}
.stat{text-align:center}
.stat .num{font-size:2rem;font-weight:800;background:linear-gradient(135deg,var(--accent),var(--accent2));-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}
.stat .label{color:var(--muted);font-size:0.85rem;margin-top:0.25rem}

footer{text-align:center;padding:3rem 2rem;color:var(--muted);font-size:0.9rem;border-top:1px solid var(--border)}
footer a{color:var(--accent)}
"##;

const CODE_SECRETS: &str = r##"<span class="cm"># CLI &mdash; manage secrets from the command line</span>
<span class="kw">$</span> traits secrets set fly_api_token <span class="s">"FlyV1 ..."</span>
<span class="kw">$</span> traits secrets list
<span class="cm"># [&quot;admin_password&quot;, &quot;fly_api_token&quot;]  &mdash; values never exposed</span>

<span class="cm"># Rust &mdash; scoped access in your trait</span>
<span class="kw">let</span> ctx = SecretContext::resolve(&amp;[<span class="s">"fly_api_token"</span>]);
<span class="kw">let</span> token = ctx.get(<span class="s">"fly_api_token"</span>).unwrap();
<span class="cm">// token is zeroized when ctx drops</span>"##;

const CODE_INTERFACES: &str = r##"<span class="cm"># kernel/main/main.trait.toml</span>
[trait]
description = <span class="s">"Binary entry point"</span>

<span class="cm"># Declare what this trait needs</span>
[requires]
dispatcher = <span class="s">"kernel/dispatcher"</span>
registry   = <span class="s">"kernel/registry"</span>
config     = <span class="s">"kernel/config"</span>

<span class="cm"># Wire each slot to a concrete provider</span>
[bindings]
dispatcher = <span class="s">"kernel.dispatcher"</span>
registry   = <span class="s">"kernel.registry"</span>
config     = <span class="s">"kernel.config"</span>"##;

const CODE_TRAIT_DEF: &str = r##"<span class="cm"># traits/sys/checksum/checksum.trait.toml</span>
[trait]
description = <span class="s">"SHA-256 checksums"</span>
version     = <span class="s">"v260320.142947"</span>
author      = <span class="s">"system"</span>
tags        = [<span class="s">"system"</span>, <span class="s">"crypto"</span>]
provides    = [<span class="s">"sys/checksum"</span>]

[<span class="kw">signature</span>]
params = [
  { name = <span class="s">"mode"</span>,  type = <span class="s">"string"</span>, description = <span class="s">"hash | io_pairs | signature"</span> },
  { name = <span class="s">"input"</span>, type = <span class="s">"any"</span>,    description = <span class="s">"Data to checksum"</span> },
]

[signature.returns]
type = <span class="s">"string"</span>
description = <span class="s">"Hex-encoded SHA-256 hash"</span>

[<span class="kw">implementation</span>]
language = <span class="s">"rust"</span>
source   = <span class="s">"builtin"</span>    <span class="cm"># compiled directly into the binary</span>
entry    = <span class="s">"checksum"</span>"##;

const CODE_CLI: &str = r##"<span class="cm"># CLI &mdash; every sys.* trait is a direct subcommand</span>
<span class="kw">$</span> traits serve <span class="dim">--port 8090</span>
<span class="kw">$</span> traits list
<span class="kw">$</span> traits checksum hash <span class="s">"hello"</span>
<span class="s">"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"</span>
<span class="kw">$</span> traits info sys.checksum
<span class="kw">$</span> traits test_runner <span class="s">'*'</span>       <span class="cm"># run all .features.json tests</span>
<span class="kw">$</span> echo hello | traits checksum hash  <span class="cm"># pipe support</span>"##;

const CODE_REST: &str = r##"<span class="cm"># REST API &mdash; POST /traits/{namespace}/{name}</span>
<span class="kw">$</span> curl -X POST https://traits.build/traits/sys/checksum \
    -H <span class="s">'Content-Type: application/json'</span> \
    -d <span class="s">'{"args": ["hash", "hello"]}'</span>

<span class="cm"># List everything</span>
<span class="kw">$</span> curl https://traits.build/traits/sys/list

<span class="cm"># Health check</span>
<span class="kw">$</span> curl https://traits.build/health</pre>"##;

const CODE_BROWSER: &str = r##"<span class="cm">// Browser &mdash; TraitsSDK (WASM → helper → REST cascade)</span>
<span class="kw">const</span> sdk = window._traitsSDK;

<span class="cm">// Call any trait from JavaScript</span>
<span class="kw">const</span> hash = <span class="kw">await</span> sdk.call(<span class="s">"sys.checksum"</span>, [<span class="s">"hash"</span>, <span class="s">"hello"</span>]);
<span class="cm">// → "2cf24dba..."  (resolved via WASM in &lt;1ms)</span>

<span class="cm">// Or use Trait Components — zero JS needed</span>
<span class="cm">// &lt;div data-trait="sys.list" data-render="html"&gt;&lt;/div&gt;</span>"##;
