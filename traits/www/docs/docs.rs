use serde_json::Value;
use pulldown_cmark::{Parser, Options, html};
use std::sync::OnceLock;

struct DocPage {
    slug: &'static str,
    title: &'static str,
    markdown: &'static str,
    category: &'static str,
}

const PAGES: &[DocPage] = &[
    DocPage { slug: "intro",            title: "Overview",          markdown: include_str!("../../../docs/intro.md"),            category: "" },
    DocPage { slug: "getting-started",  title: "Getting Started",   markdown: include_str!("../../../docs/getting-started.md"),  category: "" },
    DocPage { slug: "architecture",     title: "Architecture",      markdown: include_str!("../../../docs/architecture.md"),     category: "Core Concepts" },
    DocPage { slug: "trait-definition", title: "Trait Definition",  markdown: include_str!("../../../docs/trait-definition.md"), category: "Core Concepts" },
    DocPage { slug: "interfaces",       title: "Interfaces",        markdown: include_str!("../../../docs/interfaces.md"),       category: "Core Concepts" },
    DocPage { slug: "type-system",      title: "Type System",       markdown: include_str!("../../../docs/type-system.md"),      category: "Core Concepts" },
    DocPage { slug: "rest-api",         title: "REST API",          markdown: include_str!("../../../docs/rest-api.md"),         category: "Reference" },
    DocPage { slug: "cli",              title: "CLI Reference",     markdown: include_str!("../../../docs/cli.md"),              category: "Reference" },
    DocPage { slug: "creating-traits",  title: "Creating Traits",   markdown: include_str!("../../../docs/creating-traits.md"),  category: "Guides" },
    DocPage { slug: "deployment",       title: "Deployment",        markdown: include_str!("../../../docs/deployment.md"),       category: "Guides" },
];

/// Strip YAML frontmatter (---...---) from markdown
fn strip_frontmatter(md: &str) -> &str {
    if md.starts_with("---") {
        if let Some(end) = md[3..].find("---") {
            let after = 3 + end + 3;
            return md[after..].trim_start_matches('\n');
        }
    }
    md
}

fn render_markdown(md: &str) -> String {
    let clean = strip_frontmatter(md);
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(clean, opts);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

fn build_html() -> String {
    // Build sidebar HTML
    let mut sidebar = String::new();
    let mut current_cat = "";
    for page in PAGES {
        if page.category != current_cat {
            if !current_cat.is_empty() {
                sidebar.push_str("</div>");
            }
            if !page.category.is_empty() {
                sidebar.push_str(&format!(
                    r#"<div class="sb-cat"><div class="sb-cat-label">{}</div>"#,
                    page.category
                ));
            }
            current_cat = page.category;
        }
        sidebar.push_str(&format!(
            r##"<a class="sb-link" href="#{}" onclick="showPage('{}'); return false;">{}</a>"##,
            page.slug, page.slug, page.title
        ));
    }
    if !current_cat.is_empty() {
        sidebar.push_str("</div>");
    }

    // Render all pages
    let mut pages_html = String::new();
    for (i, page) in PAGES.iter().enumerate() {
        let display = if i == 0 { "block" } else { "none" };
        let content = render_markdown(page.markdown);
        pages_html.push_str(&format!(
            r#"<div id="page-{}" class="doc-page" style="display:{}">{}</div>"#,
            page.slug, display, content
        ));
    }

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Documentation — traits.build</title>
<meta name="description" content="Documentation for the traits.build composable function kernel">
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github-dark.min.css">
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: #0a0a0a; color: #c9d1d9; }}
  a {{ color: #58a6ff; text-decoration: none; }}
  a:hover {{ text-decoration: underline; }}

  .layout {{ display: flex; min-height: 100vh; }}

  /* Sidebar */
  .sidebar {{ width: 260px; min-width: 260px; background: #111; border-right: 1px solid #222; padding: 1.5rem 0; position: sticky; top: 0; height: 100vh; overflow-y: auto; }}
  .sb-brand {{ padding: 0 1.25rem 1.25rem; border-bottom: 1px solid #222; margin-bottom: 1rem; }}
  .sb-brand a {{ color: #e0e0e0; font-size: 1.1rem; font-weight: 600; }}
  .sb-brand span {{ color: #666; font-weight: 300; }}
  .sb-link {{ display: block; padding: 0.4rem 1.25rem; color: #999; font-size: 0.9rem; cursor: pointer; border-left: 2px solid transparent; transition: all 0.1s; }}
  .sb-link:hover {{ color: #e0e0e0; background: #1a1a1a; text-decoration: none; }}
  .sb-link.active {{ color: #58a6ff; border-left-color: #58a6ff; background: #0d1117; }}
  .sb-cat {{ margin-top: 0.75rem; }}
  .sb-cat-label {{ padding: 0.4rem 1.25rem; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; color: #555; font-weight: 600; }}

  /* Content */
  .content {{ flex: 1; max-width: 800px; padding: 2.5rem 3rem; }}
  .doc-page h1 {{ font-size: 2rem; color: #f0f6fc; margin-bottom: 0.5rem; border-bottom: 1px solid #222; padding-bottom: 0.75rem; }}
  .doc-page h2 {{ font-size: 1.4rem; color: #e0e0e0; margin-top: 2rem; margin-bottom: 0.5rem; border-bottom: 1px solid #1a1a1a; padding-bottom: 0.4rem; }}
  .doc-page h3 {{ font-size: 1.1rem; color: #ccc; margin-top: 1.5rem; margin-bottom: 0.4rem; }}
  .doc-page p {{ line-height: 1.7; margin-bottom: 1rem; color: #b0b0b0; }}
  .doc-page ul, .doc-page ol {{ margin-bottom: 1rem; padding-left: 1.5rem; }}
  .doc-page li {{ line-height: 1.7; margin-bottom: 0.3rem; color: #b0b0b0; }}
  .doc-page strong {{ color: #e0e0e0; }}
  .doc-page code {{ background: #161b22; padding: 0.15rem 0.4rem; border-radius: 3px; font-family: 'Berkeley Mono', 'SF Mono', 'Fira Code', monospace; font-size: 0.85em; color: #8bdb8b; }}
  .doc-page pre {{ background: #0d1117; border: 1px solid #1a1a1a; border-radius: 6px; padding: 1rem; margin-bottom: 1rem; overflow-x: auto; }}
  .doc-page pre code {{ background: none; padding: 0; color: #c9d1d9; font-size: 0.85rem; }}
  .doc-page table {{ width: 100%; border-collapse: collapse; margin-bottom: 1rem; }}
  .doc-page th {{ text-align: left; padding: 0.5rem 0.75rem; background: #161b22; border: 1px solid #222; color: #e0e0e0; font-size: 0.85rem; }}
  .doc-page td {{ padding: 0.5rem 0.75rem; border: 1px solid #1a1a1a; font-size: 0.85rem; color: #b0b0b0; }}
  .doc-page td code {{ font-size: 0.82rem; }}
  .doc-page blockquote {{ border-left: 3px solid #333; padding-left: 1rem; margin-bottom: 1rem; color: #888; }}

  /* Mobile */
  @media (max-width: 768px) {{
    .sidebar {{ display: none; }}
    .content {{ padding: 1.5rem; }}
  }}
</style>
</head>
<body>
<div class="layout">
  <nav class="sidebar">
    <div class="sb-brand"><a href="/">traits.build <span>docs</span></a></div>
    {sidebar}
  </nav>
  <main class="content">
    {pages_html}
  </main>
</div>
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script>
<script>
function showPage(slug) {{
  document.querySelectorAll('.doc-page').forEach(function(el) {{ el.style.display = 'none'; }});
  document.querySelectorAll('.sb-link').forEach(function(el) {{ el.classList.remove('active'); }});
  var page = document.getElementById('page-' + slug);
  if (page) {{ page.style.display = 'block'; window.scrollTo(0, 0); }}
  var link = document.querySelector('.sb-link[href="#' + slug + '"]');
  if (link) link.classList.add('active');
}}
function initFromHash() {{
  var hash = window.location.hash.slice(1);
  if (hash) showPage(hash);
  else {{
    var first = document.querySelector('.sb-link');
    if (first) first.classList.add('active');
  }}
}}
window.addEventListener('hashchange', initFromHash);
initFromHash();
hljs.highlightAll();
// Re-highlight after page switch
var _origShow = showPage;
showPage = function(slug) {{
  _origShow(slug);
  setTimeout(function() {{ hljs.highlightAll(); }}, 10);
}};
</script>
</body>
</html>"##,
        sidebar = sidebar,
        pages_html = pages_html,
    )
}

static CACHED_HTML: OnceLock<String> = OnceLock::new();

pub fn docs(_args: &[Value]) -> Value {
    let html = CACHED_HTML.get_or_init(build_html);
    Value::String(html.clone())
}
