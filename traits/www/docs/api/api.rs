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
  body { margin: 0; padding: 0; font-family: system-ui, -apple-system, sans-serif; }
  #loading {
    display: flex; align-items: center; justify-content: center;
    height: 100vh; font-size: 1.1rem; color: #666;
  }
  #loading.hidden { display: none; }
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
        '<div style="padding:2rem;color:#c00">Failed to load OpenAPI spec.</div>';
      return;
    }
    Redoc.init(spec, {
      theme: {
        colors: {
          primary: { main: '#f97316' }
        },
        typography: {
          fontFamily: 'system-ui, -apple-system, sans-serif',
          headings: { fontFamily: 'system-ui, -apple-system, sans-serif' }
        },
        sidebar: {
          backgroundColor: '#1a1a2e',
          textColor: '#eee'
        },
        rightPanel: {
          backgroundColor: '#1a1a2e'
        }
      },
      expandResponses: '200',
      hideDownloadButton: false,
      sortPropsAlphabetically: true
    }, document.getElementById('redoc'));
  })
  .catch(function(err) {
    document.getElementById('loading').className = 'hidden';
    document.getElementById('redoc').innerHTML =
      '<div style="padding:2rem;color:#c00">Error loading spec: ' + err.message + '</div>';
  });
})();
</script>
</body>
</html>"##;
