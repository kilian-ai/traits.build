#!/usr/bin/env python3
"""One-shot refactor of v86 standalone HTML:
  - remove old #social toolbar button + #social-overlay DOM/CSS
  - remove the JS social-overlay block (lines 2608-4248)
  - rename #follow toolbar button visible label to "social"
  - add stub sLog (since original sLog lived inside removed block)
  - add new #tunnels toolbar button + side panel + handlers
Mirrors the result to static/v86/standalone-v86.html.
"""
import sys, pathlib, re

SRC = pathlib.Path("traits/www/static/v86/standalone-v86.html")
MIRROR = pathlib.Path("static/v86/standalone-v86.html")

text = SRC.read_text()
lines = text.split("\n")

# Sanity: line numbers below are 1-indexed and must match the file we read.
def expect(idx1, needle):
    if needle not in lines[idx1 - 1]:
        sys.exit(f"line {idx1}: expected {needle!r} in {lines[idx1-1]!r}")

expect(49,  "social overlay")
expect(72,  "#social-search-results")
expect(73,  "follow panel")
expect(121, 'id="social"')
expect(122, 'id="follow"')
expect(183, 'id="social-overlay"')
expect(222, '</div>')   # closing of social-overlay
expect(2609, "Social — Nostr public-folder")
expect(4247, "setInterval(() => { socialBtn.disabled")
expect(4249, "follow panel")

# Build new line list. We'll remove ranges (1-indexed inclusive) and modify singletons.
out = []
i = 1
N = len(lines)
while i <= N:
    if i == 49:
        # Skip CSS lines 49..72 (social overlay CSS).
        i = 73
        continue
    if i == 121:
        # Replace toolbar button rows 121..122 (#social + #follow) with:
        #   - #follow stays, label "social"
        #   - new #tunnels button
        out.append('    <button id="follow" disabled title="social — bootstraps social CLI, shows your npub, follow/sync/publish via guest CLI">social</button>')
        out.append('    <button id="tunnels" disabled title="manage running tunnels (ports, codes, lifecycle, edit CLI)">tunnels</button>')
        i = 123
        continue
    if i == 183:
        # Skip social-overlay DOM block 183..222 inclusive.
        i = 223
        continue
    if i == 230:
        # Just after `const $ = id => ...` (line 230) we already passed; insert sLog stub here.
        out.append(lines[i - 1])
        i += 1
        continue
    if i == 232:
        # Inject sLog stub right after the very first const-block line (line 231 written
        # already). Use a one-line console.log fallback so any pre-existing call sites
        # keep working after the social-overlay block is removed.
        # We insert BEFORE writing line 232 itself.
        out.append("  // sLog: console-only stub (original wrote into #social-log which is gone)")
        out.append("  function sLog(msg, cls) { try { console.log('[social]', msg, cls || ''); } catch (_) {} }")
        out.append(lines[i - 1])
        i += 1
        continue
    if i == 2608:
        # Skip JS social-overlay block 2608..4247 inclusive (next line 4248 is blank,
        # 4249 is the follow-panel comment we keep).
        i = 4248
        continue
    out.append(lines[i - 1])
    i += 1

# Now insert the tunnels CSS, panel DOM, and JS handlers at known anchors.
new_text = "\n".join(out)

# 1) Insert tunnels CSS right after the follow-panel CSS block (after the
#    "#follow-body button:disabled" line).
follow_css_anchor = "  #follow-body button:disabled { opacity:0.4; cursor:not-allowed; }\n"
tunnels_css = """  /* ── tunnels panel ── */
  #tunnels-panel { position:absolute; right:0; top:36px; bottom:0; width:520px; max-width:60vw; display:flex; flex-direction:column; background:#0b0d10; border-left:1px solid #2a2f36; z-index:50; box-shadow:-4px 0 12px rgba(0,0,0,0.5); }
  #tunnels-panel[hidden] { display:none; }
  #tunnels-hdr { display:flex; gap:8px; align-items:center; padding:6px 10px; background:#161a1f; border-bottom:1px solid #2a2f36; font-size:12px; }
  #tunnels-hdr button { background:#222830; color:#ddd; border:1px solid #3a424d; border-radius:4px; padding:3px 10px; font:inherit; cursor:pointer; }
  #tunnels-hdr button:hover:not(:disabled) { background:#2d343d; }
  #tunnels-hdr button:disabled { opacity:0.4; cursor:not-allowed; }
  #tunnels-body { flex:1; min-height:0; overflow:auto; padding:12px 14px; display:flex; flex-direction:column; gap:10px; font-size:12px; }
  #tunnels-list { display:flex; flex-direction:column; gap:8px; }
  #tunnels-list .tn-row { background:#161a1f; border:1px solid #2a2f36; border-radius:6px; padding:10px 12px; display:flex; flex-direction:column; gap:4px; }
  #tunnels-list .tn-row.tn-empty { color:#7a8392; }
  #tunnels-list .tn-code { color:#7ab8f5; font-weight:600; letter-spacing:0.04em; }
  #tunnels-list .tn-meta { color:#aab2bc; word-break:break-all; }
  #tunnels-list .tn-relay-ok { color:#4ade80; }
  #tunnels-list .tn-relay-warn { color:#facc15; }
  #tunnels-list .tn-relay-err { color:#f87171; }
  #tunnels-list .tn-actions { display:flex; gap:6px; flex-wrap:wrap; margin-top:4px; }
  #tunnels-list .tn-actions button { background:#222830; color:#ddd; border:1px solid #3a424d; border-radius:4px; padding:2px 10px; font:inherit; cursor:pointer; }
  #tunnels-list .tn-actions button:hover:not(:disabled) { background:#2d343d; }
  #tunnels-list .tn-actions button:disabled { opacity:0.4; cursor:not-allowed; }
  #tunnels-list .tn-cmd { font-family:'Menlo','Monaco','Consolas',monospace; font-size:11px; color:#7a8392; word-break:break-all; }
  #tunnels-log { font-size:11px; line-height:1.45; max-height:240px; overflow:auto; background:#0b0d10; border:1px solid #2a2f36; border-radius:4px; padding:8px; font-family:'Menlo','Monaco','Consolas',monospace; }
  #tunnels-log .tl-cmd { color:#7ab8f5; white-space:pre-wrap; word-break:break-all; }
  #tunnels-log .tl-out { color:#aab2bc; white-space:pre-wrap; word-break:break-all; padding-left:8px; }
  #tunnels-log .tl-ok { color:#4ade80; }
  #tunnels-log .tl-err { color:#f87171; }
  #tunnels-log .tl-info { color:#7a8392; }
"""
if follow_css_anchor not in new_text:
    sys.exit("could not find follow-panel CSS anchor")
new_text = new_text.replace(follow_css_anchor, follow_css_anchor + tunnels_css, 1)

# 2) Insert tunnels DOM panel right after the closing </div> of #follow-panel.
#    Anchor on the unique `<div id="follow-panel" hidden>` block end: search for
#    the div that follows the follow-action-search line.
follow_dom_anchor = '''      <div class="fp-section">
        <div class="fp-label">command log</div>
        <div id="follow-log"></div>
      </div>
    </div>
  </div>
'''
tunnels_dom = '''  <div id="tunnels-panel" hidden>
    <div id="tunnels-hdr">
      <b>tunnels</b>
      <span class="hint" style="margin-left:6px;font-size:11px">running tunnel processes — guest /tmp/tunnel.* + relay status</span>
      <button id="tunnels-refresh" title="re-scan guest + relay">refresh</button>
      <button id="tunnels-close" style="margin-left:auto">close</button>
    </div>
    <div id="tunnels-body">
      <div id="tunnels-list"></div>
      <div id="tunnels-log"></div>
    </div>
  </div>
'''
if follow_dom_anchor not in new_text:
    sys.exit("could not find follow-panel DOM anchor")
new_text = new_text.replace(follow_dom_anchor, follow_dom_anchor + tunnels_dom, 1)

# 3) Insert tunnels JS handlers right before the autoboot block at end of file.
js_anchor = "  // ?autoboot=1 [&restore=0|1]\n"
tunnels_js = """  // ─────────────────────────────────────────────────────────────────────────
  // Tunnels panel — manages lifecycle of running tunnels (ssh, social, etc.)
  // Reads guest state from /tmp/tunnel.code, /tmp/tunnel.pids, /tmp/tunnel.cmd
  // Pretty-formats one row per tunnel with relay-side status, supports
  // start / stop / restart / edit-cmd. Edited commands are persisted to
  // localStorage (per pairing-code) and used by future restart actions.
  // ─────────────────────────────────────────────────────────────────────────
  const tunnelsBtn         = $('tunnels');
  const tunnelsPanel       = $('tunnels-panel');
  const tunnelsCloseBtn    = $('tunnels-close');
  const tunnelsRefreshBtn  = $('tunnels-refresh');
  const tunnelsListEl      = $('tunnels-list');
  const tunnelsLogEl       = $('tunnels-log');

  const TUNNEL_CMD_OVERRIDE_PREFIX = 'traits.tunnels.cmd.';
  const TUNNEL_DEFAULT_CMD = 'sh <(curl -sS https://www.traits.build/local/tunnel-up.sh) 22 8080 22000 8384';
  const TUNNEL_RELAY_ENDPOINTS = [
    'https://traits-build-tunnel.fly.dev',
    'https://tunnel.traits.build',
  ];

  function tnLog(line, cls) {
    const d = document.createElement('div');
    d.className = 'tl-' + (cls || 'info');
    d.textContent = line;
    tunnelsLogEl.appendChild(d);
    tunnelsLogEl.scrollTop = tunnelsLogEl.scrollHeight;
  }

  async function tnExec(cmd, opts) {
    if (!serialVfs || !serialVfs.ready) {
      if (serialVfs && typeof serialVfs.setReady === 'function') {
        try { serialVfs.setReady(true); } catch (_) {}
      }
    }
    if (!serialVfs) return { stdout: '', exitCode: -1, timedOut: true };
    return serialVfs.exec(cmd, opts || { timeout: 20000 });
  }

  function tnGetCmdOverride(code) {
    if (!code) return null;
    try { return localStorage.getItem(TUNNEL_CMD_OVERRIDE_PREFIX + code); } catch (_) { return null; }
  }
  function tnSetCmdOverride(code, cmd) {
    if (!code) return;
    try {
      if (cmd && cmd.trim()) localStorage.setItem(TUNNEL_CMD_OVERRIDE_PREFIX + code, cmd);
      else localStorage.removeItem(TUNNEL_CMD_OVERRIDE_PREFIX + code);
    } catch (_) {}
  }
  function tnEffectiveCmd(code) {
    return tnGetCmdOverride(code) || TUNNEL_DEFAULT_CMD;
  }

  async function tnFetchRelayStatus(code) {
    if (!code) return null;
    for (const base of TUNNEL_RELAY_ENDPOINTS) {
      try {
        const ctrl = new AbortController();
        const t = setTimeout(() => ctrl.abort(), 4000);
        const r = await fetch(base + '/port/status?code=' + encodeURIComponent(code), { signal: ctrl.signal });
        clearTimeout(t);
        if (!r.ok) continue;
        const j = await r.json();
        return { endpoint: base, ...j };
      } catch (_) {}
    }
    return null;
  }

  async function tnDiscoverTunnels() {
    // Single guest call to gather everything we need.
    // Outputs distinct sections marked with sentinels for easy parsing.
    const cmd =
      'echo __TUNNEL_CODE__; cat /tmp/tunnel.code 2>/dev/null; ' +
      'echo __TUNNEL_PORTS__; cat /tmp/tunnel.ports 2>/dev/null; ' +
      'echo __TUNNEL_PIDS__; cat /tmp/tunnel.pids 2>/dev/null; ' +
      'echo __TUNNEL_CMD__; cat /tmp/tunnel.cmd 2>/dev/null; ' +
      'echo __TUNNEL_PS__; ps -o pid,args 2>/dev/null | grep -E "websocat|tunnel-up" | grep -v grep || true; ' +
      'echo __TUNNEL_END__';
    const r = await tnExec(cmd, { timeout: 15000 });
    const out = (r.stdout || '').replace(/\\r/g, '');
    function section(name) {
      const re = new RegExp('__' + name + '__\\\\n([\\\\s\\\\S]*?)(?=__TUNNEL_[A-Z]+__|$)');
      const m = out.match(re);
      return m ? m[1].trim() : '';
    }
    const code  = section('TUNNEL_CODE');
    const ports = section('TUNNEL_PORTS').split(/\\s+/).filter(Boolean);
    const pids  = section('TUNNEL_PIDS').split(/\\s+/).filter(Boolean);
    const cmdFile = section('TUNNEL_CMD');
    const ps    = section('TUNNEL_PS').split(/\\n/).map(s => s.trim()).filter(Boolean);

    const tunnels = [];
    if (code) {
      tunnels.push({
        id: code,
        code,
        ports,
        pids,
        cmd: cmdFile || null,
        ps,
      });
    } else if (ps.length) {
      // No code file but websocat processes running.
      tunnels.push({ id: 'unmanaged', code: null, ports: [], pids: [], cmd: null, ps });
    }
    return tunnels;
  }

  function tnRenderRow(t) {
    const row = document.createElement('div');
    row.className = 'tn-row';
    row.dataset.code = t.code || '';

    const head = document.createElement('div');
    head.innerHTML = '<span class="tn-code">' + (t.code ? 'code: ' + t.code : '(no pairing code)') + '</span>';
    row.appendChild(head);

    if (t.ports && t.ports.length) {
      const meta = document.createElement('div');
      meta.className = 'tn-meta';
      meta.textContent = 'ports: ' + t.ports.join(', ');
      row.appendChild(meta);
    }
    if (t.pids && t.pids.length) {
      const meta = document.createElement('div');
      meta.className = 'tn-meta';
      meta.textContent = 'pids: ' + t.pids.join(' ') + ' (' + t.pids.length + ' bridge proc' + (t.pids.length === 1 ? '' : 's') + ')';
      row.appendChild(meta);
    }
    if (t.ps && t.ps.length) {
      const meta = document.createElement('div');
      meta.className = 'tn-meta';
      meta.textContent = 'ps: ' + t.ps.length + ' matching process' + (t.ps.length === 1 ? '' : 'es');
      meta.title = t.ps.join('\\n');
      row.appendChild(meta);
    }

    const relay = document.createElement('div');
    relay.className = 'tn-meta';
    relay.textContent = 'relay: querying…';
    row.appendChild(relay);

    if (t.code) {
      tnFetchRelayStatus(t.code).then(s => {
        if (!s) {
          relay.className = 'tn-meta tn-relay-err';
          relay.textContent = 'relay: unreachable';
          return;
        }
        const active = !!s.active;
        const guestPorts = (s.guest_ports && Object.keys(s.guest_ports)) || [];
        const queueDepth = s.guest_queue_depth || {};
        const pairs = s.active_pairs || {};
        const age = (typeof s.age_s === 'number') ? Math.round(s.age_s) + 's' : '';
        const parts = [];
        parts.push(active ? '✓ active' : '✗ inactive');
        if (guestPorts.length) parts.push('guest=' + guestPorts.join(','));
        if (Object.keys(pairs).length) parts.push('pairs=' + JSON.stringify(pairs));
        if (Object.keys(queueDepth).length) parts.push('queue=' + JSON.stringify(queueDepth));
        if (age) parts.push('age=' + age);
        relay.className = 'tn-meta ' + (active && guestPorts.length ? 'tn-relay-ok' : (active ? 'tn-relay-warn' : 'tn-relay-err'));
        relay.textContent = 'relay: ' + parts.join('  ');
        relay.title = JSON.stringify(s, null, 2);
      });
    } else {
      relay.className = 'tn-meta tn-relay-warn';
      relay.textContent = 'relay: (no code)';
    }

    const cmdShown = t.code ? tnEffectiveCmd(t.code) : (t.cmd || TUNNEL_DEFAULT_CMD);
    const cmdEl = document.createElement('div');
    cmdEl.className = 'tn-cmd';
    cmdEl.textContent = '$ ' + cmdShown;
    if (t.code && tnGetCmdOverride(t.code)) cmdEl.title = 'overridden via localStorage';
    row.appendChild(cmdEl);

    const actions = document.createElement('div');
    actions.className = 'tn-actions';
    const btnRestart = document.createElement('button');
    btnRestart.textContent = 'restart';
    btnRestart.onclick = () => tnRestartTunnel(t);
    const btnStop = document.createElement('button');
    btnStop.textContent = 'stop';
    btnStop.onclick = () => tnStopTunnel(t);
    const btnEdit = document.createElement('button');
    btnEdit.textContent = 'edit cmd';
    btnEdit.onclick = () => tnEditCmd(t);
    actions.appendChild(btnRestart);
    actions.appendChild(btnStop);
    actions.appendChild(btnEdit);
    row.appendChild(actions);

    return row;
  }

  async function tnRefresh() {
    if (!emulator) {
      tunnelsListEl.innerHTML = '<div class="tn-row tn-empty">boot v86 first</div>';
      return;
    }
    tunnelsListEl.innerHTML = '<div class="tn-row tn-empty">scanning guest…</div>';
    let tunnels = [];
    try {
      tunnels = await tnDiscoverTunnels();
    } catch (e) {
      tunnelsListEl.innerHTML = '';
      tnLog('discover failed: ' + (e && e.message || e), 'err');
      return;
    }
    tunnelsListEl.innerHTML = '';
    if (!tunnels.length) {
      const empty = document.createElement('div');
      empty.className = 'tn-row tn-empty';
      empty.innerHTML =
        'no tunnels detected. start one with the default command below or click <b>restart</b> after editing.<br>' +
        '<div class="tn-cmd" style="margin-top:6px">$ ' + TUNNEL_DEFAULT_CMD + '</div>';
      const actions = document.createElement('div');
      actions.className = 'tn-actions';
      const startBtn = document.createElement('button');
      startBtn.textContent = 'start default';
      startBtn.onclick = () => tnRestartTunnel({ id: 'default', code: null });
      const editBtn = document.createElement('button');
      editBtn.textContent = 'edit cmd';
      editBtn.onclick = () => tnEditCmd({ id: 'default', code: null });
      actions.appendChild(startBtn);
      actions.appendChild(editBtn);
      empty.appendChild(actions);
      tunnelsListEl.appendChild(empty);
      return;
    }
    tunnels.forEach(t => tunnelsListEl.appendChild(tnRenderRow(t)));
  }

  async function tnStopTunnel(t) {
    tnLog('$ stop ' + (t.code || '(unmanaged)'), 'cmd');
    const cmd =
      'if [ -s /tmp/tunnel.pids ]; then ' +
        'while read p; do kill "$p" 2>/dev/null; done < /tmp/tunnel.pids; ' +
        'sleep 0.3; ' +
        'while read p; do kill -9 "$p" 2>/dev/null; done < /tmp/tunnel.pids; ' +
      'fi; ' +
      'pkill -f websocat 2>/dev/null; ' +
      'rm -f /tmp/tunnel.code /tmp/tunnel.pids /tmp/tunnel.cmd /tmp/tunnel.ports 2>/dev/null; ' +
      'echo stopped';
    const r = await tnExec(cmd, { timeout: 10000 });
    tnLog((r.stdout || '').trim() || '(no output)', r.exitCode === 0 ? 'ok' : 'err');
    setTimeout(tnRefresh, 400);
  }

  async function tnRestartTunnel(t) {
    const code = t.code || (t.id !== 'default' ? t.id : null);
    const cmd = code ? tnEffectiveCmd(code) : TUNNEL_DEFAULT_CMD;
    tnLog('$ restart ' + (code || '(default)'), 'cmd');
    tnLog(cmd, 'out');
    // Stop first so respawn loops are gone, then start fresh.
    await tnStopTunnel(t).catch(() => {});
    // Run the effective command in the background so the panel does not hang
    // on websocat's foreground respawn loop. We background it via `&` and
    // detach stdout/stderr to a log so subsequent guest commands still work.
    const wrapped = '( ' + cmd + ' ) >/tmp/tunnel.last.log 2>&1 &';
    const r = await tnExec(wrapped, { timeout: 8000 });
    tnLog('launched (exit=' + r.exitCode + ')', r.exitCode === 0 ? 'ok' : 'err');
    // Give tunnel-up.sh a moment to write /tmp/tunnel.code, then refresh.
    setTimeout(tnRefresh, 4000);
  }

  function tnEditCmd(t) {
    const code = t.code || (t.id && t.id !== 'default' ? t.id : null);
    const current = code ? tnEffectiveCmd(code) : TUNNEL_DEFAULT_CMD;
    const next = window.prompt(
      'Edit start command for ' + (code ? 'code ' + code : 'default tunnel') +
      '.\\n\\nLeave empty to clear override and revert to default.',
      current
    );
    if (next === null) return; // cancelled
    if (!code) {
      // No code yet — store as default override under sentinel key.
      tnSetCmdOverride('__default__', next);
      tnLog('default cmd updated', 'ok');
    } else {
      tnSetCmdOverride(code, next);
      tnLog('override saved for ' + code + (next.trim() ? '' : ' (cleared)'), 'ok');
    }
    tnRefresh();
  }

  tunnelsBtn.onclick = () => {
    if (!emulator) { setStatus('boot v86 first', 'err'); return; }
    tunnelsPanel.hidden = false;
    tnRefresh();
  };
  tunnelsCloseBtn.onclick = () => { tunnelsPanel.hidden = true; term.focus(); };
  tunnelsRefreshBtn.onclick = () => tnRefresh();

  setInterval(() => { tunnelsBtn.disabled = !emulator; }, 800);

"""
if js_anchor not in new_text:
    sys.exit("could not find autoboot anchor")
new_text = new_text.replace(js_anchor, tunnels_js + js_anchor, 1)

SRC.write_text(new_text)
MIRROR.write_text(new_text)
print(f"wrote {SRC} and {MIRROR}: {len(new_text.splitlines())} lines")
