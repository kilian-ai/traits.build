use serde_json::Value;
use maud::{html, DOCTYPE, PreEscaped};

pub fn admin(_args: &[Value]) -> Value {
    let fly_app = kernel_logic::platform::config_get("www.admin", "fly_app", "polygrait-api");
    let fly_region = kernel_logic::platform::config_get("www.admin", "fly_region", "iad");

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "traits.build \u{2014} Settings" }
                style { (PreEscaped(CSS)) }
            }
            body {
                div.container {
                    h1 { "traits.build " span { "settings" } }
                    p.subtitle { "Setup, run commands, and manage your deployment" }

                    // Setup
                    div.card id="setupCard" {
                        h2 { "Setup" }
                        div id="platformInfo" {}
                    }

                    // Run Command
                    div.card {
                        h2 { "Run" }
                        div style="display:flex;gap:0.5rem;align-items:center;" {
                            span style="color:#555;font-family:'Berkeley Mono','SF Mono',monospace;font-size:0.95rem;" { "traits" }
                            input id="cmdInput" type="text" placeholder="list" autocomplete="off" spellcheck="false"
                                style="flex:1;background:#1a1a1a;border:1px solid #333;border-radius:4px;padding:0.5rem 0.7rem;color:#e0e0e0;font-family:'Berkeley Mono','SF Mono',monospace;font-size:0.9rem;" {}
                            button.primary id="btnRun" onclick="runCommand()" { "Run" }
                        }
                        div.examples style="margin-top:0.5rem;" {
                            span.example onclick="setCmd('list')" { "list" }
                            span.example onclick="setCmd('info sys.checksum')" { "info" }
                            span.example onclick="setCmd('checksum hash hello')" { "checksum" }
                            span.example onclick="setCmd('test_runner *')" { "test" }
                            span.example onclick="setCmd('version')" { "version" }
                        }
                        div.log id="cmdLog" style="display:none;margin-top:1rem;" {}
                    }

                    // Server Status
                    div.card {
                        h2 { "Server Status" }
                        div.status {
                            div.dot.gray id="admStatusDot" {}
                            span.status-text id="admStatusText" { "Checking..." }
                        }
                        table id="statusTable" {
                            tr { td { "Traits" } td id="traitCount" { "\u{2014}" } }
                            tr { td { "Namespaces" } td id="nsCount" { "\u{2014}" } }
                            tr { td { "Uptime" } td id="uptime" { "\u{2014}" } }
                            tr { td { "Version" } td id="version" { "\u{2014}" } }
                        }
                    }

                    // Dispatch
                    div.card {
                        h2 { "Dispatch" }
                        p.note { "How clients reach this server. REST is always available here." }
                        div.dispatch-tiers {
                            div.tier-row {
                                div.dot.green id="dotRest" {}
                                span.tier-name { "REST" }
                                span.tier-info id="restInfo" { "checking..." }
                            }
                            div.tier-row {
                                div.dot.gray id="dotRelay" {}
                                span.tier-name { "Relay" }
                                span.tier-info id="relayInfo" { "checking..." }
                            }
                        }
                    }

                    // Fly.io Machine
                    div.card {
                        h2 { "Fly.io Machine" }
                        div.status {
                            div.dot.gray id="flyDot" {}
                            span.status-text id="flyText" { "Checking..." }
                        }
                        div.infra {
                            table {
                                tr { td { "App" } td { code id="cfgFlyApp" { (&fly_app) } } }
                                tr { td { "Region" } td { code id="cfgFlyRegion" { (&fly_region) } } }
                                tr { td { "Machine" } td id="machineId" { "\u{2014}" } }
                                tr { td { "State" } td id="machineState" { "\u{2014}" } }
                                tr { td { "Image" } td id="machineImage" style="word-break:break-all;" { "\u{2014}" } }
                            }
                        }
                        div.actions style="margin-top: 1rem;" {
                            button.primary id="btnDeploy" onclick="deploy()" { "Restart Machine" }
                            button id="btnScale0" onclick="scale(0)" { "Stop (offline)" }
                            button id="btnScale1" onclick="scale(1)" { "Start" }
                            button.danger id="btnDestroy" onclick="if(confirm('Destroy all machines? You will need to fly deploy again.'))destroy()" { "Destroy" }
                        }
                    }

                    // System Tools
                    div.card {
                        h2 { "System Tools" }
                        div.actions {
                            button onclick="listTraits()" { "List Traits" }
                            button onclick="runTests()" { "Run Tests" }
                            button onclick="reloadRegistry()" { "Reload Registry" }
                            button onclick="showVersion()" { "Version" }
                            button onclick="showProcesses()" { "Processes" }
                        }
                        div.log id="sysLog" style="display:none; margin-top: 1rem;" {
                            span.entry { span.time { "[--:--:--]" } " Ready" }
                        }
                    }

                    // Fast Deploy
                    div.card {
                        h2 { "Fast Deploy" }
                        p.note { "Builds amd64 binary in Docker with cached deps, uploads via sftp, restarts machine. Only works from local dev server." }
                        div.actions style="margin-top: 1rem;" {
                            button.primary id="btnFastDeploy" onclick="fastDeploy('build')" { "Build + Deploy" }
                            button id="btnFastUpload" onclick="fastDeploy('upload')" { "Re-upload Last Binary" }
                        }
                        div.log id="deployLog" style="display:none; margin-top: 1rem;" {
                            span.entry { span.time { "[--:--:--]" } " Ready" }
                        }
                    }

                    // Release Pipeline
                    div.card {
                        h2 { "Release Pipeline" }
                        p.note { "Run " code { "sys.release" } " \u{2014} configurable pipeline: build, test, commit, push, tag, publish, deploy." }
                        div style="margin-top: 1rem; display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: flex-end;" {
                            div {
                                label style="font-size:0.75rem;color:#666;display:block;margin-bottom:0.25rem;" { "Commit Message" }
                                input id="releaseMsg" type="text" placeholder="release: vYYMMDD" style="background:#1a1a1a;border:1px solid #333;border-radius:4px;padding:0.4rem 0.6rem;color:#e0e0e0;font-family:'Berkeley Mono','SF Mono',monospace;font-size:0.85rem;width:280px;" {}
                            }
                            label style="font-size:0.82rem;color:#888;display:flex;align-items:center;gap:0.4rem;cursor:pointer;" {
                                input type="checkbox" id="releaseDry" {} " Dry run"
                            }
                        }
                        div.actions style="margin-top: 1rem;" {
                            button.primary onclick="releasePipeline('all')" { "Full Release" }
                            button onclick="releasePipeline('ci')" { "CI (commit+push+tag)" }
                            button onclick="releasePipeline('ship')" { "Ship (CI+deploy)" }
                            button onclick="releasePipeline('commit,push')" { "Commit + Push" }
                        }
                        div.log id="releaseLog" style="display:none; margin-top: 1rem;" {
                            span.entry { span.time { "[--:--:--]" } " Ready" }
                        }
                    }

                    // Deploy Process
                    div.card {
                        h2 { "Deploy Process" }
                        p.note { "Full redeployment requires a local build + push (the buttons above only restart/stop existing machines)." }
                        div.section {
                            h3 { "Build & Deploy (from local machine)" }
                            div.step { span.step-num { "1." } span.step-text { "Build amd64 image: " code { "docker buildx build --platform linux/amd64 -t registry.fly.io/" (&fly_app) ":deployment-vN ." } } }
                            div.step { span.step-num { "2." } span.step-text { "Deploy to Fly: " code { "fly deploy --now --local-only --image registry.fly.io/" (&fly_app) ":deployment-vN" } } }
                            div.step { span.step-num { "3." } span.step-text { "Verify: " code { "curl https://traits.build/health" } } }
                        }
                        div.section {
                            h3 { "Architecture Notes" }
                            div.step { span.step-num { "\u{2022}" } span.step-text { "Binary is Rust-only. All traits compile into the binary via " code { "build.rs" } " (no filesystem needed)." } }
                            div.step { span.step-num { "\u{2022}" } span.step-text { "Traits using " code { "source = \"dylib\"" } " won't work in Docker. Use " code { "source = \"builtin\"" } " instead." } }
                            div.step { span.step-num { "\u{2022}" } span.step-text { "Dockerfile CMD must be " code { "[\"traits\"]" } " (no args). It reads " code { "TRAITS_PORT" } " env and dispatches " code { "kernel.serve" } "." } }
                            div.step { span.step-num { "\u{2022}" } span.step-text { "Must build with " code { "--platform linux/amd64" } " (Fly runs x86_64, Mac builds arm64 by default)." } }
                            div.step { span.step-num { "\u{2022}" } span.step-text { "Secrets: " code { "admin_password" } ", " code { "fly_api_token" } ", " code { "cloudflare_api_token" } " managed via the Secrets card above (encrypted, persisted to disk). Env vars (" code { "ADMIN_PASSWORD" } ", " code { "FLY_API_TOKEN" } ") still work as fallback." } }
                        }
                    }

                    // Deploy Config
                    div.card {
                        h2 { "Deploy Config" }
                        p.note { "Edit deploy settings. Persisted across deploys \u{2014} changes take effect after server restart." }
                        table style="margin-top: 1rem;" {
                            tr {
                                td { "Fly App" }
                                td { input id="cfgFlyAppInput" type="text" value=(&fly_app) style="background:#1a1a1a;border:1px solid #333;border-radius:4px;padding:0.4rem 0.6rem;color:#e0e0e0;font-family:'Berkeley Mono','SF Mono',monospace;font-size:0.85rem;width:100%;" {} }
                            }
                            tr {
                                td { "Fly Region" }
                                td { input id="cfgFlyRegionInput" type="text" value=(&fly_region) style="background:#1a1a1a;border:1px solid #333;border-radius:4px;padding:0.4rem 0.6rem;color:#e0e0e0;font-family:'Berkeley Mono','SF Mono',monospace;font-size:0.85rem;width:100%;" {} }
                            }
                        }
                        div.actions style="margin-top: 1rem;" {
                            button.primary id="btnSaveConfig" onclick="saveConfig()" { "Save" }
                            span id="cfgSaveStatus" style="color:#888;font-size:0.85rem;align-self:center;" {}
                        }
                    }

                    // Secrets
                    div.card {
                        h2 { "Secrets" }
                        p.note { "Manage encrypted secrets via " code { "sys.secrets" } ". Values are encrypted at rest and never returned by the API." }
                        div style="margin-top: 1rem;" {
                            table id="secretsTable" { tr { td colspan="3" style="color:#555;" { "Loading..." } } }
                        }
                        div style="margin-top: 1rem; display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: flex-end;" {
                            div {
                                label style="font-size:0.75rem;color:#666;display:block;margin-bottom:0.25rem;" { "ID" }
                                input id="secretId" type="text" placeholder="e.g. fly_api_token" style="background:#1a1a1a;border:1px solid #333;border-radius:4px;padding:0.4rem 0.6rem;color:#e0e0e0;font-family:'Berkeley Mono','SF Mono',monospace;font-size:0.85rem;width:180px;" {}
                            }
                            div {
                                label style="font-size:0.75rem;color:#666;display:block;margin-bottom:0.25rem;" { "Value" }
                                input id="secretValue" type="password" placeholder="secret value" style="background:#1a1a1a;border:1px solid #333;border-radius:4px;padding:0.4rem 0.6rem;color:#e0e0e0;font-family:'Berkeley Mono','SF Mono',monospace;font-size:0.85rem;width:220px;" {}
                            }
                            button.primary onclick="setSecret()" { "Set Secret" }
                            span id="secretStatus" style="color:#888;font-size:0.82rem;align-self:center;" {}
                        }
                        p.note style="margin-top:0.75rem;" { "Known IDs: " code { "admin_password" } ", " code { "fly_api_token" } ", " code { "cloudflare_api_token" } ". Secrets override matching env vars." }
                    }

                    // Activity Log
                    div.card {
                        h2 { "Activity Log" }
                        div.log id="log" {
                            span.entry { span.time { "[--:--:--]" } " Waiting for commands..." }
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
  .log .entry { display: block; margin-bottom: 0; }
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
  .examples { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .example { color: #557; font-size: 0.78rem; cursor: pointer; padding: 0.15rem 0.5rem; border: 1px solid #252525; border-radius: 3px; transition: all 0.15s; }
  .example:hover { color: #8af; border-color: #446; }
  .platform-badge { display: inline-flex; align-items: center; gap: 0.5rem; background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 6px; padding: 0.4rem 0.8rem; font-size: 0.85rem; margin-bottom: 1rem; }
  .platform-badge .icon { font-size: 1.1rem; }
  .install-row { display: flex; gap: 1rem; align-items: center; flex-wrap: wrap; margin-top: 0.75rem; }
  .install-alt { color: #555; font-size: 0.78rem; margin-top: 0.75rem; }
  .install-alt code { font-size: 0.75rem; cursor: pointer; }
  .install-alt code:hover { color: #aeb; }
  .install-alt .copied { color: #6b9; font-size: 0.75rem; margin-left: 0.5rem; }
  .dispatch-tiers { display: flex; flex-direction: column; gap: 0.6rem; }
  .tier-row { display: flex; align-items: center; gap: 0.75rem; }
  .tier-name { font-family: 'Berkeley Mono', 'SF Mono', monospace; font-size: 0.9rem; font-weight: 600; min-width: 60px; }
  .tier-info { font-size: 0.85rem; color: #888; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
"##;

const JS: &str = r##"
const API = window.location.origin + '/traits';
var _bootTime = Date.now();
var _isLocal = location.protocol === 'file:';
var _jsProviders = {};
var _SECRET_PFX = 'traits.secret.';

// ═══════════════════════════════════════════════════════════════
// Setup — platform detection + one-click install
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

  var el = document.getElementById('platformInfo');
  if (!el) return;

  var h = '<div class="platform-badge"><span class="icon">' + icon + '</span> ' + label + '</div>';

  if (os === 'ios' || os === 'android') {
    h += '<p style="color:#bbb;font-size:0.9rem;">No install needed — everything runs in your browser.</p>';
    h += '<div class="install-row">';
    h += '<button class="primary" onclick="addToHomeScreen()">Add to Home Screen</button>';
    h += '<button onclick="location.href=\'/\'">Open traits.build</button>';
    h += '</div>';
    h += '<p class="note" style="margin-top:1rem;">Traits run via the remote API. For heavy workloads, set up a local server on a desktop machine.</p>';
  } else {
    h += '<p style="color:#bbb;font-size:0.9rem;">Install the traits runtime to run commands locally.</p>';
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
    if (isIOS) {
      alert('Tap the Share button (↑) then "Add to Home Screen"');
    } else {
      alert('Open browser menu (⋮) and tap "Add to Home Screen"');
    }
  }
}
window.addEventListener('beforeinstallprompt', function(e) { e.preventDefault(); window.deferredPrompt = e; });

// ═══════════════════════════════════════════════════════════════
// Run Command — universal trait runner
// ═══════════════════════════════════════════════════════════════
function setCmd(cmd) {
  document.getElementById('cmdInput').value = cmd;
  document.getElementById('cmdInput').focus();
}

document.getElementById('cmdInput').addEventListener('keydown', function(e) {
  if (e.key === 'Enter') runCommand();
});

function cmdLog(msg, type) {
  var el = document.getElementById('cmdLog');
  el.style.display = 'block';
  var t = new Date().toTimeString().slice(0,8);
  var cls = type || 'info';
  el.innerHTML += '<span class="entry"><span class="time">[' + t + ']</span> <span class="' + cls + '">' + esc(msg) + '</span></span>';
  el.scrollTop = el.scrollHeight;
}

async function runCommand() {
  var raw = document.getElementById('cmdInput').value.trim();
  if (!raw) return;
  // Parse: first word is the subcommand (trait name without sys.), rest are args
  // e.g. "checksum hash hello" → sys.checksum ["hash", "hello"]
  // e.g. "list" → sys.list []
  // e.g. "sys.checksum hash hello" → sys.checksum ["hash", "hello"]
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
  // Resolve trait path
  var traitPath = cmd;
  if (cmd.indexOf('.') === -1) {
    traitPath = 'sys.' + cmd;  // bare name → sys.*
  }
  document.getElementById('btnRun').disabled = true;
  document.getElementById('btnRun').textContent = '...';
  cmdLog('$ traits ' + raw);
  try {
    var r = await callTrait(traitPath, args);
    if (r.error) {
      cmdLog('Error: ' + r.error, 'error');
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

function log(msg, type) {
  const el = document.getElementById('log');
  const t = new Date().toTimeString().slice(0,8);
  const cls = type || 'info';
  el.innerHTML += '<span class="entry"><span class="time">[' + t + ']</span> <span class="' + cls + '">' + esc(msg) + '</span></span>';
  el.scrollTop = el.scrollHeight;
}

function esc(s) { const d = document.createElement('div'); d.textContent = s; return d.innerHTML; }

var _statusPaused = false;

async function callTrait(path, args) {
  args = args || [];
  // 1. WASM + REST via SDK (SPA shell provides _traitsSDK)
  if (window._traitsSDK) {
    var r = await window._traitsSDK.call(path, args);
    if (r.ok) return { result: r.result };
    // SDK tried REST and got a server error — propagate, don't retry
    if (r.dispatch === 'rest') return { error: r.error };
  }
  // 2. Direct REST (standalone page served by server, no SDK)
  if (!_isLocal) {
    try {
      var res = await fetch(API + '/' + path, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args: args })
      });
      return await res.json();
    } catch(e) { /* server unreachable — fall through to JS providers */ }
  }
  // 3. JS-native provider (browser-only fallback: localStorage, etc.)
  if (_jsProviders[path]) {
    try { return { result: await _jsProviders[path](args) }; }
    catch(e) { return { error: e.message || String(e) }; }
  }
  return { error: 'No provider for ' + path };
}

// ═══════════════════════════════════════════════════════════════
// JS-native providers — browser OS layer for the WASM kernel.
// Provides localStorage secrets, direct Fly Machines API, etc.
// Active when WASM + REST both can't serve a trait.
// ═══════════════════════════════════════════════════════════════

// ── sys.secrets → localStorage ──
_jsProviders['sys.secrets'] = async function(args) {
  var action = String(args[0] || 'list');
  if (action === 'list') {
    var secrets = [];
    for (var i = 0; i < localStorage.length; i++) {
      var k = localStorage.key(i);
      if (k.indexOf(_SECRET_PFX) === 0) secrets.push(k.slice(_SECRET_PFX.length));
    }
    return { ok: true, secrets: secrets };
  }
  if (action === 'set') {
    if (!args[1] || !args[2]) return { ok: false, error: 'ID and value required' };
    localStorage.setItem(_SECRET_PFX + args[1], String(args[2]));
    return { ok: true };
  }
  if (action === 'delete') {
    localStorage.removeItem(_SECRET_PFX + (args[1] || ''));
    return { ok: true };
  }
  return { ok: false, error: 'Unknown action: ' + action };
};

// ── Fly.io admin traits — server-only (CORS prevents direct browser API access) ──
_jsProviders['www.admin.deploy'] = async function() {
  throw new Error('Fly.io management requires a running traits server');
};
_jsProviders['www.admin.scale'] = async function() {
  throw new Error('Fly.io scaling requires a running traits server');
};
_jsProviders['www.admin.destroy'] = async function() {
  throw new Error('Fly.io management requires a running traits server');
};

// ── www.admin.save_config → localStorage ──
_jsProviders['www.admin.save_config'] = async function(args) {
  localStorage.setItem('traits.config.fly_app', args[0] || '');
  localStorage.setItem('traits.config.fly_region', args[1] || '');
  return { ok: true };
};

// ── Server-only traits (helpful error in browser) ──
_jsProviders['www.admin.fast_deploy'] = async function() {
  return { ok: false, error: 'Fast deploy requires a running traits server' };
};
_jsProviders['kernel.reload'] = async function() {
  var c = (window._traitsSDK && window._traitsSDK.status) ? window._traitsSDK.status.callable : 0;
  return { ok: true, trait_count: c, note: 'WASM registry is static' };
};
_jsProviders['sys.ps'] = async function() { return { processes: [] }; };

async function checkStatus() {
  if (_statusPaused) return;
  // Try /health endpoint (works when HTTP server is reachable)
  try {
    var r = await fetch(window.location.origin + '/health');
    var h = await r.json();
    if (h.status === 'healthy' || h.status === 'running') {
      document.getElementById('admStatusDot').className = 'dot green';
      document.getElementById('admStatusText').textContent = 'Healthy';
    } else {
      document.getElementById('admStatusDot').className = 'dot yellow';
      document.getElementById('admStatusText').textContent = h.status || 'Unknown';
    }
    document.getElementById('traitCount').textContent = h.trait_count || '—';
    document.getElementById('nsCount').textContent = h.namespace_count || '—';
    document.getElementById('uptime').textContent = h.uptime_human || '—';
    document.getElementById('version').textContent = h.version || '—';
    return;
  } catch(e) {}
  // Fallback: WASM SDK info (file:// or cross-origin)
  if (window._traitsSDK && window._traitsSDK.status && window._traitsSDK.status.wasm) {
    document.getElementById('admStatusDot').className = 'dot green';
    document.getElementById('admStatusText').textContent = 'WASM (local)';
    try {
      var traits = await window._traitsSDK.list();
      document.getElementById('traitCount').textContent = traits.length;
      var ns = new Set(traits.map(function(t) { return (t.path || '').split('.')[0]; }));
      document.getElementById('nsCount').textContent = ns.size;
    } catch(e2) {
      document.getElementById('traitCount').textContent = window._traitsSDK.status.callable;
    }
    try {
      var vr = await callTrait('sys.version', []);
      if (vr.result) document.getElementById('version').textContent = vr.result.version || vr.result;
    } catch(e2) {}
    var _el = Math.floor((Date.now() - _bootTime) / 1000);
    var _mm = Math.floor(_el / 60), _ss = _el % 60;
    document.getElementById('uptime').textContent = _mm + 'm ' + _ss + 's (session)';
  } else {
    document.getElementById('admStatusDot').className = 'dot red';
    document.getElementById('admStatusText').textContent = 'Unreachable';
  }
}

function sysLog(msg, type) {
  const el = document.getElementById('sysLog');
  el.style.display = 'block';
  const t = new Date().toTimeString().slice(0,8);
  const cls = type || 'info';
  el.innerHTML += '<span class="entry"><span class="time">[' + t + ']</span> <span class="' + cls + '">' + esc(msg) + '</span></span>';
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
  el.innerHTML += '<span class="entry"><span class="time">[' + t + ']</span> <span class="' + cls + '">' + esc(msg) + '</span></span>';
  el.scrollTop = el.scrollHeight;
}

async function fastDeploy(mode) {
  const isBuild = mode !== 'upload';
  const btn = isBuild ? document.getElementById('btnFastDeploy') : document.getElementById('btnFastUpload');
  const label = isBuild ? 'Build + Deploy' : 'Re-upload';
  btn.disabled = true;
  deployLog(isBuild ? 'Starting build + deploy...' : 'Re-uploading last binary...');
  var startTime = Date.now();
  var stages = isBuild ? [
    [0, 'Copying source to build container...'],
    [5, 'Compiling (deps cached after first run)...'],
    [30, 'Still compiling...'],
    [60, 'Still compiling (first build takes ~3 min)...'],
    [120, 'Almost there...'],
    [180, 'This is taking longer than usual...'],
  ] : [[0, 'Uploading binary...']];
  var stageIdx = 0;
  var timer = setInterval(function() {
    var elapsed = Math.floor((Date.now() - startTime) / 1000);
    btn.textContent = (isBuild ? 'Building' : 'Uploading') + '... ' + elapsed + 's';
    while (stageIdx < stages.length && elapsed >= stages[stageIdx][0]) {
      deployLog(stages[stageIdx][1]);
      stageIdx++;
    }
  }, 1000);
  try {
    const r = await callTrait('www.admin.fast_deploy', [mode]);
    clearInterval(timer);
    var elapsed = Math.floor((Date.now() - startTime) / 1000);
    const d = r.result || r;
    if (d && d.ok) {
      deployLog('Deploy succeeded in ' + elapsed + 's');
      if (d.output) d.output.split('\n').forEach(function(l) { if(l.trim()) deployLog('  ' + l); });
    } else {
      deployLog('Deploy failed after ' + elapsed + 's: ' + (d.error || d.output || JSON.stringify(d)), 'error');
      if (d.output) d.output.split('\n').forEach(function(l) { if(l.trim()) deployLog('  ' + l, 'error'); });
    }
  } catch(e) { clearInterval(timer); deployLog('Error: ' + e.message, 'error'); }
  btn.disabled = false;
  btn.textContent = label;
  setTimeout(checkStatus, 8000);
  setTimeout(checkFlyMachine, 10000);
}

function releaseLog(msg, type) {
  const el = document.getElementById('releaseLog');
  el.style.display = 'block';
  const t = new Date().toTimeString().slice(0,8);
  const cls = type || 'info';
  el.innerHTML += '<span class="entry"><span class="time">[' + t + ']</span> <span class="' + cls + '">' + esc(msg) + '</span></span>';
  el.scrollTop = el.scrollHeight;
}

async function releasePipeline(steps) {
  var msg = document.getElementById('releaseMsg').value.trim() || '';
  var dry = document.getElementById('releaseDry').checked;
  var btns = document.querySelectorAll('.card:nth-of-type(5) button');
  btns.forEach(function(b) { b.disabled = true; });
  releaseLog((dry ? '[DRY RUN] ' : '') + 'Starting release pipeline: ' + steps + (msg ? ' — "' + msg + '"' : ''));
  var startTime = Date.now();
  var timer = setInterval(function() {
    var elapsed = Math.floor((Date.now() - startTime) / 1000);
    btns[0].textContent = 'Running... ' + elapsed + 's';
  }, 1000);
  try {
    const r = await callTrait('sys.release', [steps, msg, dry]);
    clearInterval(timer);
    var elapsed = Math.floor((Date.now() - startTime) / 1000);
    const d = r.result || r;
    if (d && d.steps) {
      d.steps.forEach(function(s) {
        if (s.skipped) { releaseLog('  ○ ' + s.name + ' — skipped'); }
        else if (s.dry_run || s.would_run) { releaseLog('  ◇ ' + s.name + ' — would run'); }
        else if (s.ok) { releaseLog('  ✓ ' + s.name + (s.output ? ' — ' + s.output.split('\n')[0] : '')); }
        else { releaseLog('  ✗ ' + s.name + ' — ' + (s.output || 'failed'), 'error'); }
      });
      var ok = d.ok !== false;
      releaseLog((ok ? 'Pipeline completed' : 'Pipeline failed') + ' in ' + elapsed + 's' + (d.version ? ' — ' + d.version : ''), ok ? 'info' : 'error');
    } else {
      releaseLog('Result: ' + JSON.stringify(d, null, 2));
    }
  } catch(e) { clearInterval(timer); releaseLog('Error: ' + e.message, 'error'); }
  btns.forEach(function(b) { b.disabled = false; });
  btns[0].textContent = 'Full Release';
  setTimeout(checkStatus, 5000);
}

async function checkFlyMachine() {
  if (_statusPaused) return;
  try {
    const r = await callTrait('www.admin.deploy', ['status']);
    if (r.error) {
      document.getElementById('flyDot').className = 'dot yellow';
      var msg = String(r.error);
      document.getElementById('flyText').textContent = msg.length > 50 ? msg.slice(0, 50) + '...' : msg;
      return;
    }
    const d = r.result || r;
    if (typeof d === 'string') {
      document.getElementById('flyDot').className = 'dot red';
      document.getElementById('flyText').textContent = d.length > 60 ? d.slice(0, 60) + '...' : d;
      log('Fly API: ' + d, 'error');
      return;
    }
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
    } else {
      document.getElementById('flyDot').className = 'dot red';
      document.getElementById('flyText').textContent = 'Unexpected response';
      log('Fly API: unexpected response: ' + JSON.stringify(d), 'warn');
    }
  } catch(e) {
    document.getElementById('flyDot').className = 'dot red';
    document.getElementById('flyText').textContent = 'Error: ' + e.message;
    log('Fly check error: ' + e.message, 'error');
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
  if (n > 0) { _statusPaused = false; setTimeout(checkStatus, 3000); }
  else {
    document.getElementById('admStatusDot').className = 'dot red';
    document.getElementById('admStatusText').textContent = 'Stopped';
    document.getElementById('uptime').textContent = '—';
    _statusPaused = true;
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

async function saveConfig() {
  const app = document.getElementById('cfgFlyAppInput').value.trim();
  const region = document.getElementById('cfgFlyRegionInput').value.trim();
  const status = document.getElementById('cfgSaveStatus');
  if (!app || !region) { status.textContent = 'Both fields are required'; status.style.color = '#e55'; return; }
  document.getElementById('btnSaveConfig').disabled = true;
  status.textContent = 'Saving...'; status.style.color = '#888';
  try {
    const r = await callTrait('www.admin.save_config', [app, region]);
    const d = r.result || r;
    if (d && d.ok) {
      status.textContent = 'Saved — restart to apply'; status.style.color = '#6b9';
      document.getElementById('cfgFlyApp').textContent = app;
      document.getElementById('cfgFlyRegion').textContent = region;
      log('Config saved: fly_app=' + app + ', fly_region=' + region);
    } else {
      status.textContent = 'Error: ' + (d.error || JSON.stringify(d)); status.style.color = '#e55';
      log('Config save failed: ' + (d.error || JSON.stringify(d)), 'error');
    }
  } catch(e) { status.textContent = 'Error: ' + e.message; status.style.color = '#e55'; log('Config save error: ' + e.message, 'error'); }
  document.getElementById('btnSaveConfig').disabled = false;
}

async function checkDispatch() {
  // REST — always available since this page is server-rendered
  try {
    const r = await fetch('/health');
    const d = await r.json();
    document.getElementById('dotRest').className = 'dot green';
    document.getElementById('restInfo').textContent = location.origin + ' — ' + (d.version || '?');
    // Relay — from health response
    if (d.relay && d.relay.code) {
      document.getElementById('dotRelay').className = d.relay.connected ? 'dot green' : 'dot yellow';
      var info = d.relay.code;
      if (d.relay.url) info += ' via ' + d.relay.url.replace(/^https?:\\/\\//, '');
      if (!d.relay.connected) info += ' (reconnecting)';
      document.getElementById('relayInfo').textContent = info;
    } else {
      document.getElementById('dotRelay').className = 'dot gray';
      document.getElementById('relayInfo').textContent = 'Not configured (set RELAY_URL)';
    }
  } catch(e) {
    document.getElementById('dotRest').className = 'dot red';
    document.getElementById('restInfo').textContent = 'Unreachable';
    document.getElementById('dotRelay').className = 'dot gray';
    document.getElementById('relayInfo').textContent = '—';
  }
}

checkStatus();
checkFlyMachine();
checkDispatch();
loadSecrets();
var _admTimers = [setInterval(checkStatus, 30000), setInterval(checkFlyMachine, 60000), setInterval(checkDispatch, 30000)];
window._pageCleanup = function() { _admTimers.forEach(clearInterval); };

async function loadSecrets() {
  try {
    const r = await callTrait('sys.secrets', ['list']);
    const d = r.result || r;
    const tbl = document.getElementById('secretsTable');
    if (d && d.ok && d.secrets) {
      if (d.secrets.length === 0) {
        tbl.innerHTML = '<tr><td colspan="3" style="color:#555;">No secrets stored yet</td></tr>';
      } else {
        tbl.innerHTML = d.secrets.map(function(id) {
          return '<tr><td style="width:auto;"><code>' + esc(id) + '</code></td>'
            + '<td style="color:#555;width:100px;text-align:center;">●●●●●●</td>'
            + '<td style="width:80px;text-align:right;"><button style="padding:0.2rem 0.6rem;font-size:0.78rem;" class="danger" onclick="deleteSecret(\'' + esc(id) + '\')">Delete</button></td></tr>';
        }).join('');
      }
    } else {
      tbl.innerHTML = '<tr><td colspan="3" style="color:#e55;">Failed to load</td></tr>';
    }
  } catch(e) {
    document.getElementById('secretsTable').innerHTML = '<tr><td colspan="3" style="color:#e55;">Error: ' + esc(e.message) + '</td></tr>';
  }
}

async function setSecret() {
  var id = document.getElementById('secretId').value.trim();
  var val = document.getElementById('secretValue').value;
  var status = document.getElementById('secretStatus');
  if (!id || !val) { status.textContent = 'ID and value required'; status.style.color = '#e55'; return; }
  status.textContent = 'Saving...'; status.style.color = '#888';
  try {
    const r = await callTrait('sys.secrets', ['set', id, val]);
    const d = r.result || r;
    if (d && d.ok) {
      status.textContent = 'Saved ✓'; status.style.color = '#6b9';
      document.getElementById('secretId').value = '';
      document.getElementById('secretValue').value = '';
      log('Secret set: ' + id);
      loadSecrets();
    } else {
      status.textContent = d.error || 'Failed'; status.style.color = '#e55';
    }
  } catch(e) { status.textContent = 'Error: ' + e.message; status.style.color = '#e55'; }
  setTimeout(function() { status.textContent = ''; }, 4000);
}

async function deleteSecret(id) {
  if (!confirm('Delete secret "' + id + '"?')) return;
  log('Deleting secret: ' + id + '...');
  try {
    const r = await callTrait('sys.secrets', ['delete', id]);
    const d = r.result || r;
    if (d && d.ok) { log('Deleted: ' + id); }
    else { log('Delete failed: ' + (d.error || JSON.stringify(d)), 'error'); }
  } catch(e) { log('Delete error: ' + e.message, 'error'); }
  loadSecrets();
}
"##;
