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
  .redoc-wrap code { background: #161b22 !important; }
</style>
</head>
<body>
<div id="loading">Loading API documentation…</div>
<div id="redoc"></div>
<script src="https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js"></script>
<script>
(function() {
  fetch('/traits/sys/openapi', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ args: [] })
  })
  .then(function(r) { return r.json(); })
  .then(function(data) {
    document.getElementById('loading').className = 'hidden';
    var spec = data.result;
    if (!spec || !spec.openapi) {
      document.getElementById('redoc').innerHTML =
        '<div style="padding:2rem;color:#f85149">Failed to load OpenAPI spec.</div>';
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
  })
  .catch(function(err) {
    document.getElementById('loading').className = 'hidden';
    document.getElementById('redoc').innerHTML =
      '<div style="padding:2rem;color:#f85149">Error loading spec: ' + err.message + '</div>';
  });
})();
</script>
</body>
</html>"##;
