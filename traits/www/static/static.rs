use serde_json::Value;

pub fn static_page(_args: &[Value]) -> Value {
    Value::String(HTML_SHELL.to_string())
}

const HTML_SHELL: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>traits.build</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
:root{--bg:#0a0a0f;--fg:#e8e6e3;--accent:#6c63ff;--accent2:#00d4aa;--muted:#6b7280;--card:#12121a;--border:#1e1e2e}
body{font-family:system-ui,-apple-system,sans-serif;background:var(--bg);color:var(--fg);line-height:1.6}
a{color:var(--accent);text-decoration:none}
a:hover{text-decoration:underline}

/* ── Shell nav ── */
#shell-nav{position:sticky;top:0;z-index:9999;background:rgba(10,10,15,0.95);backdrop-filter:blur(12px);border-bottom:1px solid var(--border);padding:0 2rem;display:flex;align-items:center;height:48px;gap:2rem}
#shell-nav .logo{font-weight:800;font-size:1.05rem;letter-spacing:-0.02em;cursor:pointer}
#shell-nav .logo span{background:linear-gradient(135deg,var(--accent),var(--accent2));-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}
#shell-nav .links{display:flex;gap:1.25rem;font-size:0.88rem}
#shell-nav .links a{color:var(--muted);transition:color 0.15s;cursor:pointer}
#shell-nav .links a:hover,#shell-nav .links a.active{color:var(--fg);text-decoration:none}
#shell-nav .status{margin-left:auto;font-size:0.78rem;color:var(--muted)}
.dot{display:inline-block;width:7px;height:7px;border-radius:50%;margin-right:4px;vertical-align:middle}
.dot.ok{background:var(--accent2)}
.dot.loading{background:var(--muted);animation:pulse 1s infinite}
.dot.off{background:#f44}
@keyframes pulse{0%,100%{opacity:1}50%{opacity:0.3}}

/* ── Page frame ── */
#page-frame{min-height:calc(100vh - 48px)}

/* ── Loading overlay ── */
#boot-overlay{position:fixed;inset:0;background:var(--bg);display:flex;flex-direction:column;align-items:center;justify-content:center;z-index:10000;transition:opacity 0.3s}
#boot-overlay.hidden{opacity:0;pointer-events:none}
#boot-overlay .spinner{width:36px;height:36px;border:3px solid var(--border);border-top-color:var(--accent);border-radius:50%;animation:spin 0.8s linear infinite}
#boot-overlay p{margin-top:1rem;color:var(--muted);font-size:0.9rem}
@keyframes spin{to{transform:rotate(360deg)}}

@media(max-width:640px){
  #shell-nav{padding:0 1rem;gap:0.75rem}
  #shell-nav .links{gap:0.75rem;font-size:0.82rem}
}
</style>
</head>
<body>

<div id="boot-overlay">
  <div class="spinner"></div>
  <p>Loading WASM kernel…</p>
</div>

<nav id="shell-nav" style="display:none">
  <div class="logo" data-href="/"><span>traits</span>.build</div>
  <div class="links">
    <a data-href="/" class="active">Home</a>
    <a data-href="/docs">Docs</a>
    <a data-href="/docs/api">API</a>
    <a data-href="/admin">Admin</a>
  </div>
  <div class="status">
    <span class="dot loading" id="statusDot"></span>
    <span id="statusText">loading…</span>
  </div>
</nav>

<div id="page-frame"></div>

<script type="module">
// ═══════════════════════════════════════════════════════════════
// SPA Bootloader — loads WASM kernel, routes URLs to www.* traits
// Page JS uses window._traitsSDK for all trait calls
// (WASM-first, REST-fallback — transparent to pages)
// ═══════════════════════════════════════════════════════════════

// ── Route table (mirrors serve.trait.toml bindings) ──
const ROUTES = {
  '/':         'www.traits.build',
  '/docs':     'www.docs',
  '/docs/api': 'www.docs.api',
  '/admin':    'www.admin',
};

// ── Detect WASM file location ──
function wasmBase() {
  if (location.protocol === 'file:') return '../../kernel/wasm/pkg/';
  return '/wasm/';
}
const isLocal = location.protocol === 'file:';

function decodeBase64Bytes(base64) {
  const bin = atob(base64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

// ── Minimal SDK (inlined for zero-dependency boot) ──
let wasm = null, wasmReady = false;
const wasmCallableSet = new Set();

class TraitsSDK {
  constructor() { this._origin = location.origin; }

  async call(path, args = []) {
    // Try WASM first
    if (wasmReady && wasmCallableSet.has(path)) {
      const t0 = performance.now();
      try {
        const raw = wasm.call(path, JSON.stringify(args));
        return { ok: true, result: JSON.parse(raw), dispatch: 'wasm',
                 ms: Math.round((performance.now() - t0) * 10) / 10 };
      } catch(e) {
        return { ok: false, error: e.message || String(e), dispatch: 'wasm' };
      }
    }
    // REST fallback (skip if no server — file:// mode)
    if (isLocal) {
      return { ok: false, error: 'Trait not available in WASM: ' + path, dispatch: 'none' };
    }
    const rest = path.replace(/\./g, '/');
    const t0 = performance.now();
    try {
      const res = await fetch(`${this._origin}/traits/${rest}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args }),
      });
      const data = await res.json();
      return {
        ok: res.ok,
        result: res.ok ? data.result : undefined,
        error: res.ok ? undefined : (data.error || `HTTP ${res.status}`),
        dispatch: 'rest',
        ms: Math.round((performance.now() - t0) * 10) / 10,
      };
    } catch(e) {
      return { ok: false, error: e.message || String(e), dispatch: 'rest' };
    }
  }

  isCallable(path) { return wasmReady && wasmCallableSet.has(path); }
  dispatchMode(path) {
    if (wasmReady && wasmCallableSet.has(path)) return 'wasm';
    if (this._origin) return 'rest';
    return 'unknown';
  }

  async list() {
    if (wasmReady) return JSON.parse(wasm.list_traits());
    if (isLocal) return [];
    const r = await fetch(`${this._origin}/traits`);
    return r.json();
  }

  async info(path) {
    if (wasmReady) {
      const raw = wasm.get_trait_info(path);
      return raw ? JSON.parse(raw) : null;
    }
    if (isLocal) return null;
    const rest = path.replace(/\./g, '/');
    const res = await fetch(`${this._origin}/traits/${rest}`);
    return res.ok ? res.json() : null;
  }

  async search(query) {
    if (wasmReady) return JSON.parse(wasm.search_traits(query));
    const all = await this.list();
    const q = query.toLowerCase();
    return all.filter(t => t.path?.toLowerCase().includes(q) || t.description?.toLowerCase().includes(q));
  }

  get callableTraits() { return [...wasmCallableSet]; }
  get status() {
    return { wasm: wasmReady, callable: wasmCallableSet.size };
  }
}

const sdk = new TraitsSDK();
window._traitsSDK = sdk;

// ── DOM refs ──
const $ = s => document.querySelector(s);
const $$ = s => [...document.querySelectorAll(s)];
const overlay = $('#boot-overlay');
const nav = $('#shell-nav');
const frame = $('#page-frame');
const dot = $('#statusDot');
const statusText = $('#statusText');

// ── Page rendering ──
// Inject page HTML from a trait call into the frame.
// Extracts <body> content from the full HTML document returned by the trait.
// Handles <style> and <script> tags properly.

let currentPageCleanup = null;

function injectPage(fullHtml, route) {
  // Clean up previous page
  if (currentPageCleanup) { currentPageCleanup(); currentPageCleanup = null; }
  document.querySelectorAll('style[data-page]').forEach(s => s.remove());
  document.querySelectorAll('link[data-page]').forEach(l => l.remove());

  const parser = new DOMParser();
  const doc = parser.parseFromString(fullHtml, 'text/html');

  // Extract and inject <style> tags from <head>
  doc.querySelectorAll('head style').forEach(style => {
    const s = document.createElement('style');
    s.dataset.page = route;
    s.textContent = style.textContent;
    document.head.appendChild(s);
  });

  // Extract and inject <link> tags from <head>
  doc.querySelectorAll('head link[rel="stylesheet"]').forEach(link => {
    const l = document.createElement('link');
    l.rel = 'stylesheet';
    l.href = link.href;
    l.dataset.page = route;
    document.head.appendChild(l);
  });

  // Set body content
  frame.innerHTML = doc.body.innerHTML;

  // Execute scripts (innerHTML doesn't auto-execute them)
  const scripts = frame.querySelectorAll('script');
  scripts.forEach(old => {
    const s = document.createElement('script');
    for (const attr of old.attributes) {
      // Skip type="module" — handle it separately
      s.setAttribute(attr.name, attr.value);
    }
    s.textContent = old.textContent;
    old.replaceWith(s);
  });

  // Update nav active state
  $$('#shell-nav .links a').forEach(a => {
    a.classList.toggle('active', a.dataset.href === route);
  });

  // Update document title from the page
  const pageTitle = doc.querySelector('title');
  if (pageTitle) document.title = pageTitle.textContent;
}

// ── Router ──
let currentRoute = null;

async function navigate(route, pushState = true) {
  if (route === currentRoute) return;
  currentRoute = route;

  const traitPath = ROUTES[route];
  if (!traitPath) {
    frame.innerHTML = '<div style="padding:4rem;text-align:center;color:var(--muted)"><h2>404</h2><p>Page not found: ' + route + '</p></div>';
    return;
  }

  // Show a brief loading state for slow pages
  frame.style.opacity = '0.6';

  const result = await sdk.call(traitPath, []);

  frame.style.opacity = '1';

  if (result.ok && typeof result.result === 'string') {
    injectPage(result.result, route);
  } else {
    frame.innerHTML = '<div style="padding:4rem;text-align:center;color:#f44"><h2>Error loading page</h2><p>' +
      (result.error || 'Unknown error') + '</p><p style="color:var(--muted);margin-top:0.5rem">Trait: ' + traitPath +
      ' · Dispatch: ' + (result.dispatch || '?') + '</p></div>';
  }

  if (pushState) {
    history.pushState({ route }, '', route);
  }
}

// ── Nav click handling ──
nav.addEventListener('click', e => {
  const el = e.target.closest('[data-href]');
  if (!el) return;
  e.preventDefault();
  navigate(el.dataset.href);
});

// ── Browser back/forward ──
window.addEventListener('popstate', e => {
  const route = e.state?.route || location.pathname;
  navigate(route, false);
});

// ── Boot sequence ──
async function boot() {
  try {
    const base = wasmBase();
    const mod = await import(base + 'traits_wasm.js');
    if (isLocal) {
      const inline = await import('./wasm-inline.js');
      mod.initSync(decodeBase64Bytes(inline.WASM_BASE64));
    } else {
      await mod.default(base + 'traits_wasm_bg.wasm');
    }
    const info = JSON.parse(mod.init());
    JSON.parse(mod.callable_traits()).forEach(p => wasmCallableSet.add(p));
    wasm = mod;
    wasmReady = true;

    dot.className = 'dot ok';
    statusText.textContent = `${info.traits_registered} traits · WASM`;
  } catch(e) {
    console.warn('WASM init failed, using REST only:', e);
    dot.className = 'dot off';
    statusText.textContent = isLocal ? 'WASM load failed' : 'REST mode';
  }

  // Show nav, hide overlay
  nav.style.display = '';
  overlay.classList.add('hidden');
  setTimeout(() => overlay.remove(), 300);

  // Route to initial page
  const path = location.pathname === '' || location.pathname === '/static' ? '/' : location.pathname;
  const route = ROUTES[path] ? path : '/';
  await navigate(route, false);
}

boot();
</script>
</body>
</html>"##;
