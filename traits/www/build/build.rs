use serde_json::Value;

/// www.build — IDE-style page with tree view, Monaco editor, and embedded terminal.
pub fn build_page(_args: &[Value]) -> Value {
    Value::String(HTML.to_string())
}

const HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Build — traits.build</title>
<meta name="description" content="Browse, edit, and run traits in an IDE-style workspace">
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.min.css">
<style>
  * { box-sizing: border-box; }
  html, body { margin:0; padding:0; height:100%; background:#0d1117; color:#c9d1d9; font-family: system-ui,-apple-system,sans-serif; overflow:hidden; }
  a { color:#58a6ff; text-decoration:none; }
  a:hover { text-decoration:underline; }

  .topnav { display:flex; align-items:center; gap:1.5rem; padding:0.6rem 1.25rem; background:#0d1117; border-bottom:1px solid #222; }
  .topnav .brand { font-size:1.05rem; font-weight:600; color:#e0e0e0; }
  .topnav .brand span { color:#f97316; }
  .topnav .tabs { display:flex; gap:0.25rem; margin-left:1rem; }
  .topnav .tab { padding:0.4rem 0.9rem; border-radius:6px; color:#999; font-size:0.9rem; }
  .topnav .tab:hover { background:#161b22; color:#e0e0e0; text-decoration:none; }
  .topnav .tab.active { background:#161b22; color:#f97316; }
  .topnav .toolbar { margin-left:auto; display:flex; gap:0.5rem; }
  .btn { padding:0.4rem 0.85rem; border-radius:6px; border:1px solid #30363d; background:#161b22; color:#e0e0e0; font:inherit; font-size:0.85rem; cursor:pointer; }
  .btn:hover { border-color:#f97316; color:#f97316; }
  .btn.primary { background:#f97316; border-color:#f97316; color:#0d1117; font-weight:600; }
  .btn.primary:hover { background:#fb923c; color:#0d1117; }

  .ide { display:grid; height:calc(100vh - 49px); grid-template-columns:280px 1fr; grid-template-rows:1fr 320px; grid-template-areas: "tree editor" "tree term"; transition: grid-template-rows 0.15s ease; }
  .ide.term-collapsed { grid-template-rows: 1fr 34px; }
  .pane-tree { grid-area:tree; background:#0a0e14; border-right:1px solid #222; overflow:auto; }
  .pane-editor { grid-area:editor; background:#1e1e1e; min-height:0; }
  #editor { width:100%; height:100%; }
  .pane-term { grid-area:term; background:#0d1117; border-top:1px solid #222; display:flex; flex-direction:column; min-height:0; overflow:hidden; }
  .term-header { display:flex; align-items:center; gap:1rem; padding:0.4rem 0.9rem; background:#161b22; border-bottom:1px solid #222; user-select:none; }
  .term-toggle { background:none; border:none; color:#8b949e; font-size:0.85rem; font-weight:600; cursor:pointer; padding:0; }
  .term-status { font-size:0.7rem; color:#8b949e; margin-left:auto; }
  .term-status.ready { color:#3fb950; } .term-status.loading { color:#d29922; } .term-status.error { color:#f85149; }
  .term-body { flex:1; min-height:0; padding:4px; }
  #xterm { width:100%; height:100%; }

  .tree-search { padding:0.6rem; border-bottom:1px solid #222; }
  .tree-search input { width:100%; padding:0.4rem 0.6rem; border-radius:5px; background:#161b22; border:1px solid #30363d; color:#e0e0e0; font:inherit; font-size:0.85rem; }
  .tree-list { padding:0.4rem 0; font-size:0.85rem; }
  .tree-group { padding:0.35rem 0.85rem; color:#666; font-size:0.7rem; text-transform:uppercase; letter-spacing:0.05em; font-weight:600; }
  .tree-item { padding:0.3rem 0.85rem 0.3rem 1.5rem; color:#b0b0b0; cursor:pointer; }
  .tree-item:hover { background:#161b22; color:#e0e0e0; }
  .tree-item.active { background:#161b22; color:#f97316; border-left:2px solid #f97316; padding-left:calc(1.5rem - 2px); }
  .tree-item .desc { display:block; color:#666; font-size:0.72rem; margin-top:0.1rem; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .editor-empty { display:flex; align-items:center; justify-content:center; height:100%; color:#8b949e; font-size:0.95rem; }
</style>
</head>
<body>
<header class="topnav">
  <a class="brand" href="/traits">traits<span>.build</span></a>
  <nav class="tabs">
    <a class="tab" href="/traits">traits</a>
    <a class="tab active" href="/build">build</a>
    <a class="tab" href="/api">api</a>
  </nav>
  <div class="toolbar">
    <button id="btnRun" class="btn primary" type="button" title="Call the active trait">▶ Run</button>
    <button id="btnBuild" class="btn" type="button" title="Rebuild the binary">⟳ Build</button>
    <button id="btnReload" class="btn" type="button" title="Reload trait list">↻ Reload</button>
  </div>
</header>
<div class="ide">
  <aside class="pane-tree">
    <div class="tree-search"><input id="treeFilter" type="search" placeholder="filter traits…"></div>
    <label class="tree-wasm-only" style="display:flex;align-items:center;gap:.4rem;padding:.25rem .5rem;font-size:.8rem;color:#888;cursor:pointer"><input id="treeWasmOnly" type="checkbox"> WASM-callable only</label>
    <div id="treeList" class="tree-list">Loading…</div>
  </aside>
  <section class="pane-editor"><div id="editor" class="editor-empty">Select a trait from the tree to view its source</div></section>
  <section class="pane-term">
    <div class="term-header" id="termHeader">
      <button id="termToggle" class="term-toggle">▼ Terminal</button>
      <span class="term-status" id="termStatus"></span>
    </div>
    <div class="term-body"><div id="xterm"></div></div>
  </section>
</div>

<script>
// ── In-browser WASM kernel boot (static deploy) ────────────────────
// In live (helper) mode `window._traitsSDK` is already provided by the
// host page; on the GitHub Pages deploy we load /wasm-runtime.js,
// initialize TraitsWasm from its embedded base64 module, and expose a
// minimal SDK shim so `hasKernel()` returns true.
function loadScriptOnce(src) {
  return new Promise((resolve, reject) => {
    if (document.querySelector(`script[data-src="${src}"]`)) return resolve();
    const s = document.createElement('script');
    s.src = src; s.dataset.src = src;
    s.onload = () => resolve();
    s.onerror = () => reject(new Error('load failed: ' + src));
    document.head.appendChild(s);
  });
}
async function bootKernel() {
  if (window._traitsSDK && typeof window._traitsSDK.call === 'function') return;
  // Drop legacy "WASM kernel not loaded" banner cached from earlier broken
  // deploys so a stale scrollback doesn't masquerade as a runtime error.
  try {
    const sb = localStorage.getItem('traits.terminal.scrollback') || '';
    if (sb.includes('WASM kernel not loaded')) {
      localStorage.removeItem('traits.terminal.scrollback');
    }
  } catch (_) {}
  try {
    await loadScriptOnce('/wasm-runtime.js');
    const mod = window.TraitsWasm;
    if (!mod) throw new Error('TraitsWasm runtime missing');
    const b64 = mod.WASM_BASE64;
    if (!b64) throw new Error('WASM_BASE64 missing');
    const bin = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
    mod.initSync({ module: bin });
    JSON.parse(mod.init());
    let _callable = new Set();
    try { _callable = new Set(JSON.parse(mod.callable_traits()).map(t => t.path || t)); } catch (_) {}
    const callWasmRaw = (path, args) => {
      const raw = mod.call(path, JSON.stringify(args || []));
      try { return JSON.parse(raw); } catch (_) { return raw; }
    };
    // SDK-shape call: returns { ok, result?, error? } as terminal.js expects.
    const sdkCall = async (path, args /*, opts */) => {
      try {
        if (!_callable.has(path)) {
          return { ok: false, error: `Trait "${path}" is not available in browser-only mode (requires the local binary or a connected helper).` };
        }
        const result = callWasmRaw(path, args);
        if (result && typeof result === 'object' && result.error && Object.keys(result).length === 1) {
          return { ok: false, error: String(result.error) };
        }
        return { ok: true, result };
      } catch (e) {
        return { ok: false, error: String(e?.message || e) };
      }
    };
    const backgroundCall = async (cmd, payload = {}) => {
      try {
        if (cmd === 'cli_input')   return { ok: true, result: mod.cli_input(payload.data || '') };
        if (cmd === 'cli_welcome') return { ok: true, result: mod.cli_welcome ? mod.cli_welcome() : '' };
        if (cmd === 'cli_format_rest_result' && mod.cli_format_rest_result) {
          return { ok: true, result: mod.cli_format_rest_result(payload.path, payload.args_json, payload.result_json) };
        }
        if (cmd === 'call' && payload.path) return sdkCall(payload.path, payload.args);
        return { ok: false, error: 'unsupported background cmd: ' + cmd };
      } catch (e) {
        return { ok: false, error: String(e?.message || e) };
      }
    };
    window._traitsSDK = {
      call: sdkCall,
      backgroundCall,
      initWorkerPool: async () => {},
      attachWasm: () => {},
      callable: _callable,
      status: { wasm: true, callable: _callable.size },
    };
  } catch (e) {
    console.warn('WASM kernel boot failed:', e);
  }
}

// ── Trait list & tree ──────────────────────────────────────────────
let TRAITS = [];
let ACTIVE = null;
let monacoEditor = null;
let terminalInstance = null;

async function fetchTraits() {
  try {
    const sdk = window._traitsSDK;
    if (sdk && typeof sdk.call === 'function') {
      const res = await sdk.call('sys.list', []);
      const list = (res && typeof res === 'object' && 'ok' in res) ? (res.ok ? res.result : null) : res;
      if (Array.isArray(list)) return list;
    }
    // Static deploy: prerendered JSON shipped alongside the page.
    for (const url of ['./traits.json', '/traits.json', '/api/list']) {
      try {
        const r = await fetch(url);
        if (r.ok) return await r.json();
      } catch (_) {}
    }
    return [];
  } catch (e) {
    console.error('fetchTraits failed', e);
    return [];
  }
}

function hasKernel() {
  const sdk = window._traitsSDK;
  return !!(sdk && typeof sdk.call === 'function');
}

function renderTree(filter = '') {
  const root = document.getElementById('treeList');
  if (!TRAITS.length) { root.textContent = 'No traits found'; return; }
  const f = filter.trim().toLowerCase();
  const wasmOnly = document.getElementById('treeWasmOnly')?.checked;
  const callable = window._traitsSDK?.callable;
  const groups = new Map();
  for (const t of TRAITS) {
    if (f && !(t.path || '').toLowerCase().includes(f)) continue;
    if (wasmOnly && callable && !callable.has(t.path)) continue;
    const ns = (t.path || '').split('.')[0] || '_';
    if (!groups.has(ns)) groups.set(ns, []);
    groups.get(ns).push(t);
  }
  const out = [];
  for (const ns of [...groups.keys()].sort()) {
    out.push(`<div class="tree-group">${ns}</div>`);
    for (const t of groups.get(ns).sort((a,b) => (a.path||'').localeCompare(b.path||''))) {
      const cls = t.path === ACTIVE ? 'tree-item active' : 'tree-item';
      const desc = (t.description || '').replace(/[<>&]/g, c => ({'<':'&lt;','>':'&gt;','&':'&amp;'})[c]);
      out.push(`<div class="${cls}" data-path="${t.path}"><div>${t.path}</div><span class="desc">${desc}</span></div>`);
    }
  }
  root.innerHTML = out.join('') || '<div style="padding:1rem;color:#666">No matches</div>';
  for (const el of root.querySelectorAll('.tree-item')) {
    el.addEventListener('click', () => selectTrait(el.dataset.path));
  }
}

document.getElementById('treeFilter').addEventListener('input', e => renderTree(e.target.value));
document.getElementById('treeWasmOnly').addEventListener('change', () => renderTree(document.getElementById('treeFilter').value));

// ── Source loading ─────────────────────────────────────────────────
async function loadSource(path) {
  // Live mode: ask the kernel for richer registry info.
  try {
    const sdk = window._traitsSDK;
    if (sdk && typeof sdk.call === 'function') {
      const meta = await sdk.call('sys.registry', ['info', path]);
      if (meta && typeof meta === 'object') return JSON.stringify(meta, null, 2);
    }
  } catch (_) {}
  // Static / fallback: render the entry from the prerendered trait list.
  const t = TRAITS.find(x => x && x.path === path);
  if (t) return JSON.stringify(t, null, 2);
  return JSON.stringify({ path, error: 'trait not found in registry' }, null, 2);
}

async function ensureMonaco() {
  if (window.monaco) return window.monaco;
  return new Promise((resolve, reject) => {
    if (!document.querySelector('script[data-monaco-loader]')) {
      const s = document.createElement('script');
      s.src = 'https://cdn.jsdelivr.net/npm/monaco-editor@0.45.0/min/vs/loader.js';
      s.dataset.monacoLoader = '1';
      s.onload = afterLoader;
      s.onerror = () => reject(new Error('failed to load monaco loader'));
      document.head.appendChild(s);
    } else {
      afterLoader();
    }
    function afterLoader() {
      window.require.config({ paths: { vs: 'https://cdn.jsdelivr.net/npm/monaco-editor@0.45.0/min/vs' } });
      window.require(['vs/editor/editor.main'], () => resolve(window.monaco));
    }
  });
}

async function selectTrait(path) {
  ACTIVE = path;
  renderTree(document.getElementById('treeFilter').value);
  const host = document.getElementById('editor');
  let monaco, src;
  try {
    monaco = await ensureMonaco();
    src = await loadSource(path);
  } catch (e) {
    console.error('selectTrait failed', e);
    host.classList.remove('editor-empty');
    host.textContent = '';
    const pre = document.createElement('pre');
    pre.style.cssText = 'padding:1rem;color:#f85149;white-space:pre-wrap;margin:0;font:12px ui-monospace,monospace';
    pre.textContent = `Failed to load editor for ${path}\n\n${e && e.message || e}`;
    host.appendChild(pre);
    return;
  }
  if (!monacoEditor) {
    host.classList.remove('editor-empty');
    host.textContent = '';
    monacoEditor = monaco.editor.create(host, {
      value: src,
      language: 'json',
      theme: 'vs-dark',
      automaticLayout: true,
      readOnly: true,
      minimap: { enabled: false },
      fontSize: 13,
    });
  } else {
    monacoEditor.setValue(src);
  }
}

// ── Terminal ───────────────────────────────────────────────────────
// Always-available collapse toggle (works even without the kernel runtime).
(function wireTerminalToggle(){
  const ide = document.querySelector('.ide');
  const btn = document.getElementById('termToggle');
  if (!ide || !btn) return;
  function apply() {
    const collapsed = ide.classList.contains('term-collapsed');
    btn.textContent = (collapsed ? '▲' : '▼') + ' Terminal';
    if (terminalInstance && terminalInstance.fit && !collapsed) {
      try { terminalInstance.fit(); } catch (_) {}
    }
  }
  btn.addEventListener('click', (e) => {
    e.stopPropagation();
    ide.classList.toggle('term-collapsed');
    apply();
  });
  apply();
})();

async function ensureTerminal() {
  if (terminalInstance) return terminalInstance;
  let createTerminal = window.createTerminal;
  if (!createTerminal) {
    // Try classic <script> first (works for IIFE bundles like /terminal-runtime.js),
    // then ES-module dynamic import for source-tree builds.
    for (const p of ['/terminal-runtime.js', '/static/terminal-runtime.js']) {
      try { await loadScriptOnce(p); if (window.createTerminal) { createTerminal = window.createTerminal; break; } } catch (_) {}
    }
    if (!createTerminal) {
      for (const p of ['/static/www/terminal/terminal.js']) {
        try { const m = await import(p); createTerminal = m.createTerminal || (window.createTerminal); if (createTerminal) break; } catch (_) {}
      }
    }
  }
  if (!createTerminal) {
    document.getElementById('termStatus').textContent = 'terminal unavailable';
    document.getElementById('termStatus').className = 'term-status error';
    return null;
  }
  terminalInstance = await createTerminal(document.getElementById('xterm'), {
    header: document.getElementById('termHeader'),
    container: document.querySelector('.term-body'),
    toggleBtn: document.getElementById('termToggle'),
    statusEl: document.getElementById('termStatus'),
  });
  return terminalInstance;
}

function pasteCommand(cmd) {
  if (!terminalInstance || !terminalInstance.term) return;
  try {
    terminalInstance.term.focus();
    if (typeof terminalInstance.term.paste === 'function') terminalInstance.term.paste(cmd + '\n');
    else terminalInstance.term.write(cmd + '\r');
  } catch (e) { console.error(e); }
}

document.getElementById('btnRun').addEventListener('click', async () => {
  if (!ACTIVE) return;
  if (!hasKernel()) { showStaticNotice(`Run requires the local binary. Try: traits call ${ACTIVE}`); return; }
  await ensureTerminal();
  pasteCommand(`call ${ACTIVE}`);
});
document.getElementById('btnBuild').addEventListener('click', async () => {
  if (!hasKernel()) { showStaticNotice('Build requires the local binary. Run: bash build.sh'); return; }
  await ensureTerminal();
  pasteCommand('reload');
});

function showStaticNotice(msg) {
  const s = document.getElementById('termStatus');
  if (s) { s.textContent = msg; s.className = 'term-status error'; }
}
document.getElementById('btnReload').addEventListener('click', async () => {
  TRAITS = await fetchTraits();
  renderTree(document.getElementById('treeFilter').value);
});

// ── Boot ───────────────────────────────────────────────────────────
(async () => {
  await bootKernel();
  TRAITS = await fetchTraits();
  renderTree();
  if (hasKernel()) {
    setTimeout(() => { ensureTerminal().catch(console.error); }, 250);
  } else {
    document.querySelector('.ide')?.classList.add('term-collapsed');
    document.getElementById('termToggle').textContent = '▲ Terminal';
    showStaticNotice('Read-only mode — install the binary to run traits locally.');
  }
})();
</script>
</body>
</html>"##;
