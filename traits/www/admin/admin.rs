use serde_json::Value;

pub fn admin(_args: &[Value]) -> Value {
    Value::String(ADMIN_HTML.to_string())
}

const ADMIN_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>traits.build — Admin</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; min-height: 100vh; }
  .container { max-width: 900px; margin: 0 auto; padding: 2rem; }
  h1 { font-size: 1.8rem; margin-bottom: 0.5rem; }
  h1 span { color: #888; font-weight: 300; }
  .subtitle { color: #666; margin-bottom: 2rem; }
  .card { background: #151515; border: 1px solid #2a2a2a; border-radius: 8px; padding: 1.5rem; margin-bottom: 1.5rem; }
  .card h2 { font-size: 1.1rem; margin-bottom: 1rem; color: #ccc; }
  .status { display: flex; align-items: center; gap: 0.75rem; margin-bottom: 1rem; }
  .dot { width: 12px; height: 12px; border-radius: 50%; }
  .dot.green { background: #22c55e; box-shadow: 0 0 8px #22c55e44; }
  .dot.red { background: #ef4444; box-shadow: 0 0 8px #ef444444; }
  .dot.yellow { background: #eab308; box-shadow: 0 0 8px #eab30844; }
  .dot.gray { background: #666; }
  .status-text { font-size: 1.1rem; }
  .meta { color: #888; font-size: 0.85rem; margin-top: 0.5rem; }
  .actions { display: flex; gap: 1rem; flex-wrap: wrap; }
  button { padding: 0.6rem 1.2rem; border-radius: 6px; border: 1px solid #333; background: #1a1a1a; color: #e0e0e0; cursor: pointer; font-size: 0.9rem; transition: all 0.15s; }
  button:hover { background: #252525; border-color: #555; }
  button.primary { background: #1d4ed8; border-color: #2563eb; color: white; }
  button.primary:hover { background: #2563eb; }
  button.danger { background: #7f1d1d; border-color: #991b1b; color: #fca5a5; }
  button.danger:hover { background: #991b1b; }
  button:disabled { opacity: 0.5; cursor: not-allowed; }
  .log { background: #0d0d0d; border: 1px solid #222; border-radius: 6px; padding: 1rem; font-family: 'Berkeley Mono', 'SF Mono', monospace; font-size: 0.8rem; color: #888; max-height: 300px; overflow-y: auto; white-space: pre-wrap; margin-top: 1rem; }
  .log .entry { margin-bottom: 0.25rem; }
  .log .time { color: #555; }
  .log .info { color: #6b9; }
  .log .warn { color: #eb5; }
  .log .error { color: #e55; }
  table { width: 100%; border-collapse: collapse; }
  td { padding: 0.4rem 0; border-bottom: 1px solid #1a1a1a; }
  td:first-child { color: #888; width: 140px; }
  .infra { margin-top: 0.75rem; }
  .infra table td:first-child { width: 120px; }
  code { background: #1a1a1a; padding: 0.15rem 0.4rem; border-radius: 3px; font-family: 'Berkeley Mono', 'SF Mono', monospace; font-size: 0.85rem; color: #8b8; }
  .section { margin-top: 1.5rem; }
  .section h3 { font-size: 0.95rem; color: #999; margin-bottom: 0.75rem; border-bottom: 1px solid #222; padding-bottom: 0.4rem; }
  .step { display: flex; gap: 0.75rem; margin-bottom: 0.6rem; font-size: 0.88rem; }
  .step-num { color: #555; font-weight: 600; min-width: 1.5rem; }
  .step-text { color: #bbb; }
  .step-text code { font-size: 0.82rem; }
  .note { color: #888; font-size: 0.82rem; font-style: italic; margin-top: 0.5rem; }
</style>
</head>
<body>
<div class="container">
  <h1>traits.build <span>admin</span></h1>
  <p class="subtitle">Deployment control panel &mdash; Fly.io &middot; iad region</p>

  <div class="card">
    <h2>Server Status</h2>
    <div class="status">
      <div class="dot gray" id="statusDot"></div>
      <span class="status-text" id="statusText">Checking...</span>
    </div>
    <table id="statusTable">
      <tr><td>Traits</td><td id="traitCount">—</td></tr>
      <tr><td>Namespaces</td><td id="nsCount">—</td></tr>
      <tr><td>Uptime</td><td id="uptime">—</td></tr>
      <tr><td>Version</td><td id="version">—</td></tr>
    </table>
  </div>

  <div class="card">
    <h2>Fly.io Machine</h2>
    <div class="status">
      <div class="dot gray" id="flyDot"></div>
      <span class="status-text" id="flyText">Checking...</span>
    </div>
    <div class="infra">
      <table>
        <tr><td>App</td><td><code>polygrait-api</code></td></tr>
        <tr><td>Region</td><td><code>iad</code> (Ashburn, Virginia)</td></tr>
        <tr><td>Machine</td><td id="machineId">—</td></tr>
        <tr><td>State</td><td id="machineState">—</td></tr>
        <tr><td>Image</td><td id="machineImage" style="word-break:break-all;">—</td></tr>
      </table>
    </div>
    <div class="actions" style="margin-top: 1rem;">
      <button class="primary" id="btnDeploy" onclick="deploy()">Restart Machine</button>
      <button id="btnScale0" onclick="scale(0)">Stop (offline)</button>
      <button id="btnScale1" onclick="scale(1)">Start</button>
      <button class="danger" id="btnDestroy" onclick="if(confirm('Destroy all machines? You will need to fly deploy again.'))destroy()">Destroy</button>
    </div>
  </div>

  <div class="card">
    <h2>System Tools</h2>
    <div class="actions">
      <button onclick="listTraits()">List Traits</button>
      <button onclick="runTests()">Run Tests</button>
      <button onclick="reloadRegistry()">Reload Registry</button>
      <button onclick="showVersion()">Version</button>
      <button onclick="showProcesses()">Processes</button>
    </div>
    <div class="log" id="sysLog" style="display:none; margin-top: 1rem;"><span class="entry"><span class="time">[--:--:--]</span> Ready</span></div>
  </div>

  <div class="card">
    <h2>Fast Deploy</h2>
    <p class="note">Builds amd64 binary in Docker with cached deps, uploads via sftp, restarts machine. Only works from local dev server.</p>
    <div class="actions" style="margin-top: 1rem;">
      <button class="primary" id="btnFastDeploy" onclick="fastDeploy('build')">Build + Deploy</button>
      <button id="btnFastUpload" onclick="fastDeploy('upload')">Re-upload Last Binary</button>
    </div>
    <div class="log" id="deployLog" style="display:none; margin-top: 1rem;"><span class="entry"><span class="time">[--:--:--]</span> Ready</span></div>
  </div>

  <div class="card">
    <h2>Deploy Process</h2>
    <p class="note">Full redeployment requires a local build + push (the buttons above only restart/stop existing machines).</p>
    <div class="section">
      <h3>Build &amp; Deploy (from local machine)</h3>
      <div class="step"><span class="step-num">1.</span><span class="step-text">Build amd64 image: <code>docker buildx build --platform linux/amd64 -t registry.fly.io/polygrait-api:deployment-vN .</code></span></div>
      <div class="step"><span class="step-num">2.</span><span class="step-text">Deploy to Fly: <code>fly deploy --now --local-only --image registry.fly.io/polygrait-api:deployment-vN</code></span></div>
      <div class="step"><span class="step-num">3.</span><span class="step-text">Verify: <code>curl https://traits.build/health</code></span></div>
    </div>
    <div class="section">
      <h3>Architecture Notes</h3>
      <div class="step"><span class="step-num">&bull;</span><span class="step-text">Binary is Rust-only. All traits compile into the binary via <code>build.rs</code> (no filesystem needed).</span></div>
      <div class="step"><span class="step-num">&bull;</span><span class="step-text">Traits using <code>source = "dylib"</code> won't work in Docker. Use <code>source = "builtin"</code> instead.</span></div>
      <div class="step"><span class="step-num">&bull;</span><span class="step-text">Dockerfile CMD must be <code>["traits"]</code> (no args). It reads <code>TRAITS_PORT</code> env and dispatches <code>kernel.serve</code>.</span></div>
      <div class="step"><span class="step-num">&bull;</span><span class="step-text">Must build with <code>--platform linux/amd64</code> (Fly runs x86_64, Mac builds arm64 by default).</span></div>
      <div class="step"><span class="step-num">&bull;</span><span class="step-text">Secrets: <code>ADMIN_PASSWORD</code>, <code>FLY_API_TOKEN</code>, <code>CLOUDFLARE_API_TOKEN</code> set via <code>fly secrets set</code>.</span></div>
    </div>
  </div>

  <div class="card">
    <h2>Activity Log</h2>
    <div class="log" id="log"><span class="entry"><span class="time">[--:--:--]</span> Waiting for commands...</span></div>
  </div>
</div>

<script>
const API = window.location.origin + '/traits';

function log(msg, type) {
  const el = document.getElementById('log');
  const t = new Date().toTimeString().slice(0,8);
  const cls = type || 'info';
  el.innerHTML += '\n<span class="entry"><span class="time">[' + t + ']</span> <span class="' + cls + '">' + esc(msg) + '</span></span>';
  el.scrollTop = el.scrollHeight;
}

function esc(s) { const d = document.createElement('div'); d.textContent = s; return d.innerHTML; }

var _statusPaused = false;  // pause health checks after scale-to-0

async function callTrait(path, args) {
  const res = await fetch(API + '/' + path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ args: args || [] })
  });
  return res.json();
}

async function checkStatus() {
  if (_statusPaused) return;  // don't ping /health when machine is stopped
  try {
    const r = await fetch(window.location.origin + '/health');
    const h = await r.json();
    if (h.status === 'healthy' || h.status === 'running') {
      document.getElementById('statusDot').className = 'dot green';
      document.getElementById('statusText').textContent = 'Healthy';
    } else {
      document.getElementById('statusDot').className = 'dot yellow';
      document.getElementById('statusText').textContent = h.status || 'Unknown';
    }
    document.getElementById('traitCount').textContent = h.trait_count || '—';
    document.getElementById('nsCount').textContent = h.namespace_count || '—';
    document.getElementById('uptime').textContent = h.uptime_human || '—';
    document.getElementById('version').textContent = h.version || '—';
  } catch(e) {
    document.getElementById('statusDot').className = 'dot red';
    document.getElementById('statusText').textContent = 'Unreachable';
  }
}

function sysLog(msg, type) {
  const el = document.getElementById('sysLog');
  el.style.display = 'block';
  const t = new Date().toTimeString().slice(0,8);
  const cls = type || 'info';
  el.innerHTML += '\n<span class="entry"><span class="time">[' + t + ']</span> <span class="' + cls + '">' + esc(msg) + '</span></span>';
  el.scrollTop = el.scrollHeight;
}

async function listTraits() {
  sysLog('Listing traits...');
  try {
    const r = await callTrait('sys.list', []);
    const d = r.result || r;
    if (Array.isArray(d)) {
      sysLog(d.length + ' traits:');
      d.forEach(function(t) { sysLog('  ' + (t.path || t.name || JSON.stringify(t))); });
    } else { sysLog(JSON.stringify(d, null, 2)); }
  } catch(e) { sysLog('Error: ' + e.message, 'error'); }
}

async function runTests() {
  sysLog('Running all tests...');
  try {
    const r = await callTrait('sys.test_runner', ['*', false, true]);
    const d = r.result || r;
    if (d && d.summary) {
      const s = d.summary;
      const tp = s.total_passed || 0, tf = s.total_failed || 0;
      const type = tf === 0 ? 'info' : 'error';
      sysLog('Tests: ' + tp + ' passed, ' + tf + ' failed, ' + (tp + tf) + ' total (' + (s.traits || 0) + ' traits, ' + (s.skipped || 0) + ' skipped)', type);
    }
    if (d && d.results) {
      d.results.forEach(function(tr) {
        const icon = tr.ok ? '✓' : '✗';
        const type = tr.ok ? 'info' : 'error';
        var ep = (tr.examples && tr.examples.passed) || 0;
        var ef = (tr.examples && tr.examples.failed) || 0;
        sysLog('  ' + icon + ' ' + (tr.trait || '?') + ' (' + ep + '/' + (ep + ef) + ')', type);
      });
    }
    if (!d || !d.summary) { sysLog(JSON.stringify(d, null, 2)); }
  } catch(e) { sysLog('Error: ' + e.message, 'error'); }
}

async function reloadRegistry() {
  sysLog('Reloading trait registry...');
  try {
    const r = await callTrait('kernel.reload', []);
    const d = r.result || r;
    if (d && d.ok) { sysLog('Reloaded: ' + (d.trait_count || d.traits || '?') + ' traits'); }
    else { sysLog(JSON.stringify(d)); }
  } catch(e) { sysLog('Error: ' + e.message, 'error'); }
  setTimeout(checkStatus, 1000);
}

async function showVersion() {
  sysLog('Checking version...');
  try {
    const r = await callTrait('sys.version', []);
    const d = r.result || r;
    sysLog('Version: ' + (d.version || d.date || JSON.stringify(d)));
  } catch(e) { sysLog('Error: ' + e.message, 'error'); }
}

async function showProcesses() {
  sysLog('Checking processes...');
  try {
    const r = await callTrait('sys.ps', []);
    const d = r.result || r;
    if (d && d.processes && d.processes.length > 0) {
      sysLog(d.processes.length + ' running:');
      d.processes.forEach(function(p) {
        sysLog('  ' + p.trait_path + ' (pid ' + p.pid + ', ' + (p.uptime || '?') + ', ' + (p.memory_mb || '?') + 'MB)');
      });
    } else {
      sysLog('No background processes running');
    }
  } catch(e) { sysLog('Error: ' + e.message, 'error'); }
}

function deployLog(msg, type) {
  const el = document.getElementById('deployLog');
  el.style.display = 'block';
  const t = new Date().toTimeString().slice(0,8);
  const cls = type || 'info';
  el.innerHTML += '\n<span class="entry"><span class="time">[' + t + ']</span> <span class="' + cls + '">' + esc(msg) + '</span></span>';
  el.scrollTop = el.scrollHeight;
}

async function fastDeploy(mode) {
  const isBuild = mode !== 'upload';
  const btn = isBuild ? document.getElementById('btnFastDeploy') : document.getElementById('btnFastUpload');
  const label = isBuild ? 'Build + Deploy' : 'Re-upload';
  btn.disabled = true;
  btn.textContent = isBuild ? 'Building...' : 'Uploading...';
  deployLog(isBuild ? 'Starting build + deploy (this may take a minute)...' : 'Re-uploading last binary...');
  try {
    const r = await callTrait('www.admin.fast_deploy', [mode]);
    const d = r.result || r;
    if (d && d.ok) {
      deployLog('Deploy succeeded (exit ' + d.exit_code + ')');
      if (d.output) d.output.split('\n').forEach(function(l) { if(l.trim()) deployLog('  ' + l); });
    } else {
      deployLog('Deploy failed: ' + (d.error || d.output || JSON.stringify(d)), 'error');
      if (d.output) d.output.split('\n').forEach(function(l) { if(l.trim()) deployLog('  ' + l, 'error'); });
    }
  } catch(e) { deployLog('Error: ' + e.message, 'error'); }
  btn.disabled = false;
  btn.textContent = label;
  setTimeout(checkStatus, 8000);
  setTimeout(checkFlyMachine, 10000);
}

async function checkFlyMachine() {
  if (_statusPaused) return;  // don't hit server when machine is stopped
  try {
    const r = await callTrait('www.admin.deploy', ['status']);
    const d = r.result || r;
    if (d && d.machines) {
      const m = d.machines[0];
      if (m) {
        document.getElementById('machineId').textContent = m.id || '—';
        document.getElementById('machineState').textContent = m.state || '—';
        document.getElementById('machineImage').textContent = m.image || '—';
        if (m.state === 'started') {
          document.getElementById('flyDot').className = 'dot green';
          document.getElementById('flyText').textContent = 'Running';
        } else if (m.state === 'stopped' || m.state === 'created') {
          document.getElementById('flyDot').className = 'dot yellow';
          document.getElementById('flyText').textContent = 'Stopped';
        } else {
          document.getElementById('flyDot').className = 'dot yellow';
          document.getElementById('flyText').textContent = m.state || 'Unknown';
        }
      } else {
        document.getElementById('flyDot').className = 'dot red';
        document.getElementById('flyText').textContent = 'No machines';
      }
    }
  } catch(e) {
    document.getElementById('flyDot').className = 'dot gray';
    document.getElementById('flyText').textContent = 'Could not query Fly API';
  }
}

async function deploy() {
  log('Restarting machine (stop + start)...');
  document.getElementById('btnDeploy').disabled = true;
  try {
    const r = await callTrait('www.admin.deploy', []);
    if (r.error) { log('Deploy failed: ' + r.error, 'error'); }
    else {
      const d = r.result;
      if (d && d.ok) { log('Restarted ' + d.machines + ' machine(s): ' + (d.results||[]).join(', ')); }
      else { log('Deploy result: ' + JSON.stringify(d)); }
    }
  } catch(e) { log('Deploy error: ' + e.message, 'error'); }
  document.getElementById('btnDeploy').disabled = false;
  setTimeout(checkStatus, 5000);
  setTimeout(checkFlyMachine, 8000);
}

async function scale(n) {
  log('Scaling to ' + n + ' machine(s)...');
  try {
    const r = await callTrait('www.admin.scale', [n]);
    if (r.error) { log('Scale failed: ' + r.error, 'error'); }
    else {
      const d = r.result;
      if (d && d.ok) { log(d.action + ': ' + (d.results||[]).join(', ')); }
      else { log('Scale result: ' + JSON.stringify(d)); }
    }
  } catch(e) { log('Scale error: ' + e.message, 'error'); }
  // After scale-to-0, don't hit /health (would trigger Fly auto_start)
  if (n > 0) { _statusPaused = false; setTimeout(checkStatus, 3000); }
  else {
    // Manually set UI to offline state
    document.getElementById('statusDot').className = 'dot red';
    document.getElementById('statusText').textContent = 'Stopped';
    document.getElementById('uptime').textContent = '—';
    _statusPaused = true;  // pause ALL periodic checks (health + fly machine)
    // Manually set fly machine UI to stopped
    document.getElementById('flyDot').className = 'dot yellow';
    document.getElementById('flyText').textContent = 'Stopped';
    document.getElementById('machineState').textContent = 'stopped';
  }
}

async function destroy() {
  log('Destroying all machines...', 'error');
  try {
    const r = await callTrait('www.admin.destroy', []);
    if (r.error) { log('Destroy failed: ' + r.error, 'error'); }
    else {
      const d = r.result;
      if (d && d.ok) { log('Destroyed ' + d.machines_destroyed + ' machine(s): ' + (d.results||[]).join(', '), 'error'); }
      else { log('Destroy result: ' + JSON.stringify(d), 'error'); }
    }
  } catch(e) { log('Destroy error: ' + e.message, 'error'); }
  setTimeout(checkFlyMachine, 3000);
}

checkStatus();
checkFlyMachine();
setInterval(checkStatus, 30000);
setInterval(checkFlyMachine, 60000);
</script>
</body>
</html>"##;
