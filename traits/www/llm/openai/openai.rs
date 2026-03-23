use serde_json::Value;

pub fn openai(_args: &[Value]) -> Value {
    Value::String(HTML.to_string())
}

const HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>LLM Chat — traits.build</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
:root{--bg:#0a0a0f;--fg:#e8e6e3;--accent:#6c63ff;--accent2:#00d4aa;--muted:#6b7280;--card:#12121a;--border:#1e1e2e}
body{font-family:system-ui,-apple-system,sans-serif;background:var(--bg);color:var(--fg);line-height:1.6;min-height:100vh;display:flex;flex-direction:column;align-items:center}
a{color:var(--accent);text-decoration:none}
a:hover{text-decoration:underline}

header{width:100%;padding:1rem 2rem;display:flex;align-items:center;gap:1rem;border-bottom:1px solid var(--border)}
header h1{font-size:1.2rem;font-weight:700}
header h1 span{background:linear-gradient(135deg,var(--accent),var(--accent2));-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}
header nav{margin-left:auto;display:flex;gap:1rem;font-size:0.9rem}
header nav a{color:var(--muted)}
header nav a:hover{color:var(--fg)}

.container{max-width:800px;width:100%;padding:2rem;flex:1;display:flex;flex-direction:column}

.chat-log{flex:1;overflow-y:auto;display:flex;flex-direction:column;gap:1rem;padding-bottom:1rem;min-height:200px}
.msg{padding:0.75rem 1rem;border-radius:12px;max-width:85%;word-wrap:break-word;white-space:pre-wrap;font-size:0.95rem;line-height:1.5}
.msg.user{align-self:flex-end;background:var(--accent);color:#fff;border-bottom-right-radius:4px}
.msg.assistant{align-self:flex-start;background:var(--card);border:1px solid var(--border);border-bottom-left-radius:4px}
.msg.error{align-self:flex-start;background:#2a1520;border:1px solid #5c2030;color:#ff6b6b;font-size:0.85rem}

.input-area{display:flex;gap:0.75rem;padding-top:1rem;border-top:1px solid var(--border)}
.input-area textarea{flex:1;padding:0.75rem 1rem;border-radius:12px;border:1px solid var(--border);background:var(--card);color:var(--fg);font-family:inherit;font-size:0.95rem;resize:none;outline:none;min-height:48px;max-height:200px;transition:border-color 0.2s}
.input-area textarea:focus{border-color:var(--accent)}
.input-area textarea::placeholder{color:var(--muted)}
.input-area button{padding:0.75rem 1.5rem;border-radius:12px;border:none;background:var(--accent);color:#fff;font-weight:600;font-size:0.95rem;cursor:pointer;transition:background 0.2s;white-space:nowrap}
.input-area button:hover{background:#5a52e0}
.input-area button:disabled{opacity:0.5;cursor:not-allowed}

.model-select{display:flex;align-items:center;gap:0.5rem;padding-bottom:0.75rem;font-size:0.85rem;color:var(--muted)}
.model-select select{padding:0.3rem 0.5rem;border-radius:6px;border:1px solid var(--border);background:var(--card);color:var(--fg);font-size:0.85rem;outline:none}
.model-select select:focus{border-color:var(--accent)}

.typing{display:inline-flex;gap:4px;padding:0.5rem 0}
.typing span{width:6px;height:6px;border-radius:50%;background:var(--muted);animation:bounce 1.2s infinite}
.typing span:nth-child(2){animation-delay:0.2s}
.typing span:nth-child(3){animation-delay:0.4s}
@keyframes bounce{0%,60%,100%{transform:translateY(0)}30%{transform:translateY(-8px)}}
</style>
</head>
<body>
<header>
  <h1><span>traits</span>.build</h1>
  <nav>
    <a href="/">Home</a>
    <a href="/docs/api">API</a>
    <a href="/admin">Admin</a>
  </nav>
</header>
<div class="container">
  <div class="model-select">
    <label for="model">Model:</label>
    <select id="model">
      <option value="gpt-4o-mini" selected>gpt-4o-mini</option>
      <option value="gpt-4o">gpt-4o</option>
      <option value="gpt-4.1-nano">gpt-4.1-nano</option>
      <option value="gpt-4.1-mini">gpt-4.1-mini</option>
      <option value="gpt-4.1">gpt-4.1</option>
      <option value="o4-mini">o4-mini</option>
    </select>
  </div>
  <div id="chat" class="chat-log"></div>
  <div class="input-area">
    <textarea id="prompt" placeholder="Type your message..." rows="1"></textarea>
    <button id="send" onclick="sendMessage()">Send</button>
  </div>
</div>
<script>
const chat = document.getElementById('chat');
const promptEl = document.getElementById('prompt');
const sendBtn = document.getElementById('send');
const modelEl = document.getElementById('model');

// Auto-resize textarea
promptEl.addEventListener('input', () => {
  promptEl.style.height = 'auto';
  promptEl.style.height = Math.min(promptEl.scrollHeight, 200) + 'px';
});

// Send on Enter (Shift+Enter for newline)
promptEl.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
});

function addMessage(role, text) {
  const div = document.createElement('div');
  div.className = 'msg ' + role;
  div.textContent = text;
  chat.appendChild(div);
  chat.scrollTop = chat.scrollHeight;
  return div;
}

function showTyping() {
  const div = document.createElement('div');
  div.className = 'msg assistant';
  div.id = 'typing';
  div.innerHTML = '<div class="typing"><span></span><span></span><span></span></div>';
  chat.appendChild(div);
  chat.scrollTop = chat.scrollHeight;
}

function removeTyping() {
  const el = document.getElementById('typing');
  if (el) el.remove();
}

async function sendMessage() {
  const text = promptEl.value.trim();
  if (!text) return;

  addMessage('user', text);
  promptEl.value = '';
  promptEl.style.height = 'auto';
  sendBtn.disabled = true;
  showTyping();

  try {
    const model = modelEl.value;
    const res = await fetch('/traits/llm/openai', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ args: [text, model] })
    });
    removeTyping();
    const data = await res.json();
    if (data.error) {
      addMessage('error', data.error);
    } else if (typeof data.result === 'string') {
      addMessage('assistant', data.result);
    } else {
      addMessage('assistant', JSON.stringify(data.result, null, 2));
    }
  } catch (err) {
    removeTyping();
    addMessage('error', 'Network error: ' + err.message);
  }
  sendBtn.disabled = false;
  promptEl.focus();
}

promptEl.focus();
</script>
</body>
</html>"##;
