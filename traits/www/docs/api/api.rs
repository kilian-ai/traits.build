use serde_json::Value;

/// www.docs.api — Serve the Redoc API documentation page.
///
/// Returns an HTML page that loads Redoc from CDN and fetches the OpenAPI spec
/// from the sys.openapi trait at runtime.
pub fn api_docs(_args: &[Value]) -> Value {
    Value::String(HTML.to_string())
}

const HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>API Reference — traits.build</title>
<meta name="description" content="REST API documentation for the traits.build composable function kernel">
<style>
  body { margin:0; padding:0; background:#0d1117; color:#c9d1d9; font-family:system-ui,-apple-system,sans-serif; }
  a { color:#58a6ff; text-decoration:none; }
  a:hover { text-decoration:underline; }

  .topnav { display:flex; align-items:center; gap:1.5rem; padding:0.75rem 1.5rem; background:#0d1117; border-bottom:1px solid #222; position:sticky; top:0; z-index:50; }
  .topnav .brand { font-size:1.05rem; font-weight:600; color:#e0e0e0; }
  .topnav .brand span { color:#f97316; }
  .topnav .tabs { display:flex; gap:0.25rem; margin-left:1rem; }
  .topnav .tab { padding:0.4rem 0.9rem; border-radius:6px; color:#999; font-size:0.9rem; }
  .topnav .tab:hover { background:#161b22; color:#e0e0e0; text-decoration:none; }
  .topnav .tab.active { background:#161b22; color:#f97316; }

  #loading { display:flex; align-items:center; justify-content:center; height:60vh; font-size:1.1rem; color:#8b949e; }
  #loading.hidden { display:none; }

  .redoc-wrap { background:#0d1117 !important; }
  .redoc-wrap > div > div:nth-child(2) { background:#0d1117 !important; }
  [class*="middle-panel"] { background:#0d1117 !important; }
  table, th, td { border-color:#30363d !important; }
  th { background:#161b22 !important; }
  td { background:#0d1117 !important; }
  .redoc-wrap h1, .redoc-wrap h2, .redoc-wrap h3, .redoc-wrap h4, .redoc-wrap h5 { color:#f0f6fc !important; }
  .redoc-wrap p, .redoc-wrap span, .redoc-wrap li, .redoc-wrap label, .redoc-wrap td { color:#c9d1d9 !important; }
  .redoc-wrap [kind="field"] { border-color:#30363d !important; }
  .redoc-wrap button { color:#c9d1d9 !important; }
  .redoc-wrap button[class*="tab"], .redoc-wrap [role="tab"] { color:#0d1117 !important; }
  .redoc-wrap ul[role="tablist"] button, .redoc-wrap ul[role="tablist"] li { color:#0d1117 !important; }
  .redoc-wrap select, .redoc-wrap option { color:#0d1117 !important; background:#f0f6fc !important; }
  .redoc-wrap div[class*="dropdown"] { color:#0d1117 !important; }
  .redoc-wrap code { background:#161b22 !important; }
</style>
</head>
<body>
<header class="topnav">
  <a class="brand" href="/traits">traits<span>.build</span></a>
  <nav class="tabs">
    <a class="tab" href="/traits">traits</a>
    <a class="tab" href="/build">build</a>
    <a class="tab active" href="/api">api</a>
  </nav>
</header>
<div id="loading">Loading API documentation…</div>
<div id="redoc"></div>
<script src="https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js"></script>
<script>
async function loadSpec() {
  for (const url of ['./openapi.json', '/openapi.json']) {
    try { const r = await fetch(url); if (r.ok) return await r.json(); } catch (_) {}
  }
  return null;
}
(async function() {
  const el = document.getElementById('redoc');
  const spec = await loadSpec();
  document.getElementById('loading').className = 'hidden';
  if (!spec || !spec.openapi) {
    el.innerHTML = '<div style="padding:2rem;color:#f85149">Failed to load OpenAPI spec.</div>';
    return;
  }
  Redoc.init(spec, {
    theme: {
      colors: {
        primary: { main: '#f97316' },
        text: { primary: '#c9d1d9', secondary: '#8b949e' },
        http: { post:'#f97316', get:'#58a6ff', put:'#d29922', delete:'#f85149' },
        border: { dark:'#30363d', light:'#21262d' },
        responses: { success:{backgroundColor:'#0d1117'}, error:{backgroundColor:'#0d1117'} }
      },
      typography: {
        fontFamily: 'system-ui,-apple-system,sans-serif',
        headings: { fontFamily:'system-ui,-apple-system,sans-serif' },
        code: { backgroundColor:'#161b22' }
      },
      schema: { nestedBackground:'#161b22', typeNameColor:'#f97316' },
      sidebar: {
        backgroundColor:'#010409', textColor:'#8b949e', activeTextColor:'#f0f6fc',
        groupItems: { activeTextColor:'#f97316' }
      },
      rightPanel: { backgroundColor:'#161b22' }
    },
    pathInMiddlePanel: true,
    expandResponses: '200',
    hideDownloadButton: false,
    sortPropsAlphabetically: true
  }, el);
})();
</script>
</body>
</html>"##;
