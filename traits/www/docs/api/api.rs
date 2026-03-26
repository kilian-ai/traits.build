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
  .playground-banner {
    display: none;
    gap: 1rem;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    margin: 1rem;
    border: 1px solid #30363d;
    border-radius: 10px;
    background: #161b22;
  }
  .playground-banner.show { display: flex; }
  .playground-copy h2 {
    margin: 0 0 0.35rem 0;
    font-size: 1rem;
    color: #f0f6fc;
  }
  .playground-copy p {
    margin: 0;
    color: #8b949e;
    font-size: 0.92rem;
  }
  .playground-actions { display: flex; gap: 0.75rem; flex-wrap: wrap; }
  .playground-btn {
    padding: 0.65rem 0.9rem;
    border-radius: 8px;
    border: 1px solid #30363d;
    background: #0d1117;
    color: #f0f6fc;
    font: inherit;
    cursor: pointer;
  }
  .playground-btn.primary {
    background: #f97316;
    border-color: #f97316;
    color: #0d1117;
    font-weight: 600;
  }
  @media (max-width: 720px) {
    .playground-banner { flex-direction: column; align-items: stretch; }
    .playground-actions { width: 100%; }
    .playground-btn { width: 100%; }
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
<div id="playgroundBanner" class="playground-banner">
  <div class="playground-copy">
    <h2>Playground moved here</h2>
    <p>Use the embedded terminal below as the interactive playground while browsing the API reference.</p>
  </div>
  <div class="playground-actions">
    <button id="btnOpenPlaygroundTerm" class="playground-btn primary" type="button">Open Terminal</button>
    <button id="btnGoApi" class="playground-btn" type="button">Stay in API Docs</button>
  </div>
</div>
<div id="loading">Loading API documentation…</div>
<div id="redoc" data-trait="sys.openapi" data-handler="initRedoc"></div>
<script src="https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js"></script>
<script>
function isPlaygroundRoute() {
  return location.pathname === '/playground' || location.hash === '#/playground';
}

function toggleTerminal(forceOpen) {
  var btn = document.getElementById('btnToggleTerm');
  if (!btn) return;
  var wantsOpen = forceOpen !== false;
  if (wantsOpen && /^▶/.test(btn.textContent || '')) btn.click();
}

function initPlaygroundBanner() {
  var banner = document.getElementById('playgroundBanner');
  if (!banner || !isPlaygroundRoute()) return;
  banner.classList.add('show');
  var openBtn = document.getElementById('btnOpenPlaygroundTerm');
  var apiBtn = document.getElementById('btnGoApi');
  if (openBtn) openBtn.addEventListener('click', function() { toggleTerminal(true); });
  if (apiBtn) apiBtn.addEventListener('click', function() {
    if (location.hash === '#/playground') location.hash = '#/docs/api';
    else location.pathname = '/docs/api';
  });
}

// ── Spec loading via TC (data-trait="sys.openapi" on #redoc) ──
TC.on('initRedoc', function(el, spec) {
  document.getElementById('loading').className = 'hidden';
  if (!spec || !spec.openapi) {
    var isLocal = location.protocol === 'file:';
    el.innerHTML = isLocal
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
    }, el);
});
// Handle trait call failure (network error, no dispatch path, etc.)
document.getElementById('redoc').addEventListener('trait:error', function() {
  document.getElementById('loading').className = 'hidden';
  var isLocal = location.protocol === 'file:';
  this.innerHTML = isLocal
    ? '<div style="padding:2rem;text-align:center;color:var(--fg,#c9d1d9)">' +
      '<h2 style="margin-bottom:1rem;color:#f0f6fc">API Reference</h2>' +
      '<p style="color:#8b949e;max-width:480px;margin:0 auto">The interactive API documentation requires a running server to generate the OpenAPI spec.</p>' +
      '<p style="margin-top:1rem"><code style="background:#161b22;padding:0.3rem 0.6rem;border-radius:4px;color:#8bdb8b">./target/release/traits serve -p 8091</code></p>' +
      '<p style="margin-top:0.5rem;color:#8b949e">Then visit <a href="http://127.0.0.1:8091/docs/api">http://127.0.0.1:8091/docs/api</a></p></div>'
    : '<div style="padding:2rem;color:#f85149">Failed to load OpenAPI spec.</div>';
});
</script>

<div class="terminal-wrap" id="termWrap" style="display:none">
  <div class="terminal-header" id="termHeader">
    <button id="btnToggleTerm" class="terminal-toggle">▼ Terminal</button>
    <span class="terminal-hint">WASM-powered traits CLI — try "list" or "call sys.checksum hash hello"</span>
    <span id="termStatus" class="terminal-status"></span>
  </div>
  <div id="termContainer" class="terminal-container">
    <div id="xterm" class="xterm-mount"></div>
  </div>
</div>
<script>
// Load terminal dynamically — hide wrapper if it fails to load.
(async function() {
  const PENDING_COMMAND_KEY = 'traits.pending.terminal.command';
  var createTerminal = window.createTerminal; // Pre-loaded by SPA shell (terminal-runtime.js)
  if (!createTerminal) {
    // Fallback: import() (works on HTTP server, not file:// mode)
    var paths = ['/static/www/terminal/terminal.js', '../terminal/terminal.js'];
    for (var p of paths) {
      try { var mod = await import(p); createTerminal = mod.createTerminal; break; } catch(e) {}
    }
  }
  if (!createTerminal) return; // Terminal unavailable — wrapper stays hidden
  document.getElementById('termWrap').style.display = '';
  initPlaygroundBanner();
  var terminalInstance = await createTerminal(document.getElementById('xterm'), {
    header: document.getElementById('termHeader'),
    container: document.getElementById('termContainer'),
    toggleBtn: document.getElementById('btnToggleTerm'),
    statusEl: document.getElementById('termStatus'),
  });
  if (isPlaygroundRoute()) {
    setTimeout(function() { toggleTerminal(true); }, 80);
  }
  try {
    var pendingCommand = sessionStorage.getItem(PENDING_COMMAND_KEY);
    if (pendingCommand && terminalInstance && terminalInstance.term && typeof terminalInstance.term.paste === 'function') {
      sessionStorage.removeItem(PENDING_COMMAND_KEY);
      setTimeout(function() {
        terminalInstance.term.focus();
        terminalInstance.term.paste(pendingCommand);
      }, 120);
    }
  } catch(e) {}
  // Handle bfcache restoration — re-check for pending commands
  window.addEventListener('pageshow', function(event) {
    if (!event.persisted) return;
    try {
      var cmd = sessionStorage.getItem(PENDING_COMMAND_KEY);
      if (cmd && terminalInstance && terminalInstance.term && typeof terminalInstance.term.paste === 'function') {
        sessionStorage.removeItem(PENDING_COMMAND_KEY);
        setTimeout(function() {
          terminalInstance.term.focus();
          terminalInstance.term.paste(cmd);
        }, 120);
      }
    } catch(e) {}
  });
})();
</script>
</body>
</html>"##;
