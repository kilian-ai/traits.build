use serde_json::Value;

/// Stage-3 proof of concept: call `sys.checksum` via the WebAssembly Component
/// Model from the JS console. The component is built by
/// `traits/sys/checksum-component/` (see its README) and transpiled to ESM by
/// jco; the output lives under `traits/www/static/components/sys-checksum/`.
///
/// The transpiled module imports `@bytecodealliance/preview2-shim` for WASI
/// preview2 host functions; we resolve it via an importmap to esm.sh so the
/// page works without any local node_modules.
pub fn components_demo_page(_args: &[Value]) -> Value {
    let html = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>traits.build — Component Model demo</title>
<style>
  :root { color-scheme: dark; }
  body { font: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace;
    background: #0e1116; color: #e6edf3; max-width: 880px; margin: 2em auto;
    padding: 0 1em; }
  h1 { font-size: 1.4em; margin-bottom: 0.2em; }
  h1 .accent { color: #7ee787; }
  p.lead { color: #8b949e; }
  fieldset { border: 1px solid #30363d; border-radius: 6px; padding: 1em;
    margin: 1.5em 0; }
  legend { color: #7ee787; padding: 0 .5em; }
  label { display: block; margin: .5em 0 .2em; color: #8b949e; }
  input, select, button {
    font: inherit; background: #161b22; color: #e6edf3;
    border: 1px solid #30363d; border-radius: 4px; padding: .4em .6em;
  }
  input { width: 100%; box-sizing: border-box; }
  button { cursor: pointer; background: #238636; border-color: #2ea043;
    color: white; }
  button:hover { background: #2ea043; }
  pre { background: #161b22; border: 1px solid #30363d; border-radius: 6px;
    padding: 1em; overflow: auto; white-space: pre-wrap; word-break: break-all; }
  .ok { color: #7ee787; }
  .err { color: #f85149; }
  code { background: #161b22; padding: 1px 4px; border-radius: 3px; }
  a { color: #58a6ff; }
</style>

<!-- Resolve the bytecodealliance preview2 WASI shim from a CDN so we don't
     need a local node_modules. Pin the version to match the jco we used. -->
<script type="importmap">
{
  "imports": {
    "@bytecodealliance/preview2-shim": "https://esm.sh/@bytecodealliance/preview2-shim@0.17.9",
    "@bytecodealliance/preview2-shim/cli": "https://esm.sh/@bytecodealliance/preview2-shim@0.17.9/cli",
    "@bytecodealliance/preview2-shim/clocks": "https://esm.sh/@bytecodealliance/preview2-shim@0.17.9/clocks",
    "@bytecodealliance/preview2-shim/filesystem": "https://esm.sh/@bytecodealliance/preview2-shim@0.17.9/filesystem",
    "@bytecodealliance/preview2-shim/io": "https://esm.sh/@bytecodealliance/preview2-shim@0.17.9/io",
    "@bytecodealliance/preview2-shim/random": "https://esm.sh/@bytecodealliance/preview2-shim@0.17.9/random",
    "@bytecodealliance/preview2-shim/sockets": "https://esm.sh/@bytecodealliance/preview2-shim@0.17.9/sockets"
  }
}
</script>
</head>
<body>

<h1>traits.build <span class="accent">component model</span></h1>
<p class="lead">
  Calling <code>sys.checksum</code> through a real WebAssembly Component
  Model component, transpiled to JS by <a href="https://github.com/bytecodealliance/jco" target="_blank" rel="noopener">jco</a>.
  Generated WIT contract:
  <a href="/static/sys/checksum/checksum.wit" target="_blank" rel="noopener">checksum.wit</a>.
</p>

<fieldset>
  <legend>checksum.checksum</legend>
  <label>action <small>(string)</small></label>
  <input id="action" value="hash">
  <label>data <small>(string)</small></label>
  <input id="data" value="hello world">
  <p>
    <button id="btnRun">Run via component</button>
    <span id="status"></span>
  </p>
  <pre id="out">module not loaded yet…</pre>
</fieldset>

<fieldset>
  <legend>From the JS console</legend>
  <pre>const m = await import('/static/components/sys-checksum/sys_checksum_component.js');
const ck = m.checksum || m['traits:sys-checksum/checksum@0.1.0'];
console.log(ck.checksum('hash', 'hello world'));</pre>
  <p class="lead">
    Or use the global handle: <code>window.traits.sys.checksum.checksum('hash', 'hello')</code>
  </p>
</fieldset>

<script type="module">
const status = document.getElementById('status');
const out = document.getElementById('out');

try {
  const mod = await import('/static/components/sys-checksum/sys_checksum_component.js');
  const ck = mod.checksum || mod['traits:sys-checksum/checksum'];
  if (!ck || typeof ck.checksum !== 'function') {
    throw new Error('checksum export not found; got keys: ' + Object.keys(mod).join(', '));
  }
  // Expose for console use.
  window.traits = window.traits || {};
  window.traits.sys = window.traits.sys || {};
  window.traits.sys.checksum = ck;
  status.textContent = ' ✔ component loaded';
  status.className = 'ok';
  out.textContent = 'Ready. Click "Run via component" or open the JS console.';

  document.getElementById('btnRun').addEventListener('click', () => {
    const action = document.getElementById('action').value;
    const data = document.getElementById('data').value;
    try {
      const t0 = performance.now();
      const r = ck.checksum(action, data);
      const dt = (performance.now() - t0).toFixed(2);
      out.textContent = `OK (${dt} ms)\n\n${r}`;
      out.className = 'ok';
    } catch (e) {
      out.textContent = 'Error: ' + (e && e.message ? e.message : String(e));
      out.className = 'err';
    }
  });
} catch (e) {
  status.textContent = ' ✘ load failed';
  status.className = 'err';
  out.textContent = 'Failed to load component:\n' + (e && e.stack ? e.stack : e);
  out.className = 'err';
}
</script>
</body>
</html>"##;
    Value::String(html.to_string())
}
