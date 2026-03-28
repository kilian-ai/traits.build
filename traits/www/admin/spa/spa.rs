use maud::{html, DOCTYPE, PreEscaped};
use serde_json::Value;

pub fn spa(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "traits.build - Settings" }
                style { (PreEscaped(CSS)) }
            }
            body {
                div.page {
                    section.hero.card {
                        p.eyebrow { "settings" }
                        h1 { "traits.build" }
                        p.subtitle {
                            "Setup, run commands, and configure your environment."
                        }
                        div.badges {
                            span.badge id="platformBadge" { "detecting..." }
                            span.badge { "browser-local" }
                            span.badge { "localStorage" }
                        }
                    }

                    // Setup
                    section.card id="setupCard" {
                        h2 { "Setup" }
                        div id="platformInfo" {}
                    }

                    // Run Command
                    section.card {
                        h2 { "Run" }
                        div.form-row style="align-items:center;" {
                            span style="color:var(--muted);font-family:'Iosevka Etoile','IBM Plex Mono',monospace;font-size:0.95rem;white-space:nowrap;" { "traits" }
                            input id="cmdInput" type="text" placeholder="list" autocomplete="off" spellcheck="false" style="flex:1;" {}
                            button.primary id="btnRun" onclick="runCmd()" { "Run" }
                        }
                        div.chip-row id="cmdChips" {
                            span.chip onclick="setCmd('list')" { "list" }
                            span.chip onclick="setCmd('info sys.checksum')" { "info" }
                            span.chip onclick="setCmd('checksum hash hello')" { "checksum" }
                            span.chip onclick="setCmd('test_runner sys.*')" { "test" }
                            span.chip onclick="setCmd('version')" { "version" }
                        }
                        div.log id="cmdLog" style="display:none;margin-top:14px;" {}
                    }

                    section.card id="dispatchCard" data-trait="sys.list" data-handler="refreshStats" data-interval="30000" {
                        h2 { "Dispatch" }
                        p.note {
                            "Calls cascade through four tiers. The first available tier wins."
                        }

                        // Tier 1 — WASM
                        div.tier {
                            div.tier-header {
                                div.dot.gray id="dotWasm" {}
                                span.tier-label { "WASM" }
                                span.tier-detail.muted id="wasmDetail" { "loading..." }
                            }
                        }

                        // Tier 2 — Local Helper
                        div.tier {
                            div.tier-header {
                                div.dot.gray id="dotHelper" {}
                                span.tier-label { "Local Helper" }
                                span.tier-detail.muted id="helperDetail" { "probing..." }
                            }
                            div.tier-controls id="helperControls" style="display:none;" {
                                div.form-row.compact {
                                    input id="helperUrl" type="text" placeholder="http://localhost:8090" style="flex:1;min-width:160px;" {}
                                    button.primary.sm id="btnHelperConnect" onclick="connectHelper()" { "Connect" }
                                    button.sm id="btnHelperDisconnect" onclick="disconnectHelper()" style="display:none;" { "Disconnect" }
                                }
                            }
                        }

                        // Tier 3 — Relay
                        div.tier {
                            div.tier-header {
                                div.dot.gray id="dotRelay" {}
                                span.tier-label { "Relay" }
                                span.tier-detail.muted id="relayDetail" { "not configured" }
                            }
                            div.tier-controls id="relayControls" {
                                p.note.compact-note {
                                    "Run " code { "RELAY_URL=https://relay.traits.build traits serve" } " on your Mac, enter the pairing code."
                                }
                                div.form-row.compact {
                                    input id="relayCode" type="text" placeholder="Code (e.g. A7X9)"
                                          maxlength="4" style="width:110px;text-transform:uppercase;font-family:'Iosevka Etoile','IBM Plex Mono',monospace;font-size:1rem;letter-spacing:0.12em;text-align:center;" {}
                                    input id="relayServer" type="text" placeholder="Relay server (optional)" style="flex:1;min-width:140px;" {}
                                    button.primary.sm id="btnRelayConnect" onclick="connectRelay()" { "Connect" }
                                    button.sm id="btnRelayDisconnect" onclick="disconnectRelay()" style="display:none;" { "Disconnect" }
                                }
                            }
                        }

                            // Tier 4 — Background backend (binding: kernel/background)
                            div.tier {
                              div.tier-header {
                                div.dot.gray id="dotBackground" {}
                                span.tier-label { "Background" }
                                span.tier-detail.muted id="backgroundDetail" { "loading..." }
                              }
                              div.tier-controls id="backgroundControls" {
                                p.note.compact-note {
                                  "Background tasks use the " code { "kernel/background" } " binding."
                                }
                                div.form-row.compact {
                                  select id="backgroundImpl" style="flex:1;min-width:220px;" {
                                    option value="sdk.background.worker" { "sdk.background.worker (Web Worker pool)" }
                                    option value="sdk.background.tokio" { "sdk.background.tokio (native helper)" }
                                    option value="sdk.background.direct" { "sdk.background.direct (main thread fallback)" }
                                  }
                                  button.primary.sm id="btnBackgroundApply" onclick="applyBackgroundBinding()" { "Apply" }
                                }
                              }
                            }

                        // Summary bar
                        div.dispatch-summary id="dispatchSummary" {
                            span.muted { "Active path: " }
                            span id="activePath" { "—" }
                        }
                    }

                    div.grid {
                        section.card {
                            h2 { "Kernel" }
                            table.stats {
                                tr { td { "Runtime" } td id="runtimeMode" { "-" } }
                                tr { td { "Traits" } td id="traitCount" { "-" } }
                                tr { td { "Namespaces" } td id="namespaceCount" { "-" } }
                                tr { td { "Version" } td id="buildVersion" { "-" } }
                                tr { td { "Uptime" } td id="uptimeHuman" { "-" } }
                            }
                        }

                        section.card {
                            h2 { "System tools" }
                            p.note id="terminalNote" {
                              "Click any command to run it, or type your own above."
                            }
                            table.tools {
                                tr {
                                    td { "List traits" }
                                td colspan="2" { a href="#" onclick="setCmd('list');runCmd();return false" { code { "traits list" } } }
                                }
                                tr {
                                    td { "Version" }
                                td colspan="2" { a href="#" onclick="setCmd('version');runCmd();return false" { code { "traits version" } } }
                                }
                                tr {
                                    td { "Registry" }
                                td colspan="2" { a href="#" onclick="setCmd('registry');runCmd();return false" { code { "traits registry" } } }
                                }
                                tr {
                                    td { "Processes" }
                                td colspan="2" { a href="#" onclick="setCmd('ps');runCmd();return false" { code { "traits ps" } } }
                                }
                                tr {
                                    td { "Run tests" }
                                td colspan="2" { a href="#" onclick="setCmd(\"test_runner 'sys.*'\");runCmd();return false" { code { "traits test_runner 'sys.*'" } } }
                                }
                                tr {
                                    td { "Reload registry" }
                                td colspan="2" { a href="#" onclick="setCmd('call kernel.reload');runCmd();return false" { code { "traits call kernel.reload" } } }
                                }
                            }
                        }
                    }

                    div.grid {
                        section.card {
                            h2 { "Secrets" }
                            p.note {
                                "Stored only in this browser under " code { "localStorage['traits.secret.*']" } ". "
                                "This page never syncs them to our server. They are not automatically injected into trait calls."
                            }
                            table id="secretTable" {
                                tr { td colspan="3" { "Loading..." } }
                            }
                            div.form-row {
                                input id="secretKey" type="text" placeholder="Secret ID";
                                input id="secretValue" type="password" placeholder="Secret value";
                                button class="primary" onclick="saveSecret()" { "Store" }
                            }
                            p.inline-status id="secretStatus" {}
                        }

                        section.card {
                            h2 { "Environment" }
                            p.note {
                                "Stored only in this browser under " code { "localStorage['traits.env.*']" } ". "
                                "Environment variables here are not encrypted and are never sent automatically."
                            }
                            table id="envTable" {
                                tr { td colspan="4" { "Loading..." } }
                            }
                            div.form-row {
                                input id="envKey" type="text" placeholder="Variable name";
                                input id="envValue" type="text" placeholder="Variable value";
                                button class="primary" onclick="saveEnvVar()" { "Store" }
                            }
                            p.inline-status id="envStatus" {}
                        }
                    }

                    section.card.log-card {
                        h2 { "Activity" }
                        div.log id="activityLog" {
                            span.entry { span.time { "[--:--:--]" } " Ready" }
                        }
                    }
                }

                script { (PreEscaped(JS)) }
            }
        }
    };

    Value::String(markup.into_string())
}

const CSS: &str = r##"
:root {
  --bg: #0b1115;
  --panel: #121b21;
  --panel-2: #17232c;
  --line: #243744;
  --text: #e7eef3;
  --muted: #9ab0bf;
  --accent: #59d5b0;
  --warn: #f5b942;
  --danger: #ef6b73;
}

* { box-sizing: border-box; }
body {
  margin: 0;
  background:
    radial-gradient(circle at top left, rgba(89, 213, 176, 0.08), transparent 28%),
    linear-gradient(180deg, #081015 0%, var(--bg) 100%);
  color: var(--text);
  font-family: "Iosevka Aile", "IBM Plex Sans", "Segoe UI", sans-serif;
}
.page {
  max-width: 1120px;
  margin: 0 auto;
  padding: 32px 20px 48px;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
  gap: 18px;
  margin-bottom: 18px;
}
.card {
  background: linear-gradient(180deg, rgba(23, 35, 44, 0.96), rgba(18, 27, 33, 0.96));
  border: 1px solid var(--line);
  border-radius: 18px;
  padding: 20px;
  box-shadow: 0 20px 48px rgba(0, 0, 0, 0.22);
}
.hero {
  margin-bottom: 18px;
  position: relative;
  overflow: hidden;
}
.hero::after {
  content: "";
  position: absolute;
  inset: auto -40px -60px auto;
  width: 180px;
  height: 180px;
  border-radius: 999px;
  background: radial-gradient(circle, rgba(89, 213, 176, 0.18), transparent 70%);
  pointer-events: none;
}
.eyebrow {
  margin: 0 0 8px;
  color: var(--accent);
  text-transform: uppercase;
  letter-spacing: 0.16em;
  font-size: 12px;
}
h1 {
  margin: 0;
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: clamp(34px, 5vw, 56px);
  line-height: 0.96;
}
h2 {
  margin: 0 0 12px;
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: 20px;
}
.subtitle {
  margin: 14px 0 0;
  max-width: 760px;
  color: var(--muted);
  line-height: 1.6;
}
.badges {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 16px;
}
.badge {
  border: 1px solid rgba(89, 213, 176, 0.24);
  color: var(--accent);
  border-radius: 999px;
  padding: 6px 10px;
  font-size: 12px;
  letter-spacing: 0.04em;
}
.notice,
.note,
.inline-status {
  color: var(--muted);
  line-height: 1.5;
  font-size: 14px;
}
.notice { margin-top: 14px; }
.status {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 14px;
}
.dot {
  width: 12px;
  height: 12px;
  border-radius: 999px;
  background: #516570;
}
.dot.green { background: var(--accent); box-shadow: 0 0 14px rgba(89, 213, 176, 0.45); }
.dot.yellow { background: var(--warn); box-shadow: 0 0 14px rgba(245, 185, 66, 0.35); }
.dot.red { background: var(--danger); box-shadow: 0 0 14px rgba(239, 107, 115, 0.35); }
.status-text { font-weight: 600; }
table {
  width: 100%;
  border-collapse: collapse;
}
td {
  padding: 10px 0;
  border-bottom: 1px solid rgba(36, 55, 68, 0.65);
  vertical-align: top;
}
td:first-child {
  width: 34%;
  color: var(--muted);
}
.tools td:nth-child(2) {
  width: 46%;
}
.tools td:last-child,
#envTable td:last-child,
#secretTable td:last-child {
  text-align: right;
}
.command-link {
  display: inline-block;
  text-decoration: none;
}
.command-link code {
  transition: transform 0.12s ease, border-color 0.12s ease, background 0.12s ease;
  border: 1px solid transparent;
}
.command-link:hover code {
  transform: translateY(-1px);
  border-color: rgba(89, 213, 176, 0.28);
  background: rgba(16, 32, 38, 0.96);
}
.form-row {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 16px;
}
input,
select,
button {
  border-radius: 12px;
  border: 1px solid var(--line);
  background: rgba(11, 17, 21, 0.92);
  color: var(--text);
  font: inherit;
}
input {
  min-width: 180px;
  flex: 1 1 220px;
  padding: 12px 14px;
}
select {
  min-width: 180px;
  flex: 1 1 220px;
  padding: 10px 12px;
}
button {
  padding: 10px 14px;
  cursor: pointer;
}
button:hover { border-color: #3d5b6c; }
button.primary {
  background: linear-gradient(180deg, #1d7c63, #176551);
  border-color: rgba(89, 213, 176, 0.26);
}
button.danger {
  background: linear-gradient(180deg, #6b2530, #531b24);
  border-color: rgba(239, 107, 115, 0.26);
}
code {
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: 13px;
  color: #bef4e4;
  background: rgba(11, 17, 21, 0.9);
  padding: 3px 6px;
  border-radius: 8px;
}
.log {
  min-height: 180px;
  max-height: 300px;
  overflow: auto;
  padding: 14px;
  border-radius: 14px;
  border: 1px solid var(--line);
  background: rgba(8, 12, 15, 0.95);
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: 13px;
  white-space: pre-wrap;
}
.entry { display: block; margin-bottom: 0; }
.time { color: #5f7583; margin-right: 6px; }
a { color: #8be3cb; }
.preview { color: #c9d7e0; word-break: break-all; }
.chip-row { display: flex; gap: 6px; flex-wrap: wrap; margin-top: 8px; }
.chip {
  color: var(--muted);
  font-size: 12px;
  cursor: pointer;
  padding: 4px 10px;
  border: 1px solid var(--line);
  border-radius: 8px;
  transition: all 0.12s;
}
.chip:hover { color: var(--accent); border-color: rgba(89, 213, 176, 0.3); }
.platform-icon { font-size: 1.3rem; margin-right: 6px; vertical-align: middle; }
.install-row { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; margin-top: 12px; }
.install-alt { color: var(--muted); font-size: 13px; margin-top: 12px; }
.install-alt code { cursor: pointer; transition: color 0.12s; }
.install-alt code:hover { color: var(--accent); }
.install-alt .copied { color: var(--accent); font-size: 12px; margin-left: 6px; }

@media (max-width: 720px) {
  .page { padding: 18px 14px 32px; }
  .card { padding: 16px; border-radius: 16px; }
  td:first-child, .tools td:nth-child(2) { width: auto; }
  .tools td:last-child, #envTable td:last-child, #secretTable td:last-child { text-align: left; }
  .tier-controls { padding-left: 16px; }
}

/* Dispatch tiers */
.tier {
  position: relative;
  padding-left: 28px;
  padding-bottom: 6px;
  margin-bottom: 2px;
}
.tier:not(:last-of-type)::before {
  content: "";
  position: absolute;
  left: 5px;
  top: 18px;
  bottom: -2px;
  width: 2px;
  background: var(--line);
}
.tier-header {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 28px;
}
.tier-header .dot {
  position: absolute;
  left: 0;
  width: 12px;
  height: 12px;
}
.tier-label {
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: 14px;
  font-weight: 600;
  white-space: nowrap;
}
.tier-detail {
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tier-controls {
  padding-left: 0;
  margin-top: 6px;
}
.tier-controls .compact-note {
  margin: 0 0 6px;
  font-size: 12px;
}
.form-row.compact {
  margin-top: 6px;
  gap: 6px;
}
.form-row.compact input {
  padding: 8px 10px;
  font-size: 13px;
  min-width: 100px;
}
button.sm {
  padding: 7px 12px;
  font-size: 13px;
}
.dispatch-summary {
  margin-top: 14px;
  padding: 10px 14px;
  border-radius: 10px;
  background: rgba(8, 12, 15, 0.6);
  border: 1px solid var(--line);
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: 13px;
}
.tier-header .tier-url {
  font-size: 12px;
  color: var(--muted);
  opacity: 0.7;
}
"##;

const JS: &str = r##"
(function() {
// ═══════════════════════════════════════════════════════════════
// Platform detection + Setup card
// ═══════════════════════════════════════════════════════════════
(function setupCard() {
  var ua = navigator.userAgent || '';
  var p = navigator.platform || '';
  var os = 'unknown', icon = '💻', label = 'Unknown';
  if (/iPhone|iPad|iPod/.test(ua) || (/Mac/.test(p) && 'ontouchend' in document)) { os = 'ios'; icon = '📱'; label = 'iPhone / iPad'; }
  else if (/Android/.test(ua)) { os = 'android'; icon = '📱'; label = 'Android'; }
  else if (/Mac/.test(p) || /Mac/.test(ua)) { os = 'macos'; icon = '🍎'; label = 'macOS'; }
  else if (/Win/.test(p) || /Win/.test(ua)) { os = 'windows'; icon = '🪟'; label = 'Windows'; }
  else if (/Linux/.test(p) || /Linux/.test(ua)) { os = 'linux'; icon = '🐧'; label = 'Linux'; }
  else if (/CrOS/.test(ua)) { os = 'chromeos'; icon = '💻'; label = 'ChromeOS'; }

  var badge = document.getElementById('platformBadge');
  if (badge) badge.textContent = label;

  var el = document.getElementById('platformInfo');
  if (!el) return;

  var h = '<span class="platform-icon">' + icon + '</span> <strong>' + label + '</strong><br>';

  if (os === 'ios' || os === 'android') {
    h += '<p class="note" style="margin-top:8px;">No install needed — everything runs in your browser.</p>';
    h += '<div class="install-row">';
    h += '<button class="primary" onclick="addToHomeScreen()">Add to Home Screen</button>';
    h += '</div>';
    h += '<p class="note" style="margin-top:12px;">Traits run via the remote API. For heavy workloads, set up a local server on a desktop machine.</p>';
  } else {
    h += '<p class="note" style="margin-top:8px;">Install the traits runtime to run commands locally.</p>';
    h += '<div class="install-row">';
    h += '<button class="primary" onclick="copyCmd(\'install\')">' + icon + ' Install</button>';
    h += '<button onclick="copyCmd(\'run\')">Run Once (no install)</button>';
    h += '</div>';
    h += '<div class="install-alt" id="installHint">Commands are copied to clipboard — paste in Terminal to run.</div>';
  }

  el.innerHTML = h;
})();

function copyCmd(mode) {
  var cmd = '';
  if (mode === 'run') {
    cmd = 'curl -fsSL https://traits.build/local/traits.sh | bash';
  } else {
    cmd = 'curl -fsSL https://traits.build/local/install.sh | bash';
  }
  navigator.clipboard.writeText(cmd).then(function() {
    var hint = document.getElementById('installHint');
    if (hint) {
      hint.innerHTML = '<span class="copied">Copied!</span> Now open <strong>Terminal</strong> and paste with <kbd>⌘V</kbd>';
      setTimeout(function() { hint.innerHTML = 'Commands are copied to clipboard — paste in Terminal to run.'; }, 4000);
    }
    log('Copied to clipboard: ' + cmd);
  });
}

function addToHomeScreen() {
  if (window.deferredPrompt) {
    window.deferredPrompt.prompt();
  } else {
    var isIOS = /iPhone|iPad|iPod/.test(navigator.userAgent);
    if (isIOS) alert('Tap the Share button then "Add to Home Screen"');
    else alert('Open browser menu and tap "Add to Home Screen"');
  }
}
window.addEventListener('beforeinstallprompt', function(e) { e.preventDefault(); window.deferredPrompt = e; });

// ═══════════════════════════════════════════════════════════════
// Run Command
// ═══════════════════════════════════════════════════════════════
function setCmd(cmd) {
  document.getElementById('cmdInput').value = cmd;
  document.getElementById('cmdInput').focus();
}

document.getElementById('cmdInput').addEventListener('keydown', function(e) {
  if (e.key === 'Enter') runCmd();
});

function cmdLog(msg, type) {
  var el = document.getElementById('cmdLog');
  el.style.display = 'block';
  var t = new Date().toTimeString().slice(0,8);
  var cls = type || '';
  el.innerHTML += '<span class="entry"><span class="time">[' + t + ']</span> <span' + (cls ? ' class="'+cls+'"' : '') + '>' + esc(msg) + '</span></span>';
  el.scrollTop = el.scrollHeight;
}

async function runCmd() {
  var raw = document.getElementById('cmdInput').value.trim();
  if (!raw) return;
  var parts = [];
  var rx = /("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\S+)/g;
  var m;
  while ((m = rx.exec(raw)) !== null) {
    var tok = m[1];
    if ((tok[0] === '"' || tok[0] === "'") && tok[tok.length-1] === tok[0])
      tok = tok.slice(1, -1);
    parts.push(tok);
  }
  if (parts.length === 0) return;
  var cmd = parts[0];
  var args = parts.slice(1);
  var traitPath = cmd;
  if (cmd.indexOf('.') === -1) traitPath = 'sys.' + cmd;
  document.getElementById('btnRun').disabled = true;
  document.getElementById('btnRun').textContent = '...';
  cmdLog('$ traits ' + raw);
  try {
    var r = await callTrait(traitPath, args);
    if (r.error) {
      cmdLog('Error: ' + (r.error || 'Unknown'), 'error');
    } else {
      var d = r.result;
      if (typeof d === 'string') {
        d.split('\n').forEach(function(line) { cmdLog(line); });
      } else if (d !== null && d !== undefined) {
        JSON.stringify(d, null, 2).split('\n').forEach(function(line) { cmdLog(line); });
      }
    }
  } catch(e) { cmdLog('Error: ' + e.message, 'error'); }
  document.getElementById('btnRun').disabled = false;
  document.getElementById('btnRun').textContent = 'Run';
}

// ═══════════════════════════════════════════════════════════════
// Secrets, Env, and Status
// ═══════════════════════════════════════════════════════════════
const SECRET_PFX = 'traits.secret.';
const ENV_PFX = 'traits.env.';
const BG_IMPL_KEY = 'traits.background.impl';
const SHELL_ROUTE_KEY = 'traits.shell.route';
const PENDING_COMMAND_KEY = 'traits.pending.terminal.command';
const API_DOCS_FRAGMENT = '#tag/infrastructure/operation/list_traits';
const isLocalFile = location.protocol === 'file:' || (typeof window.TraitsWasm !== 'undefined');
const memoryStorage = (() => {
  const store = new Map();
  return {
    get length() { return store.size; },
    key(index) { return Array.from(store.keys())[index] || null; },
    getItem(key) { return store.has(key) ? store.get(key) : null; },
    setItem(key, value) { store.set(String(key), String(value)); },
    removeItem(key) { store.delete(String(key)); },
  };
})();

function resolveStorage() {
  try {
    if (typeof window !== 'undefined' && window.localStorage) {
      const probe = '__traits_spa_probe__';
      window.localStorage.setItem(probe, '1');
      window.localStorage.removeItem(probe);
      return { backend: window.localStorage, persistent: true };
    }
  } catch (_error) {
  }
  return { backend: memoryStorage, persistent: false };
}

const storageState = resolveStorage();
const storage = storageState.backend;

function safeSessionSet(key, value) {
  try {
    if (typeof window !== 'undefined' && window.sessionStorage) {
      window.sessionStorage.setItem(key, value);
      return true;
    }
  } catch (_error) {
  }
  return false;
}

function esc(value) {
  const div = document.createElement('div');
  div.textContent = String(value == null ? '' : value);
  return div.innerHTML;
}

function byId(id) {
  return document.getElementById(id);
}

function log(message, type) {
  const el = byId('activityLog');
  if (!el) return;
  const t = new Date().toTimeString().slice(0, 8);
  const safe = esc(message);
  const tone = type ? ` class="${type}"` : '';
  el.innerHTML += `<span class="entry"><span class="time">[${t}]</span><span${tone}>${safe}</span></span>`;
  el.scrollTop = el.scrollHeight;
}

async function callTrait(path, args) {
  const callArgs = args || [];
  if (window._traitsSDK) {
    const result = await window._traitsSDK.call(path, callArgs);
    if (result.ok) return result;
    if (result.dispatch === 'rest') return result;
  }
  if (!isLocalFile) {
    try {
      const rest = path.replace(/\./g, '/');
      const res = await fetch(`${location.origin}/traits/${rest}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args: callArgs })
      });
      const data = await res.json();
      return {
        ok: res.ok,
        result: res.ok ? data.result : undefined,
        error: res.ok ? undefined : (data.error || `HTTP ${res.status}`),
        dispatch: 'rest'
      };
    } catch (error) {
      return { ok: false, error: error.message || String(error), dispatch: 'rest' };
    }
  }
  return { ok: false, error: `Trait unavailable in this runtime: ${path}`, dispatch: 'none' };
}

function storageEntries(prefix) {
  const out = [];
  for (let index = 0; index < storage.length; index++) {
    const key = storage.key(index);
    if (key && key.indexOf(prefix) === 0) {
      out.push({
        key: key.slice(prefix.length),
        value: storage.getItem(key) || ''
      });
    }
  }
  out.sort((left, right) => left.key.localeCompare(right.key));
  return out;
}

function previewValue(value) {
  if (!value) return '<span class="preview">(empty)</span>';
  if (value.length <= 48) return `<span class="preview">${esc(value)}</span>`;
  return `<span class="preview">${esc(value.slice(0, 28))} ... ${esc(value.slice(-12))}</span>`;
}

function apiDocsUrl(fragment) {
  const suffix = fragment || API_DOCS_FRAGMENT;
  if (isLocalFile) return `#/docs/api`;
  return `/docs/api${suffix}`;
}

// ── Stats via TC (Trait Component) ──
// The dispatch card has data-trait="sys.list" data-handler="refreshStats" data-interval="30000"
// TC auto-calls sys.list on mount + every 30s, passes result to this handler.

TC.on('refreshStats', async (el, traits, meta) => {
  const ns = new Set(traits.map(t => (t.path || '').split('.')[0]).filter(Boolean));
  const ver = await callTrait('sys.version', []);
  const version = ver.ok
    ? (ver.result.version || ver.result.date || JSON.stringify(ver.result))
    : '-';

  byId('runtimeMode').textContent = meta.dispatch || (isLocalFile ? 'wasm' : 'rest');
  byId('traitCount').textContent = String(traits.length);
  byId('namespaceCount').textContent = String(ns.size);
  byId('buildVersion').textContent = String(version || '-');
  byId('uptimeHuman').textContent = '-';

  if (!isLocalFile) {
    try {
      const resp = await fetch(`${location.origin}/health`);
      const health = await resp.json();
      if (health && resp.ok) {
        byId('uptimeHuman').textContent = health.uptime_human || '-';
        if (health.version) byId('buildVersion').textContent = health.version;
      }
    } catch (_) {}
  }

  refreshDispatchStatus();
});

// Error fallback for stats loading
document.querySelector('[data-handler="refreshStats"]')
  ?.addEventListener('trait:error', (e) => {
    byId('runtimeMode').textContent = isLocalFile ? 'file' : 'rest';
    byId('traitCount').textContent = '-';
    byId('namespaceCount').textContent = '-';
    byId('buildVersion').textContent = '-';
    byId('uptimeHuman').textContent = '-';
    refreshDispatchStatus();
    log('Stats error: ' + (e.detail?.error || 'Unknown'), 'error');
  });

function renderSecrets() {
  const rows = storageEntries(SECRET_PFX);
  const table = byId('secretTable');
  if (!rows.length) {
    table.innerHTML = '<tr><td colspan="3">No local secrets stored</td></tr>';
    return;
  }
  table.innerHTML = rows.map((entry) => {
    const encodedKey = encodeURIComponent(entry.key);
    return `<tr><td><code>${esc(entry.key)}</code></td><td>******</td><td><button class="danger" onclick="deleteSecret(decodeURIComponent('${encodedKey}'))">Delete</button></td></tr>`;
  }).join('');
}

function renderEnvVars() {
  const rows = storageEntries(ENV_PFX);
  const table = byId('envTable');
  if (!rows.length) {
    table.innerHTML = '<tr><td colspan="4">No local environment variables stored</td></tr>';
    return;
  }
  table.innerHTML = rows.map((entry) => {
    const encodedKey = encodeURIComponent(entry.key);
    return `<tr><td><code>${esc(entry.key)}</code></td><td>${previewValue(entry.value)}</td><td>${entry.value.length} chars</td><td><button class="danger" onclick="deleteEnvVar(decodeURIComponent('${encodedKey}'))">Delete</button></td></tr>`;
  }).join('');
}

function setStatus(elId, message, isError) {
  const el = $(elId);
  el.textContent = message || '';
  el.style.color = isError ? '#ef6b73' : '#9ab0bf';
  if (message) {
    setTimeout(() => {
      if (el.textContent === message) el.textContent = '';
    }, 4000);
  }
}

function saveSecret() {
  const key = byId('secretKey').value.trim();
  const value = byId('secretValue').value;
  if (!key || !value) {
    setStatus('secretStatus', 'Secret ID and value are required.', true);
    return;
  }
  storage.setItem(SECRET_PFX + key, value);
  byId('secretKey').value = '';
  byId('secretValue').value = '';
  renderSecrets();
  setStatus('secretStatus', storageState.persistent ? 'Stored locally.' : 'Stored for this session only.');
  log(`Stored local secret: ${key}`);
}

function deleteSecret(key) {
  storage.removeItem(SECRET_PFX + key);
  renderSecrets();
  setStatus('secretStatus', `Deleted ${key}.`);
  log(`Deleted local secret: ${key}`);
}

function saveEnvVar() {
  const key = byId('envKey').value.trim();
  const value = byId('envValue').value;
  if (!key) {
    setStatus('envStatus', 'Variable name is required.', true);
    return;
  }
  storage.setItem(ENV_PFX + key, value);
  byId('envKey').value = '';
  byId('envValue').value = '';
  renderEnvVars();
  setStatus('envStatus', storageState.persistent ? 'Stored locally.' : 'Stored for this session only.');
  log(`Stored local environment variable: ${key}`);
}

function deleteEnvVar(key) {
  storage.removeItem(ENV_PFX + key);
  renderEnvVars();
  setStatus('envStatus', `Deleted ${key}.`);
  log(`Deleted local environment variable: ${key}`);
}

function configureLinks() {
  // Links now use setCmd+runCmd directly; no special configuration needed.
}

try {
  configureLinks();
} catch (error) {
  log(`Link setup error: ${error.message || error}`, 'error');
}

// Stats loading is handled automatically by TC via data-trait="sys.list" on mount + interval

try {
  renderSecrets();
  renderEnvVars();
  if (!storageState.persistent) {
    log('localStorage unavailable; using in-memory storage for this session.', 'warn');
  }
} catch (error) {
  log(`Storage UI error: ${error.message || error}`, 'error');
}

// Expose functions to onclick handlers in HTML
window.copyCmd = copyCmd;
window.addToHomeScreen = addToHomeScreen;
window.setCmd = setCmd;
window.runCmd = runCmd;
window.saveSecret = saveSecret;
window.deleteSecret = deleteSecret;
window.saveEnvVar = saveEnvVar;
window.deleteEnvVar = deleteEnvVar;
window.connectRelay = connectRelay;
window.disconnectRelay = disconnectRelay;
window.connectHelper = connectHelperUI;
window.disconnectHelper = disconnectHelperUI;
window.applyBackgroundBinding = applyBackgroundBinding;

// ═══════════════════════════════════════════════════════════════
// Dispatch Status (unified 4-tier view)
// ═══════════════════════════════════════════════════════════════

function setTier(id, color, detail) {
  var dot = byId('dot' + id);
  var txt = byId(id.charAt(0).toLowerCase() + id.slice(1) + 'Detail');
  if (dot) dot.className = 'dot ' + color;
  if (txt) txt.textContent = detail;
}

function initBackgroundBinding() {
  var select = byId('backgroundImpl');
  if (!select) return;
  var saved = storage.getItem(BG_IMPL_KEY) || 'sdk.background.worker';
  select.value = saved;
}

async function applyBackgroundBinding() {
  var sdk = window._traitsSDK;
  var select = byId('backgroundImpl');
  if (!sdk || !select) return;
  var impl = select.value || 'sdk.background.worker';
  try {
    sdk.setBackgroundBinding(impl);
    storage.setItem(BG_IMPL_KEY, impl);
    log('Background binding set: kernel/background -> ' + impl);
    await refreshDispatchStatus();
  } catch (e) {
    setTier('Background', 'red', e.message || String(e));
    log('Failed to set background binding: ' + (e.message || e), 'error');
  }
}

async function refreshDispatchStatus() {
  var sdk = window._traitsSDK;
  if (!sdk) return;
  var s = sdk.status;
  var tiers = [];

  // Tier 1: WASM
  if (s.wasm) {
    setTier('Wasm', 'green', s.callable + ' callable, ' + s.traits + ' registered' + (s.version ? ' — ' + s.version : ''));
    tiers.push('WASM');
  } else {
    setTier('Wasm', 'gray', 'not loaded');
  }

  // Tier 2: Helper
  var hs = sdk.helperStatus;
  if (s.helper && hs) {
    var hDetail = hs.url || 'connected';
    if (hs.version) hDetail += ' — ' + hs.version;
    if (hs.traits_count) hDetail += ' (' + hs.traits_count + ' traits)';
    setTier('Helper', 'green', hDetail);
    byId('btnHelperConnect').style.display = 'none';
    byId('btnHelperDisconnect').style.display = '';
    byId('helperUrl').value = hs.url || '';
    byId('helperControls').style.display = '';
    tiers.push('Helper');
  } else {
    setTier('Helper', 'gray', 'not connected');
    byId('btnHelperConnect').style.display = '';
    byId('btnHelperDisconnect').style.display = 'none';
    byId('helperControls').style.display = '';
  }

  // Tier 3: Relay
  if (s.relay && s.relayCode) {
    try {
      var rs = await sdk.relayStatus();
      if (rs.connected) {
        var rDetail = 'code ' + s.relayCode;
        if (rs.age_seconds !== undefined) rDetail += ' — active ' + formatAge(rs.age_seconds);
        setTier('Relay', 'green', rDetail);
        byId('btnRelayConnect').style.display = 'none';
        byId('btnRelayDisconnect').style.display = '';
        tiers.push('Relay');
      } else {
        setTier('Relay', 'red', 'code ' + s.relayCode + ' — helper offline');
        byId('btnRelayConnect').style.display = '';
        byId('btnRelayDisconnect').style.display = 'none';
      }
    } catch (e) {
      setTier('Relay', 'red', 'error: ' + (e.message || e));
    }
  } else {
    setTier('Relay', 'gray', 'not configured');
    byId('btnRelayConnect').style.display = '';
    byId('btnRelayDisconnect').style.display = 'none';
  }

  // Tier 4: Background binding (worker/direct/tokio)
  var bg = sdk.backgroundStatus ? sdk.backgroundStatus() : null;
  if (bg) {
    var select = byId('backgroundImpl');
    if (select && bg.binding) select.value = bg.binding;
    var detail = bg.binding + ' — workers: ' + bg.workers + ', running: ' + bg.running + ', queued: ' + bg.queued;
    if (bg.binding === 'sdk.background.worker') {
      setTier('Background', 'green', detail);
      tiers.push('Background');
    } else if (bg.binding === 'sdk.background.tokio') {
      setTier('Background', 'yellow', detail + ' (helper-backed)');
      tiers.push('Background');
    } else {
      setTier('Background', 'yellow', detail);
      tiers.push('Background');
    }
  } else {
    setTier('Background', 'gray', 'not available');
  }

  // Summary
  var summary = byId('activePath');
  if (tiers.length > 0) {
    summary.textContent = tiers.join(' → ');
  } else {
    summary.textContent = 'no dispatch available';
  }
}

function formatAge(seconds) {
  if (seconds < 60) return seconds + 's';
  if (seconds < 3600) return Math.floor(seconds / 60) + 'm';
  return Math.floor(seconds / 3600) + 'h ' + Math.floor((seconds % 3600) / 60) + 'm';
}

// ── Helper controls ──

async function connectHelperUI() {
  var url = byId('helperUrl').value.trim();
  if (!url) url = 'http://localhost:8090';
  var sdk = window._traitsSDK;
  if (!sdk) { log('SDK not ready', 'error'); return; }
  setTier('Helper', 'yellow', 'connecting...');
  var res = await sdk.connectHelper(url);
  if (res.ok) {
    log('Helper connected: ' + url);
  } else {
    setTier('Helper', 'red', res.error || 'connection failed');
    log('Helper connection failed: ' + (res.error || url), 'error');
  }
  refreshDispatchStatus();
}

async function disconnectHelperUI() {
  var sdk = window._traitsSDK;
  if (sdk && sdk.disconnectHelper) sdk.disconnectHelper();
  log('Helper disconnected');
  refreshDispatchStatus();
}

// ── Relay controls ──

function initRelay() {
  var code = storage.getItem('traits.relay.code') || '';
  var server = storage.getItem('traits.relay.server') || '';
  byId('relayCode').value = code;
  byId('relayServer').value = server;
  // Stored helper URL
  try {
    var storedHelper = storage.getItem('traits.helper.url');
    if (storedHelper) byId('helperUrl').value = storedHelper;
  } catch(e) {}
  // Initial dispatch status refresh after a short delay for SDK init
  setTimeout(refreshDispatchStatus, 500);
}

async function connectRelay() {
  var code = byId('relayCode').value.trim().toUpperCase();
  if (!code) { setTier('Relay', 'gray', 'enter a pairing code'); return; }
  var server = byId('relayServer').value.trim() || undefined;
  var sdk = window._traitsSDK;
  if (!sdk) { setTier('Relay', 'gray', 'SDK not ready'); return; }
  setTier('Relay', 'yellow', 'connecting...');
  var res = await sdk.connectRelay(code, server);
  if (res.ok) {
    log('Relay connected: ' + code);
  } else {
    setTier('Relay', 'red', res.error || 'connection failed');
    log('Relay connection failed: ' + (res.error || code), 'error');
  }
  refreshDispatchStatus();
}

async function disconnectRelay() {
  var sdk = window._traitsSDK;
  if (sdk) sdk.disconnectRelay();
  byId('relayCode').value = '';
  byId('btnRelayConnect').style.display = '';
  byId('btnRelayDisconnect').style.display = 'none';
  setTier('Relay', 'gray', 'disconnected');
  log('Relay disconnected');
  refreshDispatchStatus();
}

initBackgroundBinding();
initRelay();

// TC handles interval cleanup automatically via TC.cleanup() in injectPage
})();
"##;