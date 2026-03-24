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
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.min.css">
<link rel="stylesheet" href="/static/www/terminal/terminal.css">
<style>
  body {
    margin: 0; padding: 0; background: #0d1117; color: #c9d1d9;
    font-family: system-ui, -apple-system, sans-serif;
  }
  #loading {
    display: flex; align-items: center; justify-content: center;
    height: 100vh; font-size: 1.1rem; color: #8b949e;
  }
  #loading.hidden { display: none; }
  /* Force dark background on Redoc's middle panel */
  .redoc-wrap { background: #0d1117 !important; }
  .redoc-wrap > div > div:nth-child(2) { background: #0d1117 !important; }
  /* Dark backgrounds for schema/content areas */
  [class*="middle-panel"] { background: #0d1117 !important; }
  table, th, td { border-color: #30363d !important; }
  th { background: #161b22 !important; }
  td { background: #0d1117 !important; }
  /* Invert text colors for readability */
  .redoc-wrap h1, .redoc-wrap h2, .redoc-wrap h3, .redoc-wrap h4, .redoc-wrap h5 {
    color: #f0f6fc !important;
  }
  .redoc-wrap p, .redoc-wrap span, .redoc-wrap li, .redoc-wrap label, .redoc-wrap td {
    color: #c9d1d9 !important;
  }
  /* Nested schema backgrounds */
  .redoc-wrap [kind="field"] { border-color: #30363d !important; }
  .redoc-wrap button { color: #c9d1d9 !important; }
  /* Buttons/tabs with light backgrounds need dark text */
  .redoc-wrap button[class*="tab"], .redoc-wrap [role="tab"] { color: #0d1117 !important; }
  .redoc-wrap ul[role="tablist"] button { color: #0d1117 !important; }
  .redoc-wrap ul[role="tablist"] li { color: #0d1117 !important; }
  /* Example selector dropdown */
  .redoc-wrap select, .redoc-wrap option { color: #0d1117 !important; background: #f0f6fc !important; }
  /* White-background containers in middle panel */
  .redoc-wrap div[class*="dropdown"] { color: #0d1117 !important; }
  .redoc-wrap code { background: #161b22 !important; }
  /* Leave space for fixed terminal */
  body { padding-bottom: 340px; }
</style>
</head>
<body>
<div id="loading">Loading API documentation…</div>
<div id="redoc"></div>
<script src="https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js"></script>
<script>
(async function() {
  var spec = null;
  // Use SDK if available (WASM-first, REST-fallback)
  if (window._traitsSDK) {
    try {
      var r = await window._traitsSDK.call('sys.openapi', []);
      if (r.ok) spec = r.result;
    } catch(e) { console.warn('SDK openapi failed:', e); }
  }
  // Fallback to direct REST
  if (!spec) {
    try {
      var res = await fetch('/traits/sys/openapi', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args: [] })
      });
      var data = await res.json();
      spec = data.result;
    } catch(e) {}
  }
  document.getElementById('loading').className = 'hidden';
  if (!spec || !spec.openapi) {
    var isLocal = location.protocol === 'file:';
    document.getElementById('redoc').innerHTML = isLocal
      ? '<div style="padding:2rem;text-align:center;color:var(--fg,#c9d1d9)">' +
        '<h2 style="margin-bottom:1rem;color:#f0f6fc">API Reference</h2>' +
        '<p style="color:#8b949e;max-width:480px;margin:0 auto">The interactive API documentation requires a running server to generate the OpenAPI spec.</p>' +
        '<p style="margin-top:1rem"><code style="background:#161b22;padding:0.3rem 0.6rem;border-radius:4px;color:#8bdb8b">./target/release/traits serve -p 8091</code></p>' +
        '<p style="margin-top:0.5rem;color:#8b949e">Then visit <a href="http://127.0.0.1:8091/docs/api">http://127.0.0.1:8091/docs/api</a></p></div>'
      : '<div style="padding:2rem;color:#f85149">Failed to load OpenAPI spec.</div>';
    return;
  }
    Redoc.init(spec, {
      theme: {
        colors: {
          primary: { main: '#f97316' },
          text: { primary: '#c9d1d9', secondary: '#8b949e' },
          http: { post: '#f97316', get: '#58a6ff', put: '#d29922', delete: '#f85149' },
          border: { dark: '#30363d', light: '#21262d' },
          responses: { success: { backgroundColor: '#0d1117' }, error: { backgroundColor: '#0d1117' } }
        },
        typography: {
          fontFamily: 'system-ui, -apple-system, sans-serif',
          headings: { fontFamily: 'system-ui, -apple-system, sans-serif' },
          code: { backgroundColor: '#161b22' }
        },
        schema: {
          nestedBackground: '#161b22',
          typeNameColor: '#f97316'
        },
        sidebar: {
          backgroundColor: '#010409',
          textColor: '#8b949e',
          activeTextColor: '#f0f6fc',
          groupItems: { activeTextColor: '#f97316' }
        },
        rightPanel: {
          backgroundColor: '#161b22'
        }
      },
      pathInMiddlePanel: true,
      expandResponses: '200',
      hideDownloadButton: false,
      sortPropsAlphabetically: true
    }, document.getElementById('redoc'));
})();
</script>

<div class="terminal-wrap">
  <div class="terminal-header" id="termHeader">
    <button id="btnToggleTerm" class="terminal-toggle">▼ Terminal</button>
    <span class="terminal-hint">WASM-powered traits CLI — try "list" or "call sys.checksum hash hello"</span>
    <span id="termStatus" class="terminal-status"></span>
  </div>
  <div id="termContainer" class="terminal-container">
    <div id="xterm" class="xterm-mount"></div>
  </div>
</div>
<script type="module">
import { createTerminal } from '/static/www/terminal/terminal.js';
createTerminal(document.getElementById('xterm'), {
    header: document.getElementById('termHeader'),
    container: document.getElementById('termContainer'),
    toggleBtn: document.getElementById('btnToggleTerm'),
    statusEl: document.getElementById('termStatus'),
});
</script>
<script>
// Fallback for SPA/file:// mode where module imports from absolute paths fail.
// Dynamically import terminal.js, trying multiple paths.
(async function() {
  if (document.querySelector('#xterm canvas')) return; // Already initialized by module script
  var paths = ['/static/www/terminal/terminal.js', '../terminal/terminal.js'];
  var mod = null;
  for (var p of paths) {
    try { mod = await import(p); break; } catch(e) {}
  }
  if (!mod) return; // Terminal unavailable in this mode — Redoc still works
  mod.createTerminal(document.getElementById('xterm'), {
    header: document.getElementById('termHeader'),
    container: document.getElementById('termContainer'),
    toggleBtn: document.getElementById('btnToggleTerm'),
    statusEl: document.getElementById('termStatus'),
  });
})();
</script>
</body>
</html>"##;
