use maud::{html, DOCTYPE, PreEscaped};
use serde_json::Value;

pub fn chat_logs(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Chat Logs — traits.build" }
                style { (PreEscaped(CSS)) }
            }
            body {
                main.page {
                    section.hero.panel {
                        p.kicker { "localhost test surface" }
                        h1 { "Chat logs and durable learnings" }
                        p.subtitle {
                            "Inspect VS Code workspace chat history, select existing sessions, and extract only the stable instructions into "
                            code { "LEARNINGS.md" }
                            "."
                        }
                        div.hero-actions {
                            a href="/docs/api" { "API docs" }
                            a href="/llm-openai" { "LLM test" }
                        }
                    }

                    section.controls.panel {
                        div.control-grid {
                            label.field {
                                span { "Workspace" }
                                select id="workspaceSelect" {}
                            }
                            label.field {
                                span { "Method" }
                                select id="methodSelect" {
                                    option value="auto" selected { "auto" }
                                    option value="all" { "all" }
                                    option value="json" { "json" }
                                    option value="state_vscdb" { "state_vscdb" }
                                }
                            }
                            label.field {
                                span { "Base dir override" }
                                input id="baseDirInput" type="text" placeholder="Optional workspaceStorage root";
                            }
                            label.field {
                              span { "Server origin" }
                              input id="serverOriginInput" type="text" placeholder="Auto-detect localhost server";
                            }
                        }
                        div.button-row {
                            button id="refreshWorkspacesBtn" onclick="refreshWorkspaces()" { "Refresh workspaces" }
                            button class="primary" id="loadChatsBtn" onclick="loadChats()" { "Load chats" }
                        }
                        p.meta id="workspaceMeta" { "Scanning workspaceStorage..." }
                    }

                    section.content-grid {
                        aside.panel.sidebar {
                            div.section-header {
                                h2 { "Sessions" }
                                span.badge id="sessionCountBadge" { "0" }
                            }
                            div.session-list id="sessionList" {
                                p.empty { "Load a workspace to see sessions." }
                            }
                        }

                        section.panel.viewer {
                            div.section-header {
                                h2 id="viewerTitle" { "Transcript" }
                                span.badge id="viewerSource" { "idle" }
                            }
                            div.transcript id="transcriptView" {
                                p.empty { "Select a session to inspect it." }
                            }
                            details.raw {
                                summary { "Raw payload" }
                                pre id="rawPayload" { "{}" }
                            }
                        }
                    }

                    section.panel.learn-panel {
                        div.section-header {
                            h2 { "Learning extraction" }
                            label.toggle {
                                input id="autoScanToggle" type="checkbox" onchange="toggleAutoScan()";
                                span { "Auto scan every 60s" }
                            }
                        }
                        div.learn-grid {
                            label.field.field-wide {
                                span { "Instruction field" }
                                textarea id="instructionInput" rows="5" { "Extract only durable user instructions, preferences, and standing constraints. Ignore one-off tasks and temporary debugging chatter. Return short Markdown bullet points." }
                            }
                            label.field {
                                span { "Output file" }
                                input id="outputPathInput" type="text" value="LEARNINGS.md";
                            }
                            label.field {
                                span { "Model" }
                                input id="modelInput" type="text" value="gpt-4o-mini";
                            }
                        }
                        div.button-row {
                            button class="primary" id="runLearningBtn" onclick="runLearningExtraction()" { "Run extraction" }
                        }
                        pre id="learningResult" class="result-box" { "Waiting for first extraction." }
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
  --bg: #f3efe5;
  --paper: rgba(255, 250, 239, 0.94);
  --ink: #1f2328;
  --muted: #5e6259;
  --line: #cbbfa8;
  --accent: #135d66;
  --accent-soft: #e3f0ed;
  --warn: #9a3412;
  --shadow: 0 24px 48px rgba(53, 44, 27, 0.12);
}
* { box-sizing: border-box; }
body {
  margin: 0;
  min-height: 100vh;
  color: var(--ink);
  background:
    radial-gradient(circle at top right, rgba(19, 93, 102, 0.12), transparent 24%),
    radial-gradient(circle at left center, rgba(190, 144, 72, 0.16), transparent 28%),
    linear-gradient(180deg, #efe8da 0%, var(--bg) 100%);
  font-family: "IBM Plex Sans", "Avenir Next", sans-serif;
}
.page {
  max-width: 1320px;
  margin: 0 auto;
  padding: 28px 20px 48px;
}
.panel {
  background: var(--paper);
  border: 1px solid rgba(203, 191, 168, 0.9);
  border-radius: 22px;
  box-shadow: var(--shadow);
}
.hero {
  padding: 28px;
  margin-bottom: 18px;
}
.kicker {
  margin: 0 0 8px;
  text-transform: uppercase;
  letter-spacing: 0.16em;
  font-size: 12px;
  color: var(--accent);
}
h1, h2 {
  margin: 0;
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
}
h1 { font-size: clamp(34px, 4vw, 56px); line-height: 0.96; }
h2 { font-size: 19px; }
.subtitle {
  max-width: 760px;
  margin: 14px 0 0;
  color: var(--muted);
  line-height: 1.65;
}
.hero-actions {
  display: flex;
  gap: 12px;
  margin-top: 18px;
}
a {
  color: var(--accent);
  text-decoration: none;
  font-weight: 600;
}
a:hover { text-decoration: underline; }
.controls, .learn-panel { padding: 20px; margin-bottom: 18px; }
.control-grid, .learn-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  gap: 14px;
}
.learn-grid { margin-bottom: 14px; }
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.field-wide { grid-column: 1 / -1; }
.field span {
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: var(--muted);
}
input, select, textarea, button {
  font: inherit;
}
input, select, textarea {
  width: 100%;
  border: 1px solid var(--line);
  border-radius: 14px;
  padding: 12px 14px;
  background: rgba(255, 255, 255, 0.86);
  color: var(--ink);
}
textarea {
  resize: vertical;
  min-height: 124px;
  line-height: 1.55;
}
button {
  border: 1px solid var(--line);
  background: #fff8ea;
  color: var(--ink);
  border-radius: 999px;
  padding: 11px 16px;
  cursor: pointer;
  font-weight: 700;
}
button.primary {
  background: var(--accent);
  color: white;
  border-color: var(--accent);
}
button:disabled { opacity: 0.55; cursor: not-allowed; }
.button-row {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  margin-top: 14px;
}
.meta {
  margin: 14px 0 0;
  color: var(--muted);
  font-size: 14px;
}
.content-grid {
  display: grid;
  grid-template-columns: minmax(280px, 360px) minmax(0, 1fr);
  gap: 18px;
  margin-bottom: 18px;
}
.sidebar, .viewer {
  padding: 20px;
  min-height: 520px;
}
.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}
.badge {
  padding: 6px 10px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 12px;
  font-weight: 700;
}
.session-list {
  display: grid;
  gap: 10px;
}
.session-item {
  border: 1px solid var(--line);
  border-radius: 16px;
  padding: 12px;
  background: rgba(255, 255, 255, 0.78);
  cursor: pointer;
}
.session-item.active {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.session-item h3 {
  margin: 0 0 4px;
  font-size: 15px;
}
.session-item p, .session-item small { margin: 0; color: var(--muted); }
.transcript {
  display: grid;
  gap: 12px;
}
.bubble {
  max-width: 90%;
  border-radius: 18px;
  padding: 12px 14px;
  line-height: 1.55;
  white-space: pre-wrap;
}
.bubble.user {
  margin-left: auto;
  background: #f5ede1;
  border: 1px solid #d6c5aa;
}
.bubble.assistant {
  background: #f8fbfb;
  border: 1px solid #c7dbdc;
}
.bubble .meta-line {
  display: block;
  margin-top: 8px;
  font-size: 12px;
  color: var(--muted);
}
.empty { color: var(--muted); }
.raw summary {
  cursor: pointer;
  font-weight: 700;
  margin-top: 18px;
}
pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
}
.raw pre, .result-box {
  border: 1px solid var(--line);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.72);
  padding: 14px;
  margin-top: 10px;
  min-height: 120px;
  color: var(--ink);
}
.toggle {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  color: var(--muted);
}
code {
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  background: rgba(19, 93, 102, 0.08);
  border-radius: 8px;
  padding: 0.15rem 0.4rem;
}
@media (max-width: 960px) {
  .content-grid { grid-template-columns: 1fr; }
}
"##;

const JS: &str = r##"
let workspaceEntries = [];
let normalizedSessions = [];
let selectedSessionId = null;
let autoScanTimer = null;
const SERVER_ORIGIN_KEY = 'traits.chatLogs.serverOrigin';

function traitPathToDot(path) {
  return path.includes('/') ? path.replaceAll('/', '.') : path;
}

function traitPathToRest(path) {
  return path.includes('/') ? path : path.replaceAll('.', '/');
}

function readServerOriginInput() {
  return document.getElementById('serverOriginInput').value.trim();
}

function writeServerOriginInput(value) {
  document.getElementById('serverOriginInput').value = value || '';
}

function isLikelyReachableOrigin(value) {
  return /^https?:\/\//.test(value || '');
}

function candidateServerOrigins() {
  const candidates = [];
  const explicit = readServerOriginInput();
  const stored = (() => {
    try { return localStorage.getItem(SERVER_ORIGIN_KEY) || ''; } catch (_) { return ''; }
  })();
  const current = location.protocol.startsWith('http') ? location.origin : '';
  const defaults = ['http://127.0.0.1:8090', 'http://127.0.0.1:8091', 'http://127.0.0.1:8092'];

  for (const value of [explicit, stored, current, ...defaults]) {
    if (!isLikelyReachableOrigin(value)) continue;
    if (!candidates.includes(value)) candidates.push(value);
  }
  return candidates;
}

function persistServerOrigin(value) {
  if (!isLikelyReachableOrigin(value)) return;
  writeServerOriginInput(value);
  try { localStorage.setItem(SERVER_ORIGIN_KEY, value); } catch (_) {}
}

async function traitsCall(path, args) {
  const dotPath = traitPathToDot(path);
  const restPath = traitPathToRest(path);
  const sdk = window._traitsSDK;

  if (sdk && typeof sdk.call === 'function' && location.protocol.startsWith('http')) {
    const desiredOrigin = readServerOriginInput() || location.origin;
    if (desiredOrigin === location.origin) {
      const result = await sdk.call(dotPath, args);
      if (result?.ok) {
        persistServerOrigin(location.origin);
        return result.result;
      }
    }
  }

  let lastError = 'NetworkError when attempting to fetch resource.';
  for (const origin of candidateServerOrigins()) {
    try {
      const res = await fetch(`${origin}/traits/${restPath}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args })
      });
      const payload = await res.json();
      if (!res.ok || payload.error) {
        throw new Error(payload.error || `Request failed (${res.status})`);
      }
      persistServerOrigin(origin);
      return payload.result;
    } catch (error) {
      lastError = `${error.message || String(error)} Tried: ${origin}`;
    }
  }

  throw new Error(`${lastError} Set Server origin to a running traits server, for example http://127.0.0.1:8090.`);
}

function workspaceBaseDir() {
  const value = document.getElementById('baseDirInput').value.trim();
  return value || null;
}

function setWorkspaceMeta(text) {
  document.getElementById('workspaceMeta').textContent = text;
}

async function refreshWorkspaces() {
  setWorkspaceMeta('Scanning workspaceStorage...');
  const select = document.getElementById('workspaceSelect');
  select.innerHTML = '';
  try {
    const args = [];
    const baseDir = workspaceBaseDir();
    if (baseDir) args.push(baseDir);
    const result = await traitsCall('sys/chat_workspaces', args);
    if (result.ok === false) {
      const isWasm = (result.error || '').includes('not available in WASM');
      const option = document.createElement('option');
      option.value = '';
      option.textContent = 'No workspaces found';
      select.appendChild(option);
      setWorkspaceMeta(isWasm
        ? 'Chat Logs requires a local helper. Run: curl -fsSL https://www.traits.build/local/helper | bash'
        : `Error: ${result.error}`);
      return;
    }
    workspaceEntries = result.workspaces || [];
    if (!workspaceEntries.length) {
      const option = document.createElement('option');
      option.value = '';
      option.textContent = 'No workspaces found';
      select.appendChild(option);
      setWorkspaceMeta(`No workspaces found under ${result.storage_root}`);
      return;
    }
    for (const entry of workspaceEntries) {
      const option = document.createElement('option');
      option.value = entry.workspace_id;
      const folder = entry.workspace && (entry.workspace.folder || entry.workspace.id || entry.workspace.workspaceUri);
      option.textContent = folder ? `${entry.workspace_id} — ${folder}` : entry.workspace_id;
      select.appendChild(option);
    }
    setWorkspaceMeta(`Found ${workspaceEntries.length} workspace candidates.`);
  } catch (error) {
    const opt = document.createElement('option');
    opt.value = '';
    opt.textContent = 'Helper not connected';
    document.getElementById('workspaceSelect').appendChild(opt);
    setWorkspaceMeta('Cannot reach local helper — start it with: curl -fsSL https://www.traits.build/local/helper | bash');
  }
}

function extractTranscript(requests, source) {
  const transcript = [];
  for (const request of (requests || [])) {
    if (request?.message?.text) {
      transcript.push({ role: 'user', text: request.message.text, source });
    }
    for (const response of (request?.response || [])) {
      if (typeof response?.value === 'string') {
        transcript.push({ role: 'assistant', text: response.value, source });
      } else if (Array.isArray(response?.parts)) {
        const text = response.parts.map(p => p?.value || '').filter(Boolean).join('');
        if (text) transcript.push({ role: 'assistant', text, source });
      }
    }
  }
  return transcript;
}

function normalizeSessions(result) {
  const sessions = [];
  const seenIds = new Set();

  // Build a lookup from sessionId → JSON session (for transcript data)
  const jsonBySessionId = {};
  for (const session of (result?.sources?.json?.sessions || [])) {
    if (session?.session_id) jsonBySessionId[session.session_id] = session;
  }

  // Primary source: chat.ChatSessionStore.index — ALL sessions with metadata + titles
  const dbEntries = result?.sources?.state_vscdb?.entries || [];
  const indexEntry = dbEntries.find(e => e?.key === 'chat.ChatSessionStore.index');
  const indexEntries = indexEntry?.value?.entries || {};

  for (const [sessionId, meta] of Object.entries(indexEntries)) {
    if (seenIds.has(sessionId)) continue;
    seenIds.add(sessionId);

    const jsonSession = jsonBySessionId[sessionId];
    const transcript = extractTranscript(jsonSession?.data?.requests, 'json');
    const isEmpty = meta?.isEmpty === true;

    // Skip truly empty sessions with no persisted JSON
    if (isEmpty && !jsonSession) continue;

    const title = meta?.title || jsonSession?.title || sessionId;
    const ts = meta?.lastMessageDate || meta?.timing?.startTime || 0;
    sessions.push({
      id: sessionId,
      title,
      summary: jsonSession
        ? `${jsonSession.request_count || transcript.length} request(s)`
        : (isEmpty ? 'empty' : 'session'),
      source: jsonSession ? 'json+db' : 'db',
      ts,
      transcript,
      raw: jsonSession || meta
    });
  }

  // Sort newest first
  sessions.sort((a, b) => (b.ts || 0) - (a.ts || 0));

  // Fallback: JSON sessions not covered by the index (legacy persisted files)
  for (const session of (result?.sources?.json?.sessions || [])) {
    const sid = session?.session_id;
    if (!sid || seenIds.has(sid)) continue;
    seenIds.add(sid);
    const transcript = extractTranscript(session?.data?.requests, 'json');
    sessions.push({
      id: sid,
      title: session.title || sid,
      summary: `${session.request_count || transcript.length} request(s)`,
      source: 'json',
      ts: 0,
      transcript,
      raw: session
    });
  }

  // Legacy: live interactive history (very old VS Code format)
  const liveHistory = result?.sources?.state_vscdb?.live_history?.value?.history?.copilot || [];
  if (liveHistory.length && !seenIds.has('live-history')) {
    const transcript = liveHistory
      .filter(item => item?.inputText)
      .map(item => ({ role: 'user', text: item.inputText, source: 'state_vscdb' }));
    sessions.push({
      id: 'live-history',
      title: 'Live interactive session (legacy)',
      summary: `${transcript.length} live prompt(s)`,
      source: 'state_vscdb',
      ts: 0,
      transcript,
      raw: liveHistory
    });
  }

  return sessions;
}

function renderSessionList() {
  const list = document.getElementById('sessionList');
  const badge = document.getElementById('sessionCountBadge');
  badge.textContent = String(normalizedSessions.length);
  if (!normalizedSessions.length) {
    list.innerHTML = '<p class="empty">No sessions were found for this workspace.</p>';
    return;
  }
  list.innerHTML = '';
  for (const session of normalizedSessions) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'session-item' + (session.id === selectedSessionId ? ' active' : '');
    button.onclick = () => selectSession(session.id);
    button.innerHTML = `<h3>${escapeHtml(session.title)}</h3><p>${escapeHtml(session.summary)}</p><small>${escapeHtml(session.source)}</small>`;
    list.appendChild(button);
  }
}

function selectSession(sessionId) {
  selectedSessionId = sessionId;
  renderSessionList();
  const session = normalizedSessions.find(item => item.id === sessionId);
  const transcriptView = document.getElementById('transcriptView');
  const rawPayload = document.getElementById('rawPayload');
  const viewerTitle = document.getElementById('viewerTitle');
  const viewerSource = document.getElementById('viewerSource');
  if (!session) {
    transcriptView.innerHTML = '<p class="empty">Select a session to inspect it.</p>';
    rawPayload.textContent = '{}';
    viewerTitle.textContent = 'Transcript';
    viewerSource.textContent = 'idle';
    return;
  }
  viewerTitle.textContent = session.title;
  viewerSource.textContent = session.source;
  transcriptView.innerHTML = '';
  for (const entry of session.transcript) {
    const bubble = document.createElement('div');
    bubble.className = `bubble ${entry.role}`;
    bubble.innerHTML = `${escapeHtml(entry.text)}<span class="meta-line">${escapeHtml(entry.source)}</span>`;
    transcriptView.appendChild(bubble);
  }
  if (!session.transcript.length) {
    transcriptView.innerHTML = '<p class="empty">This session has no normalized transcript entries yet.</p>';
  }
  rawPayload.textContent = JSON.stringify(session.raw, null, 2);
}

async function loadChats() {
  const workspaceId = document.getElementById('workspaceSelect').value;
  const method = document.getElementById('methodSelect').value;
  if (!workspaceId) {
    setWorkspaceMeta('Pick a workspace first.');
    return;
  }
  setWorkspaceMeta(`Loading chat history for ${workspaceId}...`);
  try {
    const args = [workspaceId, method];
    const baseDir = workspaceBaseDir();
    if (baseDir) args.push(baseDir);
    const result = await traitsCall('sys/chat_protocols', args);
    normalizedSessions = normalizeSessions(result);
    selectedSessionId = normalizedSessions[0]?.id || null;
    renderSessionList();
    selectSession(selectedSessionId);
    setWorkspaceMeta(`Loaded ${normalizedSessions.length} session view(s) for ${workspaceId}.`);
  } catch (error) {
    normalizedSessions = [];
    selectedSessionId = null;
    renderSessionList();
    selectSession(null);
    setWorkspaceMeta(error.message);
  }
}

async function runLearningExtraction(backgroundRun = false) {
  const workspaceId = document.getElementById('workspaceSelect').value;
  const instruction = document.getElementById('instructionInput').value.trim();
  const outputPath = document.getElementById('outputPathInput').value.trim() || 'LEARNINGS.md';
  const model = document.getElementById('modelInput').value.trim() || 'gpt-4o-mini';
  const method = document.getElementById('methodSelect').value;
  const resultBox = document.getElementById('learningResult');
  if (!workspaceId || !instruction) {
    resultBox.textContent = 'Workspace and instruction are required.';
    return;
  }
  if (!backgroundRun) {
    resultBox.textContent = 'Running extraction...';
  }
  try {
    const args = [workspaceId, instruction, outputPath];
    const baseDir = workspaceBaseDir();
    if (baseDir) args.push(baseDir);
    else args.push(null);
    args.push(method, model);
    const result = await traitsCall('sys/chat_learnings', args);
    resultBox.textContent = JSON.stringify(result, null, 2);
  } catch (error) {
    resultBox.textContent = error.message;
  }
}

function toggleAutoScan() {
  const enabled = document.getElementById('autoScanToggle').checked;
  if (autoScanTimer) {
    clearInterval(autoScanTimer);
    autoScanTimer = null;
  }
  if (enabled) {
    runLearningExtraction(true);
    autoScanTimer = setInterval(() => runLearningExtraction(true), 60000);
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
    .replaceAll('\n', '<br>');
}

refreshWorkspaces().then(() => {
  if (document.getElementById('workspaceSelect').value) {
    loadChats();
  }
});

writeServerOriginInput((() => {
  try {
    const stored = localStorage.getItem(SERVER_ORIGIN_KEY);
    if (stored) return stored;
  } catch (_) {}
  if (location.protocol.startsWith('http')) return location.origin;
  return 'http://127.0.0.1:8090';
})());
"##;