use serde_json::Value;

/// Context reference config — lists .md files to embed as system context.
#[derive(serde::Deserialize)]
struct ContextRefs {
    files: Vec<String>,
}

/// Load and concatenate all context .md files listed in context_refs.json.
fn load_context() -> String {
    let refs_json = include_str!("context_refs.json");
    let refs: ContextRefs = match serde_json::from_str(refs_json) {
        Ok(r) => r,
        Err(_) => return String::new(),
    };

    let mut ctx = String::from("You are an expert on the traits.build platform. Use the following documentation as reference:\n\n");
    for file_path in &refs.files {
        let content = match file_path.as_str() {
            "docs/intro.md" => include_str!("../../../docs/intro.md"),
            "docs/architecture.md" => include_str!("../../../docs/architecture.md"),
            "docs/trait-definition.md" => include_str!("../../../docs/trait-definition.md"),
            "docs/rest-api.md" => include_str!("../../../docs/rest-api.md"),
            "docs/type-system.md" => include_str!("../../../docs/type-system.md"),
            "docs/creating-traits.md" => include_str!("../../../docs/creating-traits.md"),
            _ => continue,
        };
        ctx.push_str(&format!("--- {} ---\n{}\n\n", file_path, content));
    }
    ctx
}

pub fn llm_test(_args: &[Value]) -> Value {
    let context = load_context();
    let escaped_context = serde_json::to_string(&context).unwrap_or_else(|_| "\"\"".into());

    let html = LLM_TEST_HTML.replace("{{CONTEXT_JSON}}", &escaped_context);
    Value::String(html)
}

const LLM_TEST_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>traits.build — LLM Test</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; min-height: 100vh; display: flex; flex-direction: column; }

  /* Header */
  .header { padding: 1.5rem 2rem 1rem; border-bottom: 1px solid #1a1a1a; }
  .header h1 { font-size: 1.5rem; margin-bottom: 0.25rem; }
  .header h1 span { color: #888; font-weight: 300; }
  .header .subtitle { color: #666; font-size: 0.85rem; }

  /* Controls bar */
  .controls { display: flex; gap: 1rem; padding: 1rem 2rem; border-bottom: 1px solid #1a1a1a; align-items: center; flex-wrap: wrap; }
  .control-group { display: flex; flex-direction: column; gap: 0.25rem; }
  .control-group label { font-size: 0.7rem; color: #666; text-transform: uppercase; letter-spacing: 0.05em; }
  select { padding: 0.5rem 0.75rem; border-radius: 6px; border: 1px solid #333; background: #151515; color: #e0e0e0; font-size: 0.85rem; cursor: pointer; min-width: 160px; }
  select:hover { border-color: #555; }
  select:focus { outline: none; border-color: #1d4ed8; }
  .context-toggle { display: flex; align-items: center; gap: 0.5rem; margin-left: auto; }
  .context-toggle label { font-size: 0.8rem; color: #888; cursor: pointer; }
  .toggle { position: relative; width: 36px; height: 20px; }
  .toggle input { opacity: 0; width: 0; height: 0; }
  .toggle .slider { position: absolute; cursor: pointer; top: 0; left: 0; right: 0; bottom: 0; background: #333; border-radius: 20px; transition: 0.2s; }
  .toggle .slider:before { content: ''; position: absolute; height: 14px; width: 14px; left: 3px; bottom: 3px; background: #888; border-radius: 50%; transition: 0.2s; }
  .toggle input:checked + .slider { background: #1d4ed8; }
  .toggle input:checked + .slider:before { transform: translateX(16px); background: white; }

  /* Progress bar for WebGPU model loading */
  .progress-bar { display: none; padding: 0.5rem 2rem; border-bottom: 1px solid #1a1a1a; }
  .progress-bar.active { display: block; }
  .progress-label { font-size: 0.75rem; color: #888; margin-bottom: 0.35rem; }
  .progress-track { height: 4px; background: #222; border-radius: 2px; overflow: hidden; }
  .progress-fill { height: 100%; background: linear-gradient(90deg, #1d4ed8, #4ade80); border-radius: 2px; width: 0%; transition: width 0.3s ease; }

  /* Chat area */
  .chat-area { flex: 1; overflow-y: auto; padding: 1.5rem 2rem; display: flex; flex-direction: column; gap: 1rem; max-width: 900px; width: 100%; margin: 0 auto; }
  .message { display: flex; gap: 0.75rem; animation: fadeIn 0.2s ease; }
  @keyframes fadeIn { from { opacity: 0; transform: translateY(4px); } to { opacity: 1; transform: none; } }
  .message.user { flex-direction: row-reverse; }
  .avatar { width: 28px; height: 28px; border-radius: 50%; background: #222; display: flex; align-items: center; justify-content: center; font-size: 0.75rem; flex-shrink: 0; color: #888; }
  .message.user .avatar { background: #1d4ed8; color: white; }
  .message.assistant .avatar { background: #1a3a1a; color: #4ade80; }
  .bubble { background: #151515; border: 1px solid #2a2a2a; border-radius: 12px; padding: 0.75rem 1rem; max-width: 75%; font-size: 0.9rem; line-height: 1.6; white-space: pre-wrap; word-wrap: break-word; }
  .message.user .bubble { background: #1a2744; border-color: #1d4ed8; }
  .bubble code { background: #1a1a1a; padding: 0.1rem 0.3rem; border-radius: 3px; font-family: 'Berkeley Mono', 'SF Mono', monospace; font-size: 0.82rem; }
  .bubble pre { background: #0d0d0d; border: 1px solid #222; border-radius: 6px; padding: 0.75rem; margin: 0.5rem 0; overflow-x: auto; }
  .bubble pre code { background: none; padding: 0; }
  .meta-line { font-size: 0.7rem; color: #555; margin-top: 0.4rem; }

  /* Typing indicator */
  .typing { display: flex; gap: 4px; padding: 0.75rem 1rem; }
  .typing span { width: 6px; height: 6px; background: #555; border-radius: 50%; animation: blink 1.4s infinite; }
  .typing span:nth-child(2) { animation-delay: 0.2s; }
  .typing span:nth-child(3) { animation-delay: 0.4s; }
  @keyframes blink { 0%, 80%, 100% { opacity: 0.3; } 40% { opacity: 1; } }

  /* Input area */
  .input-area { border-top: 1px solid #1a1a1a; padding: 1rem 2rem; display: flex; gap: 0.75rem; max-width: 900px; width: 100%; margin: 0 auto; }
  textarea { flex: 1; padding: 0.75rem 1rem; border-radius: 8px; border: 1px solid #333; background: #151515; color: #e0e0e0; font-size: 0.9rem; font-family: inherit; resize: none; min-height: 44px; max-height: 200px; line-height: 1.5; }
  textarea:focus { outline: none; border-color: #1d4ed8; }
  textarea::placeholder { color: #555; }
  button.send { padding: 0.75rem 1.5rem; border-radius: 8px; border: 1px solid #1d4ed8; background: #1d4ed8; color: white; font-size: 0.9rem; cursor: pointer; transition: all 0.15s; white-space: nowrap; }
  button.send:hover { background: #2563eb; }
  button.send:disabled { opacity: 0.5; cursor: not-allowed; }

  /* Empty state */
  .empty-state { flex: 1; display: flex; align-items: center; justify-content: center; }
  .empty-state .inner { text-align: center; color: #444; }
  .empty-state .icon { font-size: 2.5rem; margin-bottom: 0.75rem; }
  .empty-state .hint { font-size: 0.85rem; color: #555; margin-top: 0.5rem; }

  /* Error toast */
  .toast { position: fixed; bottom: 5rem; left: 50%; transform: translateX(-50%); background: #7f1d1d; border: 1px solid #991b1b; color: #fca5a5; padding: 0.75rem 1.25rem; border-radius: 8px; font-size: 0.85rem; z-index: 100; animation: fadeIn 0.2s ease; }

  /* WebGPU badge */
  .webgpu-badge { display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.7rem; padding: 0.2rem 0.5rem; border-radius: 4px; background: #0a2a0a; border: 1px solid #1a4a1a; color: #4ade80; margin-left: 0.5rem; }
  .webgpu-badge.not-supported { background: #2a0a0a; border-color: #4a1a1a; color: #f87171; }
  .webgpu-badge .dot { width: 6px; height: 6px; border-radius: 50%; background: #4ade80; }
  .webgpu-badge.not-supported .dot { background: #f87171; }
</style>
</head>
<body>

<div class="header">
  <h1>traits.build <span>llm test</span></h1>
  <p class="subtitle">Chat with LLM models — OpenAI API, local server, or in-browser WebGPU inference</p>
</div>

<div class="controls">
  <div class="control-group">
    <label>Provider</label>
    <select id="provider" onchange="onProviderChange()">
      <option value="openai">OpenAI</option>
      <option value="webgpu">Browser (WebGPU)</option>
      <option value="local">Local Server (ollama, etc.)</option>
    </select>
  </div>
  <div class="control-group">
    <label>Model</label>
    <select id="model" onchange="onModelChange()"></select>
  </div>
  <div class="control-group" id="localUrlGroup" style="display:none;">
    <label>Local Server URL</label>
    <input type="text" id="localUrl" value="http://127.0.0.1:8080" style="padding:0.5rem 0.75rem; border-radius:6px; border:1px solid #333; background:#151515; color:#e0e0e0; font-size:0.85rem; width:220px;">
  </div>
  <div class="control-group" id="webgpuStatus" style="display:none;">
    <label>WebGPU Engine</label>
    <span id="webgpuBadge" class="webgpu-badge"><span class="dot"></span>Checking...</span>
  </div>
  <div class="context-toggle">
    <label for="ctxToggle">Include docs context</label>
    <div class="toggle">
      <input type="checkbox" id="ctxToggle" checked>
      <span class="slider"></span>
    </div>
  </div>
</div>

<div class="progress-bar" id="progressBar">
  <div class="progress-label" id="progressLabel">Loading model...</div>
  <div class="progress-track"><div class="progress-fill" id="progressFill"></div></div>
</div>

<div class="chat-area" id="chatArea">
  <div class="empty-state" id="emptyState">
    <div class="inner">
      <div class="icon">&#x1F50D;</div>
      <p>Send a message to start chatting</p>
      <p class="hint">Context docs are loaded from the reference file</p>
    </div>
  </div>
</div>

<div class="input-area">
  <textarea id="input" placeholder="Type your message..." rows="1" onkeydown="handleKey(event)"></textarea>
  <button class="send" id="sendBtn" onclick="sendMessage()">Send</button>
</div>

<script type="module">
const CONTEXT = {{CONTEXT_JSON}};

const MODELS = {
  openai: [
    { value: 'gpt-4.1-nano', label: 'GPT-4.1 Nano' },
    { value: 'gpt-4.1-mini', label: 'GPT-4.1 Mini' },
    { value: 'gpt-4.1',      label: 'GPT-4.1' },
    { value: 'gpt-4o',       label: 'GPT-4o' },
    { value: 'o3-mini',      label: 'o3-mini' },
  ],
  webgpu: [
    { value: 'SmolLM2-135M-Instruct-q4f16_1-MLC',   label: 'SmolLM2 135M (tiny, fast)' },
    { value: 'SmolLM2-360M-Instruct-q4f16_1-MLC',   label: 'SmolLM2 360M (small)' },
    { value: 'Qwen2.5-0.5B-Instruct-q4f16_1-MLC',   label: 'Qwen2.5 0.5B' },
    { value: 'Qwen2.5-1.5B-Instruct-q4f16_1-MLC',   label: 'Qwen2.5 1.5B' },
    { value: 'Llama-3.2-1B-Instruct-q4f16_1-MLC',    label: 'Llama 3.2 1B' },
    { value: 'Phi-3.5-mini-instruct-q4f16_1-MLC',    label: 'Phi 3.5 Mini (3.8B)' },
    { value: 'Llama-3.2-3B-Instruct-q4f16_1-MLC',    label: 'Llama 3.2 3B' },
  ],
  local: [
    { value: 'default',  label: 'Default (server model)' },
    { value: 'llama2',   label: 'Llama 2' },
    { value: 'llama3.2', label: 'Llama 3.2' },
    { value: 'qwen2.5',  label: 'Qwen 2.5' },
    { value: 'phi3',     label: 'Phi-3' },
  ]
};

let messages = [];
let sending = false;

// ── WebGPU / WebLLM state ──
let webllm = null;
let webgpuEngine = null;
let webgpuReady = false;
let webgpuLoadedModel = null;
let webgpuSupported = false;

// Check WebGPU support on page load
async function checkWebGPU() {
  if (navigator.gpu) {
    try {
      const adapter = await navigator.gpu.requestAdapter();
      webgpuSupported = !!adapter;
    } catch (e) {
      webgpuSupported = false;
    }
  }
}
checkWebGPU();

function showProgress(text, pct) {
  const bar = document.getElementById('progressBar');
  const label = document.getElementById('progressLabel');
  const fill = document.getElementById('progressFill');
  bar.classList.add('active');
  label.textContent = text;
  fill.style.width = Math.max(0, Math.min(100, pct)) + '%';
}

function hideProgress() {
  document.getElementById('progressBar').classList.remove('active');
}

function updateWebGPUBadge(status, text) {
  const badge = document.getElementById('webgpuBadge');
  badge.textContent = '';
  const dot = document.createElement('span');
  dot.className = 'dot';
  badge.appendChild(dot);
  badge.appendChild(document.createTextNode(text));
  if (status === 'ok') {
    badge.className = 'webgpu-badge';
  } else if (status === 'error') {
    badge.className = 'webgpu-badge not-supported';
  } else {
    badge.className = 'webgpu-badge';
  }
}

async function loadWebLLM() {
  if (webllm) return;
  showProgress('Loading WebLLM library...', 5);
  try {
    webllm = await import('https://esm.run/@mlc-ai/web-llm');
  } catch (e) {
    hideProgress();
    showToast('Failed to load WebLLM: ' + e.message);
    updateWebGPUBadge('error', 'Load failed');
    throw e;
  }
}

async function ensureWebGPUEngine(modelId) {
  if (!webgpuSupported) {
    showToast('WebGPU is not supported in this browser. Use Chrome 113+ or Edge 113+.');
    updateWebGPUBadge('error', 'Not supported');
    return false;
  }

  await loadWebLLM();

  // Already loaded with this model
  if (webgpuEngine && webgpuLoadedModel === modelId) {
    return true;
  }

  // Need to load or switch model
  webgpuReady = false;
  updateWebGPUBadge('loading', 'Loading...');

  try {
    const initProgressCallback = (report) => {
      const pct = Math.round((report.progress || 0) * 100);
      const text = report.text || 'Loading model...';
      showProgress(text, pct);
      updateWebGPUBadge('loading', pct + '%');
    };

    if (webgpuEngine) {
      // Reload with new model
      showProgress('Switching model to ' + modelId + '...', 0);
      webgpuEngine.setInitProgressCallback(initProgressCallback);
      await webgpuEngine.reload(modelId);
    } else {
      showProgress('Initializing WebGPU engine...', 0);
      // Use explicit instantiation + reload instead of CreateMLCEngine
      // for more reliable model loading via CDN
      webgpuEngine = new webllm.MLCEngine();
      webgpuEngine.setInitProgressCallback(initProgressCallback);
      await webgpuEngine.reload(modelId);
    }

    webgpuLoadedModel = modelId;
    webgpuReady = true;
    hideProgress();
    updateWebGPUBadge('ok', 'Ready');
    return true;
  } catch (e) {
    hideProgress();
    updateWebGPUBadge('error', 'Failed');
    showToast('WebGPU model load failed: ' + e.message);
    return false;
  }
}

function onProviderChange() {
  const prov = document.getElementById('provider').value;
  const modelSel = document.getElementById('model');
  const localGroup = document.getElementById('localUrlGroup');
  const webgpuGroup = document.getElementById('webgpuStatus');

  localGroup.style.display = prov === 'local' ? 'flex' : 'none';
  webgpuGroup.style.display = prov === 'webgpu' ? 'flex' : 'none';

  modelSel.innerHTML = '';
  for (const m of MODELS[prov] || []) {
    const opt = document.createElement('option');
    opt.value = m.value;
    opt.textContent = m.label;
    modelSel.appendChild(opt);
  }

  if (prov === 'webgpu') {
    if (!webgpuSupported) {
      updateWebGPUBadge('error', 'Not supported');
    } else if (webgpuReady && webgpuLoadedModel === modelSel.value) {
      updateWebGPUBadge('ok', 'Ready');
    } else {
      updateWebGPUBadge('loading', 'Not loaded');
    }
  }
}

function onModelChange() {
  const prov = document.getElementById('provider').value;
  if (prov === 'webgpu') {
    const modelId = document.getElementById('model').value;
    if (webgpuReady && webgpuLoadedModel === modelId) {
      updateWebGPUBadge('ok', 'Ready');
    } else {
      updateWebGPUBadge('loading', 'Not loaded');
    }
  }
}

function handleKey(e) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
}

function autoResize(el) {
  el.style.height = 'auto';
  el.style.height = Math.min(el.scrollHeight, 200) + 'px';
}

document.getElementById('input').addEventListener('input', function() { autoResize(this); });

function appendMessage(role, content, meta) {
  const emptyState = document.getElementById('emptyState');
  if (emptyState) emptyState.remove();

  const area = document.getElementById('chatArea');
  const div = document.createElement('div');
  div.className = 'message ' + role;

  const avatar = document.createElement('div');
  avatar.className = 'avatar';
  avatar.textContent = role === 'user' ? 'U' : 'AI';

  const bubble = document.createElement('div');
  bubble.className = 'bubble';
  bubble.textContent = content;

  div.appendChild(avatar);
  div.appendChild(bubble);

  if (meta) {
    const metaDiv = document.createElement('div');
    metaDiv.className = 'meta-line';
    metaDiv.textContent = meta;
    bubble.appendChild(metaDiv);
  }

  area.appendChild(div);
  area.scrollTop = area.scrollHeight;
  return bubble;
}

function showTyping() {
  const emptyState = document.getElementById('emptyState');
  if (emptyState) emptyState.remove();

  const area = document.getElementById('chatArea');
  const div = document.createElement('div');
  div.className = 'message assistant';
  div.id = 'typingMsg';

  const avatar = document.createElement('div');
  avatar.className = 'avatar';
  avatar.textContent = 'AI';

  const bubble = document.createElement('div');
  bubble.className = 'bubble typing';
  bubble.innerHTML = '<span></span><span></span><span></span>';

  div.appendChild(avatar);
  div.appendChild(bubble);
  area.appendChild(div);
  area.scrollTop = area.scrollHeight;
}

function removeTyping() {
  const el = document.getElementById('typingMsg');
  if (el) el.remove();
}

function showToast(msg) {
  const existing = document.querySelector('.toast');
  if (existing) existing.remove();
  const toast = document.createElement('div');
  toast.className = 'toast';
  toast.textContent = msg;
  document.body.appendChild(toast);
  setTimeout(() => toast.remove(), 5000);
}

// ── WebGPU in-browser inference ──
async function sendWebGPU(text, modelId, useContext) {
  const ok = await ensureWebGPUEngine(modelId);
  if (!ok) return;

  const chatMessages = [];
  if (useContext) {
    chatMessages.push({ role: 'system', content: CONTEXT });
  }
  // Include conversation history
  for (const m of messages) {
    chatMessages.push({ role: m.role, content: m.content });
  }
  chatMessages.push({ role: 'user', content: text });

  // Use streaming for real-time output
  const chunks = await webgpuEngine.chat.completions.create({
    messages: chatMessages,
    temperature: 0.7,
    max_tokens: 2048,
    stream: true,
    stream_options: { include_usage: true },
  });

  removeTyping();

  // Create assistant bubble for streaming
  const bubble = appendMessage('assistant', '');
  let fullContent = '';
  let usage = null;

  for await (const chunk of chunks) {
    const delta = chunk.choices[0]?.delta?.content || '';
    fullContent += delta;
    bubble.childNodes[0].textContent = fullContent;
    document.getElementById('chatArea').scrollTop = document.getElementById('chatArea').scrollHeight;
    if (chunk.usage) usage = chunk.usage;
  }

  // Add meta line
  let meta = 'webgpu · ' + modelId;
  if (usage) {
    meta += ' · ' + (usage.prompt_tokens || 0) + '+' + (usage.completion_tokens || 0) + ' tokens';
  }
  const metaDiv = document.createElement('div');
  metaDiv.className = 'meta-line';
  metaDiv.textContent = meta;
  bubble.appendChild(metaDiv);

  messages.push({ role: 'assistant', content: fullContent });
}

// ── Server-side inference (OpenAI / Local) ──
async function sendServer(text, provider, model, useContext, localUrl) {
  const body = {
    args: [
      text,
      provider,
      model,
      useContext ? CONTEXT : null,
      provider === 'local' ? localUrl : null
    ]
  };

  const resp = await fetch('/traits/sys/llm', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  });

  const raw = await resp.json();
  removeTyping();

  // REST API wraps trait results in { result, error }
  if (raw.error) {
    showToast('Error: ' + raw.error);
    appendMessage('assistant', '\u26A0 ' + raw.error);
  } else {
    const data = raw.result || raw;
    if (data.ok && data.content) {
      let meta = data.provider + ' \u00B7 ' + data.model;
      if (data.usage) {
        const u = data.usage;
        meta += ' \u00B7 ' + (u.prompt_tokens || 0) + '+' + (u.completion_tokens || 0) + ' tokens';
      }
      appendMessage('assistant', data.content, meta);
      messages.push({ role: 'assistant', content: data.content });
    } else {
      const err = data.error || 'Unknown error';
      showToast('Error: ' + err);
      appendMessage('assistant', '\u26A0 ' + err);
    }
  }
}

async function sendMessage() {
  if (sending) return;

  const input = document.getElementById('input');
  const text = input.value.trim();
  if (!text) return;

  const provider = document.getElementById('provider').value;
  const model = document.getElementById('model').value;
  const useContext = document.getElementById('ctxToggle').checked;
  const localUrl = document.getElementById('localUrl').value;

  input.value = '';
  autoResize(input);

  appendMessage('user', text);
  messages.push({ role: 'user', content: text });

  sending = true;
  document.getElementById('sendBtn').disabled = true;
  showTyping();

  try {
    if (provider === 'webgpu') {
      await sendWebGPU(text, model, useContext);
    } else {
      await sendServer(text, provider, model, useContext, localUrl);
    }
  } catch (e) {
    removeTyping();
    showToast('Error: ' + e.message);
  } finally {
    sending = false;
    document.getElementById('sendBtn').disabled = false;
    input.focus();
  }
}

// Make functions available globally for inline event handlers
window.onProviderChange = onProviderChange;
window.onModelChange = onModelChange;
window.handleKey = handleKey;
window.sendMessage = sendMessage;

// Init
onProviderChange();
document.getElementById('input').focus();
</script>
</body>
</html>"##;
