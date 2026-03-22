use serde_json::Value;
use std::collections::HashSet;

pub fn website(_args: &[Value]) -> Value {
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
    let html = HTML
        .replace("{{TRAIT_COUNT}}", &trait_count.to_string())
        .replace("{{NS_COUNT}}", &ns_count.to_string());
    Value::String(html)
}

const HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>traits.build — composable function kernel</title>
<style>
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
</style>
</head>
<body>

<!-- Hero -->
<div class="hero">
  <div class="pill">open source &middot; pure Rust &middot; AI-ready</div>
  <h1><span>traits</span>.build</h1>
  <p class="sub">Typed, composable function objects, ready for AI development, compiled into a single Rust binary. Define traits in TOML, call them via CLI, REST, or MCP. The kernel is traits all the way down.</p>
  <div class="cta">
    <a href="#built-in-traits" class="btn btn-primary">Explore Traits</a>
    <a href="/docs" class="btn btn-outline">Documentation</a>
    <a href="/docs/api" class="btn btn-outline">API Docs</a>
    <a href="https://github.com/kilian-ai/traits.build" class="btn btn-outline">GitHub</a>
  </div>
</div>

<!-- Stats -->
<div class="stats">
  <div class="stat"><div class="num">{{TRAIT_COUNT}}</div><div class="label">compiled traits</div></div>
  <div class="stat"><div class="num">{{NS_COUNT}}</div><div class="label">namespaces</div></div>
  <div class="stat"><div class="num">~2 MB</div><div class="label">binary size</div></div>
  <div class="stat"><div class="num">0</div><div class="label">runtime deps</div></div>
</div>

<!-- Architecture -->
<h2 class="section-title">Architecture</h2>
<p class="section-sub">The kernel is traits all the way down</p>

<section>
<div class="arch">
  <div class="arch-box">
    <h4>CLI &amp; HTTP</h4>
    <p>sys.cli parses args, kernel.serve starts actix-web. Every sys.* trait is a direct subcommand.</p>
  </div>
  <div class="arch-box">
    <h4>Dispatcher</h4>
    <p>Resolves paths, validates &amp; coerces arguments, dispatches to compiled Rust trait functions.</p>
  </div>
  <div class="arch-box">
    <h4>Registry</h4>
    <p>Concurrent DashMap of .trait.toml definitions. Loads from disk + compiled builtins. Hot-reloadable.</p>
  </div>
  <div class="arch-box">
    <h4>Interface System</h4>
    <p>provides / requires / bindings. Per-call overrides, global bindings, or auto-discovery.</p>
  </div>
  <div class="arch-box">
    <h4>Plugin Loader</h4>
    <p>kernel.dylib_loader discovers cdylib .dylib plugins at startup for dynamic trait extensions.</p>
  </div>
  <div class="arch-box">
    <h4>Type System</h4>
    <p>int, float, string, bool, bytes, any, list&lt;T&gt;, map&lt;K,V&gt;, T? &mdash; validated at dispatch time.</p>
  </div>
</div>
</section>

<!-- Features -->
<h2 class="section-title">Why Traits?</h2>
<p class="section-sub">Functions as the universal building block</p>

<section>
<div class="features">
  <div class="card">
    <div class="icon">&#x1f680;</div>
    <h3>Single binary, zero runtime</h3>
    <p>Every trait compiles directly into the binary via build.rs. No containers, no workers, no runtime dependencies. One ~2 MB executable does everything.</p>
  </div>
  <div class="card">
    <div class="icon">&#x2699;&#xfe0f;</div>
    <h3>Self-describing kernel</h3>
    <p>Registry, dispatcher, config, CLI, and HTTP server are all traits. Call <code>kernel.main</code> to see every module, interface, and bootstrap step.</p>
  </div>
  <div class="card">
    <div class="icon">&#x1f50c;</div>
    <h3>CLI + REST + MCP</h3>
    <p>Every trait is callable via REST API (<code>POST /traits/ns/name</code>), CLI (<code>traits name args</code>), or MCP tool protocol. One trait, three surfaces.</p>
  </div>
  <div class="card">
    <div class="icon">&#x1f517;</div>
    <h3>Interface wiring</h3>
    <p>Traits declare <code>provides</code>, <code>requires</code>, and <code>bindings</code> in TOML. Resolution: per-call overrides &rarr; global bindings &rarr; auto-discover.</p>
  </div>
  <div class="card">
    <div class="icon">&#x1f9e9;</div>
    <h3>Build-time codegen</h3>
    <p>build.rs discovers all .trait.toml files, generates dispatch tables, embeds definitions, auto-bumps versions and checksums.</p>
  </div>
  <div class="card">
    <div class="icon">&#x1f4e6;</div>
    <h3>cdylib plugins</h3>
    <p>Extend at runtime with .dylib shared libraries. The kernel discovers and loads them at startup via kernel.dylib_loader.</p>
  </div>
</div>
</section>

<!-- AI-Ready -->
<h2 class="section-title">AI-Ready by default</h2>
<p class="section-sub">Every trait is a tool an AI agent can discover, call, and test</p>

<section>
<div class="features">
  <div class="card">
    <div class="icon">&#x1f916;</div>
    <h3>MCP server export</h3>
    <p>Every compiled trait can be exported as an MCP tool. Agents discover traits via the registry, read signatures from TOML, and call them over stdio.</p>
  </div>
  <div class="card">
    <div class="icon">&#x1f9e0;</div>
    <h3>Agent file included</h3>
    <p>The repo ships with a <code>.github/agents/</code> file that teaches AI agents the project structure, build system, conventions, and how to add new traits.</p>
  </div>
  <div class="card">
    <div class="icon">&#x1f4cb;</div>
    <h3>Self-testing traits</h3>
    <p>Every trait has a <code>.features.json</code> with features, examples, and test commands. Agents can run <code>traits test_runner '*'</code> to validate changes.</p>
  </div>
  <div class="card">
    <div class="icon">&#x1f4d6;</div>
    <h3>Auto-generated docs</h3>
    <p>OpenAPI spec is generated live from the trait registry with real response examples. No hand-written API docs &mdash; the kernel documents itself.</p>
  </div>
  <div class="card">
    <div class="icon">&#x2699;&#xfe0f;</div>
    <h3>TOML is the spec</h3>
    <p>Each <code>.trait.toml</code> declares the full contract: signature, types, interfaces, dependencies, and wiring. Machine-readable by design.</p>
  </div>
  <div class="card">
    <div class="icon">&#x1f6e0;&#xfe0f;</div>
    <h3>Workspace-ready</h3>
    <p>VS Code workspace ships with build/test/serve tasks, MCP server config, and editor settings. Open and go.</p>
  </div>
</div>
</section>

<!-- Kernel traits -->
<h2 class="section-title" id="built-in-traits">Built-in traits</h2>
<p class="section-sub">{{TRAIT_COUNT}} traits across kernel, sys, and www &mdash; all compiled in</p>

<section>
<table class="trait-table">
<tr><th>Trait</th><th>What it does</th></tr>
<tr><td>kernel.main</td><td>Entry point, bootstrap, compiled module list, interface introspection</td></tr>
<tr><td>kernel.dispatcher</td><td>Path resolution, argument validation &amp; coercion, compiled dispatch</td></tr>
<tr><td>kernel.registry</td><td>Load .trait.toml definitions, DashMap lookup, interface resolution</td></tr>
<tr><td>kernel.config</td><td>traits.toml parsing, env var overrides, runtime config</td></tr>
<tr><td>kernel.serve</td><td>Start the actix-web HTTP server with CORS &amp; SSE streaming</td></tr>
<tr><td>kernel.dylib_loader</td><td>Discover and load cdylib .dylib plugins at startup</td></tr>
<tr><td>kernel.types</td><td>TraitValue, TraitType, type parsing (list&lt;map&lt;string,int&gt;&gt;)</td></tr>
<tr><td>kernel.globals</td><td>OnceLock statics: REGISTRY, CONFIG, TRAITS_DIR, HANDLES</td></tr>
<tr><td>kernel.call</td><td>Call any trait by dot-notation path (inter-trait dispatch)</td></tr>
<tr><td>kernel.plugin_api</td><td>C ABI export macro for cdylib trait plugins</td></tr>
<tr><td>kernel.reload</td><td>Hot-reload the trait registry from disk</td></tr>
<tr><td>sys.cli</td><td>Clap parsing, subcommand dispatch, arg coercion, pipe support</td></tr>
<tr><td>sys.registry</td><td>Read API: list, info, tree, namespaces, count, search</td></tr>
<tr><td>sys.checksum</td><td>SHA-256 checksums for strings, I/O pairs, and signatures</td></tr>
<tr><td>sys.version</td><td>Generate YYMMDD date-format versions</td></tr>
<tr><td>sys.snapshot</td><td>Snapshot a trait version (YYMMDD or YYMMDD.HHMMSS)</td></tr>
<tr><td>sys.test_runner</td><td>Run .features.json tests &mdash; dispatch examples + shell commands</td></tr>
<tr><td>sys.list</td><td>List all registered traits</td></tr>
<tr><td>sys.info</td><td>Show detailed trait metadata and signatures</td></tr>
<tr><td>sys.ps</td><td>List running background traits with process details</td></tr>
<tr><td>sys.openapi</td><td>Generate OpenAPI 3.0 spec with live examples from the registry</td></tr>
<tr><td>www.traits.build</td><td>This landing page</td></tr>
<tr><td>www.admin</td><td>Admin dashboard with deployment controls (Basic Auth)</td></tr>
<tr><td>www.admin.deploy</td><td>Deploy to Fly.io</td></tr>
<tr><td>www.admin.scale</td><td>Scale Fly.io machines up or down</td></tr>
<tr><td>www.admin.destroy</td><td>Destroy Fly.io machines</td></tr>
<tr><td>www.admin.fast_deploy</td><td>Fast deploy: Docker build + sftp upload + restart</td></tr>
<tr><td>www.docs.api</td><td>Serve Redoc API documentation page</td></tr>
<tr><td>www.docs</td><td>Single-page documentation with all guides rendered from markdown</td></tr>
<tr><td>sys.mcp</td><td>MCP stdio server &mdash; JSON-RPC 2.0 over stdin/stdout</td></tr>
</table>
</section>

<!-- Interface system -->
<h2 class="section-title">Interface system</h2>
<p class="section-sub">Declare dependencies as typed contracts, not hard-coded paths</p>

<section>
<div class="code-block" style="max-width:100%;padding:0">
<pre><span class="cm"># kernel/main/main.trait.toml</span>
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
config     = <span class="s">"kernel.config"</span></pre>
</div>
<p style="text-align:center;color:var(--muted);margin:1.5rem 0 0.5rem">Resolution chain</p>
<div class="iface-flow">
  <span class="iface-step">per-call overrides</span>
  <span class="iface-arrow">&rarr;</span>
  <span class="iface-step">global bindings</span>
  <span class="iface-arrow">&rarr;</span>
  <span class="iface-step">caller [bindings]</span>
  <span class="iface-arrow">&rarr;</span>
  <span class="iface-step">auto-discover</span>
</div>
</section>

<!-- Trait definition format -->
<h2 class="section-title">Trait definition format</h2>
<p class="section-sub">.trait.toml &mdash; one file defines everything</p>

<section>
<div class="code-block" style="max-width:100%;padding:0">
<pre><span class="cm"># traits/sys/checksum/checksum.trait.toml</span>
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
entry    = <span class="s">"checksum"</span></pre>
</div>
<div class="type-grid">
  <div class="type-chip">int</div>
  <div class="type-chip">float</div>
  <div class="type-chip">string</div>
  <div class="type-chip">bool</div>
  <div class="type-chip">bytes</div>
  <div class="type-chip">any</div>
  <div class="type-chip">list&lt;T&gt;</div>
  <div class="type-chip">map&lt;K,V&gt;</div>
  <div class="type-chip">T? (optional)</div>
  <div class="type-chip">handle</div>
</div>
</section>

<!-- Quick start -->
<h2 class="section-title">Quick start</h2>
<p class="section-sub">Two ways to talk to every trait</p>

<section>
<div class="code-block" style="max-width:100%;padding:0;margin-bottom:2rem">
<pre><span class="cm"># CLI &mdash; every sys.* trait is a direct subcommand</span>
<span class="kw">$</span> traits serve <span class="dim">--port 8090</span>
<span class="kw">$</span> traits list
<span class="kw">$</span> traits checksum hash <span class="s">"hello"</span>
<span class="s">"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"</span>
<span class="kw">$</span> traits info sys.checksum
<span class="kw">$</span> traits test_runner <span class="s">'*'</span>       <span class="cm"># run all .features.json tests</span>
<span class="kw">$</span> echo hello | traits checksum hash  <span class="cm"># pipe support</span></pre>
</div>

<div class="code-block" style="max-width:100%;padding:0">
<pre><span class="cm"># REST API &mdash; POST /traits/{namespace}/{name}</span>
<span class="kw">$</span> curl -X POST https://traits.build/traits/sys/checksum \
    -H <span class="s">'Content-Type: application/json'</span> \
    -d <span class="s">'{"args": ["hash", "hello"]}'</span>

<span class="cm"># List everything</span>
<span class="kw">$</span> curl https://traits.build/traits/sys/list

<span class="cm"># Health check</span>
<span class="kw">$</span> curl https://traits.build/health</pre>
</div>
</section>

<!-- How it's built -->
<h2 class="section-title">How it works</h2>
<p class="section-sub">build.rs discovers traits, generates dispatch, compiles everything in</p>

<section>
<div class="code-block" style="max-width:100%;padding:0">
<pre><span class="cm"># 1. Define a trait (TOML + Rust source)</span>
traits/sys/checksum/checksum.trait.toml
traits/sys/checksum/checksum.rs

<span class="cm"># 2. build.rs discovers it automatically</span>
<span class="cm">#    - Embeds the TOML via include_str!</span>
<span class="cm">#    - Generates mod declarations</span>
<span class="cm">#    - Creates dispatch_compiled() match arms</span>
<span class="cm">#    - Validates checksums, bumps versions</span>

<span class="cm"># 3. cargo build produces a single binary</span>
<span class="kw">$</span> cargo build --release
<span class="cm">#    target/release/traits (~2 MB)</span>

<span class="cm"># 4. Run it anywhere</span>
<span class="kw">$</span> ./traits serve --port 8090
<span class="cm">#    {{TRAIT_COUNT}} traits loaded, 0 workers, 0 dependencies</span></pre>
</div>
</section>

<footer>
  <p>traits.build &mdash; a pure Rust kernel, AI-ready by default. &middot; <a href="/docs">Docs</a> &middot; <a href="/docs/api">API Docs</a> &middot; <a href="https://github.com/kilian-ai/traits.build">GitHub</a> &middot; <a href="/traits/kernel/main">kernel.main</a> &middot; <a href="/traits/sys/list">sys.list</a> &middot; <a href="/health">health</a></p>
</footer>

</body>
</html>"##;
