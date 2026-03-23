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
</style>
</head>
<body>

<div class="header">
  <h1>traits.build <span>llm test</span></h1>
  <p class="subtitle">Chat with LLM models — OpenAI API or local inference server</p>
</div>

<div class="controls">
  <div class="control-group">
    <label>Provider</label>
    <select id="provider" onchange="onProviderChange()">
      <option value="openai">OpenAI</option>
      <option value="local">Local (wgml / ollama)</option>
    </select>
  </div>
  <div class="control-group">
    <label>Model</label>
    <select id="model"></select>
  </div>
  <div class="control-group" id="localUrlGroup" style="display:none;">
    <label>Local Server URL</label>
    <input type="text" id="localUrl" value="http://127.0.0.1:8080" style="padding:0.5rem 0.75rem; border-radius:6px; border:1px solid #333; background:#151515; color:#e0e0e0; font-size:0.85rem; width:220px;">
  </div>
  <div class="context-toggle">
    <label for="ctxToggle">Include docs context</label>
    <div class="toggle">
      <input type="checkbox" id="ctxToggle" checked>
      <span class="slider"></span>
    </div>
  </div>
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

<script>
const CONTEXT = {{CONTEXT_JSON}};

const MODELS = {
  openai: [
    { value: 'gpt-4.1-nano', label: 'GPT-4.1 Nano' },
    { value: 'gpt-4.1-mini', label: 'GPT-4.1 Mini' },
    { value: 'gpt-4.1',      label: 'GPT-4.1' },
    { value: 'gpt-4o',       label: 'GPT-4o' },
    { value: 'o3-mini',      label: 'o3-mini' },
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

function onProviderChange() {
  const prov = document.getElementById('provider').value;
  const modelSel = document.getElementById('model');
  const localGroup = document.getElementById('localUrlGroup');
  localGroup.style.display = prov === 'local' ? 'flex' : 'none';
  modelSel.innerHTML = '';
  for (const m of MODELS[prov] || []) {
    const opt = document.createElement('option');
    opt.value = m.value;
    opt.textContent = m.label;
    modelSel.appendChild(opt);
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

    const data = await resp.json();
    removeTyping();

    if (data.ok && data.content) {
      let meta = `${data.provider} · ${data.model}`;
      if (data.usage) {
        const u = data.usage;
        meta += ` · ${u.prompt_tokens || 0}+${u.completion_tokens || 0} tokens`;
      }
      appendMessage('assistant', data.content, meta);
      messages.push({ role: 'assistant', content: data.content });
    } else {
      const err = data.error || data.body?.error?.message || 'Unknown error';
      showToast('Error: ' + err);
      appendMessage('assistant', '⚠ ' + err);
    }
  } catch (e) {
    removeTyping();
    showToast('Network error: ' + e.message);
  } finally {
    sending = false;
    document.getElementById('sendBtn').disabled = false;
    input.focus();
  }
}

// Init
onProviderChange();
document.getElementById('input').focus();
</script>
</body>
</html>"##;
