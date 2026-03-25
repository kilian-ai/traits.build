use maud::{html, DOCTYPE, PreEscaped};
use serde_json::Value;

pub fn spa(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "traits.build - Admin Workspace" }
                style { (PreEscaped(CSS)) }
            }
            body {
                div.page {
                    section.hero.card {
                        p.eyebrow { "SPA admin workspace" }
                        h1 { "Local operator workspace" }
                        p.subtitle {
                            "Browser-only tools for diagnostics, secrets, and environment variables. "
                            "This page stores user data in localStorage and does not expose Fly.io controls."
                        }
                        div.badges {
                            span.badge { "browser-local" }
                            span.badge { "no auth" }
                            span.badge { "localStorage" }
                        }
                        p.notice id="serverAdminNote" {
                            "Deployment controls remain on the secured localhost admin page. "
                            a href="/admin?server=1" target="_blank" rel="noopener" { "Open localhost admin" }
                        }
                    }

                    div.grid {
                        section.card {
                            h2 { "Server stats" }
                            div.status {
                                div.dot.gray id="spaStatusDot" {}
                                span.status-text id="spaStatusText" { "Checking..." }
                            }
                            table.stats {
                                tr { td { "Runtime" } td id="runtimeMode" { "-" } }
                                tr { td { "Traits" } td id="traitCount" { "-" } }
                                tr { td { "Namespaces" } td id="namespaceCount" { "-" } }
                                tr { td { "Version" } td id="buildVersion" { "-" } }
                                tr { td { "Uptime" } td id="uptimeHuman" { "-" } }
                            }
                            p.note {
                                "When the HTTP server is reachable this card reads " code { "/health" } ". "
                                "In file or pure WASM mode it falls back to local trait metadata."
                            }
                        }

                        section.card {
                            h2 { "System tools" }
                            p.note id="terminalNote" {
                                "Every tool below can be tested in the browser terminal. "
                                a href="/wasm" target="_blank" rel="noopener" id="terminalLink" { "Open terminal" }
                            }
                            table.tools {
                                tr {
                                    td { "List traits" }
                                    td { code { "traits list" } }
                                    td { button onclick="copyCommand('traits list')" { "Copy" } }
                                }
                                tr {
                                    td { "Version" }
                                    td { code { "traits version" } }
                                    td { button onclick="copyCommand('traits version')" { "Copy" } }
                                }
                                tr {
                                    td { "Registry" }
                                    td { code { "traits registry" } }
                                    td { button onclick="copyCommand('traits registry')" { "Copy" } }
                                }
                                tr {
                                    td { "Processes" }
                                    td { code { "traits ps" } }
                                    td { button onclick="copyCommand('traits ps')" { "Copy" } }
                                }
                                tr {
                                    td { "Run tests" }
                                    td { code { "traits test_runner 'sys.*'" } }
                                    td { button onclick="copyCommand(\"traits test_runner 'sys.*'\")" { "Copy" } }
                                }
                                tr {
                                    td { "Reload registry" }
                                    td { code { "traits call kernel.reload" } }
                                    td { button onclick="copyCommand('traits call kernel.reload')" { "Copy" } }
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
.form-row {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 16px;
}
input,
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
.entry { display: block; margin-bottom: 6px; }
.time { color: #5f7583; margin-right: 6px; }
a { color: #8be3cb; }
.preview { color: #c9d7e0; word-break: break-all; }

@media (max-width: 720px) {
  .page { padding: 18px 14px 32px; }
  .card { padding: 16px; border-radius: 16px; }
  td:first-child, .tools td:nth-child(2) { width: auto; }
  .tools td:last-child, #envTable td:last-child, #secretTable td:last-child { text-align: left; }
}
"##;

const JS: &str = r##"
const SECRET_PFX = 'traits.secret.';
const ENV_PFX = 'traits.env.';
const isLocalFile = location.protocol === 'file:';
const timers = [];

function esc(value) {
  const div = document.createElement('div');
  div.textContent = String(value == null ? '' : value);
  return div.innerHTML;
}

function $(id) {
  return document.getElementById(id);
}

function log(message, type) {
  const el = $('activityLog');
  const t = new Date().toTimeString().slice(0, 8);
  const safe = esc(message);
  const tone = type ? ` class="${type}"` : '';
  el.innerHTML += `\n<span class="entry"><span class="time">[${t}]</span><span${tone}>${safe}</span></span>`;
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
  for (let index = 0; index < localStorage.length; index++) {
    const key = localStorage.key(index);
    if (key && key.indexOf(prefix) === 0) {
      out.push({
        key: key.slice(prefix.length),
        value: localStorage.getItem(key) || ''
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

function copyCommand(command) {
  if (!navigator.clipboard || !navigator.clipboard.writeText) {
    log('Clipboard API is unavailable in this browser.', 'error');
    return;
  }
  navigator.clipboard.writeText(command)
    .then(() => log(`Copied command: ${command}`))
    .catch((error) => log(`Clipboard error: ${error.message || error}`, 'error'));
}

async function refreshStats() {
  const dot = $('spaStatusDot');
  const text = $('spaStatusText');
  try {
    const listResult = await callTrait('sys.list', []);
    if (!listResult.ok || !Array.isArray(listResult.result)) {
      throw new Error(listResult.error || 'Unable to load trait list');
    }
    const traits = listResult.result;
    const namespaces = new Set(traits.map((trait) => String(trait.path || '').split('.')[0]).filter(Boolean));
    const versionResult = await callTrait('sys.version', []);
    const version = versionResult.ok
      ? (versionResult.result.version || versionResult.result.date || JSON.stringify(versionResult.result))
      : '-';

    $('runtimeMode').textContent = listResult.dispatch || (isLocalFile ? 'wasm' : 'rest');
    $('traitCount').textContent = String(traits.length);
    $('namespaceCount').textContent = String(namespaces.size);
    $('buildVersion').textContent = String(version || '-');
    $('uptimeHuman').textContent = '-';

    if (!isLocalFile) {
      try {
        const response = await fetch(`${location.origin}/health`);
        const health = await response.json();
        if (health && response.ok) {
          $('uptimeHuman').textContent = health.uptime_human || '-';
          if (health.version) $('buildVersion').textContent = health.version;
          dot.className = 'dot green';
          text.textContent = 'Healthy';
          return;
        }
      } catch (_error) {
      }
    }

    dot.className = isLocalFile ? 'dot yellow' : 'dot green';
    text.textContent = isLocalFile ? 'Local browser mode' : 'Reachable';
  } catch (error) {
    dot.className = 'dot red';
    text.textContent = 'Unavailable';
    $('runtimeMode').textContent = isLocalFile ? 'file' : 'rest';
    $('traitCount').textContent = '-';
    $('namespaceCount').textContent = '-';
    $('buildVersion').textContent = '-';
    $('uptimeHuman').textContent = '-';
    log(`Stats error: ${error.message || error}`, 'error');
  }
}

function renderSecrets() {
  const rows = storageEntries(SECRET_PFX);
  const table = $('secretTable');
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
  const table = $('envTable');
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
  const key = $('secretKey').value.trim();
  const value = $('secretValue').value;
  if (!key || !value) {
    setStatus('secretStatus', 'Secret ID and value are required.', true);
    return;
  }
  localStorage.setItem(SECRET_PFX + key, value);
  $('secretKey').value = '';
  $('secretValue').value = '';
  renderSecrets();
  setStatus('secretStatus', 'Stored locally.');
  log(`Stored local secret: ${key}`);
}

function deleteSecret(key) {
  localStorage.removeItem(SECRET_PFX + key);
  renderSecrets();
  setStatus('secretStatus', `Deleted ${key}.`);
  log(`Deleted local secret: ${key}`);
}

function saveEnvVar() {
  const key = $('envKey').value.trim();
  const value = $('envValue').value;
  if (!key) {
    setStatus('envStatus', 'Variable name is required.', true);
    return;
  }
  localStorage.setItem(ENV_PFX + key, value);
  $('envKey').value = '';
  $('envValue').value = '';
  renderEnvVars();
  setStatus('envStatus', 'Stored locally.');
  log(`Stored local environment variable: ${key}`);
}

function deleteEnvVar(key) {
  localStorage.removeItem(ENV_PFX + key);
  renderEnvVars();
  setStatus('envStatus', `Deleted ${key}.`);
  log(`Deleted local environment variable: ${key}`);
}

function configureLinks() {
  if (isLocalFile) {
    $('serverAdminNote').innerHTML = 'Deployment controls stay on the secured localhost admin page when you run a local server.';
    $('terminalNote').innerHTML = 'Every tool below can be tested in the browser terminal once you run the local server at <code>/wasm</code>.';
    $('terminalLink').removeAttribute('href');
  }
}

configureLinks();
refreshStats();
renderSecrets();
renderEnvVars();
timers.push(setInterval(refreshStats, 30000));
window._pageCleanup = function() {
  while (timers.length) clearInterval(timers.pop());
};
"##;