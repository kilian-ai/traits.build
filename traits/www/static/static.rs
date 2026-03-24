use serde_json::Value;

pub fn static_page(_args: &[Value]) -> Value {
    Value::String(format!("{HTML_SHELL}"))
}

const HTML_SHELL: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>traits.build</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
:root{--bg:#0a0a0f;--fg:#e8e6e3;--accent:#6c63ff;--accent2:#00d4aa;--muted:#6b7280;--card:#12121a;--border:#1e1e2e;--hover:#1a1a2a}
body{font-family:system-ui,-apple-system,sans-serif;background:var(--bg);color:var(--fg);line-height:1.6;overflow-x:hidden}
a{color:var(--accent);text-decoration:none;cursor:pointer}
a:hover{text-decoration:underline}
code{font-family:'SF Mono',Consolas,monospace;font-size:0.9em;background:var(--card);padding:0.15em 0.4em;border-radius:4px}
pre{font-family:'SF Mono',Consolas,monospace;font-size:0.85rem;line-height:1.7;background:var(--card);border:1px solid var(--border);border-radius:12px;padding:1.25rem;overflow-x:auto;white-space:pre-wrap}

/* ── Nav ── */
nav{position:sticky;top:0;z-index:100;background:rgba(10,10,15,0.92);backdrop-filter:blur(12px);border-bottom:1px solid var(--border);padding:0 2rem;display:flex;align-items:center;height:52px;gap:2rem}
nav .logo{font-weight:800;font-size:1.1rem;letter-spacing:-0.02em}
nav .logo span{background:linear-gradient(135deg,var(--accent),var(--accent2));-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}
nav .links{display:flex;gap:1.5rem;font-size:0.9rem}
nav .links a{color:var(--muted);transition:color 0.15s}
nav .links a:hover,nav .links a.active{color:var(--fg);text-decoration:none}
nav .status{margin-left:auto;font-size:0.8rem;color:var(--muted)}
nav .status .dot{display:inline-block;width:7px;height:7px;border-radius:50%;margin-right:5px;vertical-align:middle}
nav .status .dot.ok{background:var(--accent2)}
nav .status .dot.loading{background:var(--muted);animation:pulse 1s infinite}
nav .status .dot.off{background:#f44}
@keyframes pulse{0%,100%{opacity:1}50%{opacity:0.3}}

/* ── Layout ── */
main{max-width:1100px;margin:0 auto;padding:2rem}
section{display:none}
section.active{display:block}

/* ── Hero ── */
.hero{min-height:70vh;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;position:relative}
.hero::before{content:'';position:absolute;inset:0;background:radial-gradient(ellipse at 50% 0%,rgba(108,99,255,0.12) 0%,transparent 60%);pointer-events:none}
.hero h1{font-size:clamp(2.5rem,6vw,4.5rem);font-weight:800;letter-spacing:-0.03em;margin-bottom:0.5rem}
.hero h1 span{background:linear-gradient(135deg,var(--accent),var(--accent2));-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}
.hero .sub{font-size:clamp(1.05rem,2.2vw,1.35rem);color:var(--muted);max-width:620px;margin:0 auto 2rem}
.pill{display:inline-block;padding:0.3rem 0.9rem;border-radius:999px;font-size:0.82rem;border:1px solid var(--border);color:var(--muted);margin-bottom:1.5rem}
.cta{display:inline-flex;gap:0.8rem;flex-wrap:wrap;justify-content:center}
.btn{padding:0.65rem 1.5rem;border-radius:8px;font-weight:600;font-size:0.95rem;transition:all 0.2s;border:none;cursor:pointer}
.btn-primary{background:var(--accent);color:#fff}
.btn-primary:hover{background:#5a52e0;text-decoration:none}
.btn-outline{border:1px solid var(--border);color:var(--fg);background:transparent}
.btn-outline:hover{border-color:var(--accent);text-decoration:none}

/* ── Stats ── */
.stats{display:flex;gap:2rem;justify-content:center;flex-wrap:wrap;margin:2.5rem 0}
.stat{text-align:center}
.stat .num{font-size:2rem;font-weight:800;background:linear-gradient(135deg,var(--accent),var(--accent2));-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}
.stat .label{color:var(--muted);font-size:0.82rem;margin-top:0.2rem}

/* ── Cards ── */
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:1.25rem;margin-top:1.5rem}
.grid-sm{grid-template-columns:repeat(auto-fit,minmax(180px,1fr))}
.card{background:var(--card);border:1px solid var(--border);border-radius:12px;padding:1.5rem;transition:border-color 0.2s}
.card:hover{border-color:var(--accent)}
.card .icon{font-size:1.4rem;margin-bottom:0.6rem}
.card h3{font-size:1.05rem;margin-bottom:0.4rem}
.card p{color:var(--muted);font-size:0.9rem}

.section-title{text-align:center;font-size:clamp(1.4rem,3vw,2rem);font-weight:700;margin:3rem 0 0.4rem}
.section-sub{text-align:center;color:var(--muted);margin-bottom:1.5rem;font-size:1rem}

/* ── Traits browser ── */
.search-bar{display:flex;gap:0.75rem;margin-bottom:1.25rem;align-items:center}
.search-bar input{flex:1;background:var(--card);border:1px solid var(--border);border-radius:8px;padding:0.6rem 1rem;color:var(--fg);font-size:0.95rem;outline:none}
.search-bar input:focus{border-color:var(--accent)}
.search-bar .count{color:var(--muted);font-size:0.82rem;white-space:nowrap}
.trait-list{display:flex;flex-direction:column;gap:0.5rem;max-height:60vh;overflow-y:auto}
.trait-item{background:var(--card);border:1px solid var(--border);border-radius:8px;padding:0.75rem 1rem;cursor:pointer;transition:border-color 0.15s;display:flex;align-items:center;gap:1rem}
.trait-item:hover{border-color:var(--accent)}
.trait-item.selected{border-color:var(--accent2);background:rgba(0,212,170,0.04)}
.trait-item .path{font-family:'SF Mono',Consolas,monospace;font-size:0.9rem;color:var(--accent2);white-space:nowrap}
.trait-item .desc{color:var(--muted);font-size:0.85rem;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.trait-item .badge{font-size:0.72rem;padding:0.15rem 0.5rem;border-radius:999px;border:1px solid var(--border);color:var(--muted)}
.trait-item .badge.wasm{border-color:var(--accent2);color:var(--accent2)}

/* ── Detail panel ── */
.detail{margin-top:1.5rem;background:var(--card);border:1px solid var(--border);border-radius:12px;padding:1.5rem;display:none}
.detail.open{display:block}
.detail h2{font-size:1.3rem;font-weight:700;margin-bottom:0.25rem}
.detail .meta{color:var(--muted);font-size:0.85rem;margin-bottom:1rem}
.detail .params{margin:1rem 0}
.detail .param{display:flex;gap:0.75rem;align-items:baseline;margin-bottom:0.5rem;font-size:0.9rem}
.detail .param .name{font-family:'SF Mono',Consolas,monospace;color:var(--accent2);min-width:100px}
.detail .param .type{color:var(--accent);font-family:'SF Mono',Consolas,monospace;font-size:0.85rem}
.detail .param .pdesc{color:var(--muted)}

/* ── Call panel ── */
.call-panel{margin-top:1rem;display:flex;gap:0.75rem;align-items:flex-end;flex-wrap:wrap}
.call-panel input{flex:1;min-width:200px;background:var(--bg);border:1px solid var(--border);border-radius:8px;padding:0.6rem 1rem;color:var(--fg);font-family:'SF Mono',Consolas,monospace;font-size:0.9rem;outline:none}
.call-panel input:focus{border-color:var(--accent)}
.call-panel button{padding:0.6rem 1.5rem;background:var(--accent);color:#fff;border:none;border-radius:8px;font-weight:600;cursor:pointer;font-size:0.9rem}
.call-panel button:hover{background:#5a52e0}
.call-panel button:disabled{opacity:0.5;cursor:not-allowed}
.result{margin-top:1rem;display:none}
.result.show{display:block}
.result pre{max-height:400px;overflow-y:auto}
.result .timing{color:var(--muted);font-size:0.8rem;margin-top:0.5rem}

/* ── Footer ── */
footer{text-align:center;padding:3rem 2rem 2rem;color:var(--muted);font-size:0.85rem;border-top:1px solid var(--border);margin-top:4rem}
footer a{color:var(--accent)}

/* ── Responsive ── */
@media(max-width:640px){
    nav{padding:0 1rem;gap:1rem}
    main{padding:1rem}
    .hero{min-height:50vh}
    .grid{grid-template-columns:1fr}
}
</style>
</head>
<body>

<nav>
    <div class="logo"><span>traits</span>.build</div>
    <div class="links">
        <a href="#home" class="active" data-page="home">Home</a>
        <a href="#traits" data-page="traits">Traits</a>
        <a href="#try" data-page="try">Try It</a>
    </div>
    <div class="status">
        <span class="dot loading" id="statusDot"></span>
        <span id="statusText">loading…</span>
    </div>
</nav>

<!-- ══════════ HOME ══════════ -->
<main>
<section id="page-home" class="active">
    <div class="hero">
        <div class="pill">WASM kernel · no server · runs in your browser</div>
        <h1><span>traits</span>.build</h1>
        <p class="sub">
            Typed, composable functions compiled into a single Rust binary — and a
            186 KB WebAssembly kernel that runs right here, right now.
        </p>
        <div class="cta">
            <a class="btn btn-primary" href="#traits" data-page="traits">Browse Traits</a>
            <a class="btn btn-outline" href="#try" data-page="try">Try a Call</a>
            <a class="btn btn-outline" href="https://github.com/kilian-ai/traits.build" target="_blank">GitHub</a>
        </div>
    </div>

    <div class="stats" id="stats">
        <div class="stat"><div class="num" id="statTraits">–</div><div class="label">traits registered</div></div>
        <div class="stat"><div class="num" id="statCallable">–</div><div class="label">WASM-callable</div></div>
        <div class="stat"><div class="num" id="statNs">–</div><div class="label">namespaces</div></div>
        <div class="stat"><div class="num" id="statVersion">–</div><div class="label">kernel version</div></div>
    </div>

    <h2 class="section-title">Architecture</h2>
    <p class="section-sub">The kernel is traits all the way down</p>
    <div class="grid grid-sm">
        <div class="card"><div class="icon">⚙️</div><h3>Dispatcher</h3><p>Resolves paths, validates args, dispatches to compiled Rust functions.</p></div>
        <div class="card"><div class="icon">📦</div><h3>Registry</h3><p>Concurrent DashMap of trait definitions. Hot-reloadable.</p></div>
        <div class="card"><div class="icon">🔗</div><h3>Interfaces</h3><p>provides / requires / bindings — typed dependency wiring.</p></div>
        <div class="card"><div class="icon">🌐</div><h3>WASM Kernel</h3><p>Same Rust traits compiled to WebAssembly. Runs in any browser.</p></div>
        <div class="card"><div class="icon">🔌</div><h3>Plugin Loader</h3><p>cdylib .dylib plugins discovered and loaded at runtime.</p></div>
        <div class="card"><div class="icon">🛡️</div><h3>Type System</h3><p>int, float, string, bool, bytes, any, list&lt;T&gt;, map&lt;K,V&gt;, T?</p></div>
    </div>

    <h2 class="section-title">Three Surfaces, One Trait</h2>
    <p class="section-sub">Every trait is callable via CLI, REST API, and MCP — plus WASM in the browser</p>
    <div class="grid">
        <div class="card"><div class="icon">🖥️</div><h3>CLI</h3><p><code>traits checksum hash "hello"</code> — every sys.* trait is a direct subcommand.</p></div>
        <div class="card"><div class="icon">🌍</div><h3>REST API</h3><p><code>POST /traits/sys/checksum</code> with JSON body. SSE streaming supported.</p></div>
        <div class="card"><div class="icon">🤖</div><h3>MCP Protocol</h3><p>JSON-RPC 2.0 over stdio. AI agents discover and call traits as tools.</p></div>
    </div>

    <h2 class="section-title" id="home-traits-title">Built-in Traits</h2>
    <p class="section-sub" id="home-traits-sub">Loading from WASM kernel…</p>
    <div id="home-trait-list"></div>
</section>

<!-- ══════════ TRAITS ══════════ -->
<section id="page-traits">
    <h2 class="section-title" style="margin-top:1rem">Trait Registry</h2>
    <p class="section-sub">Browse all traits compiled into the WASM kernel</p>

    <div class="search-bar">
        <input type="text" id="traitSearch" placeholder="Search by name or description…" autocomplete="off">
        <span class="count" id="traitCount">–</span>
    </div>
    <div class="trait-list" id="traitList"></div>
    <div class="detail" id="traitDetail"></div>
</section>

<!-- ══════════ TRY IT ══════════ -->
<section id="page-try">
    <h2 class="section-title" style="margin-top:1rem">Try a Trait</h2>
    <p class="section-sub">Call WASM-callable traits directly in your browser — no server needed</p>

    <div class="grid" id="callableGrid" style="margin-bottom:2rem"></div>

    <div style="background:var(--card);border:1px solid var(--border);border-radius:12px;padding:1.5rem">
        <h3 style="margin-bottom:1rem">Call a trait</h3>
        <div class="call-panel">
            <input type="text" id="callPath" placeholder="sys.checksum" list="callableList">
            <datalist id="callableList"></datalist>
            <input type="text" id="callArgs" placeholder='["hash", "hello world"]'>
            <button id="callBtn" disabled>Call</button>
        </div>
        <div class="result" id="callResult">
            <pre id="callOutput"></pre>
            <div class="timing" id="callTiming"></div>
        </div>
    </div>

    <h2 class="section-title">Examples</h2>
    <p class="section-sub">Click to run</p>
    <div class="grid" id="examples"></div>
</section>
</main>

<footer>
    <p>
        traits.build — WASM kernel running 100% in your browser.
        <a href="https://github.com/kilian-ai/traits.build">GitHub</a>
    </p>
    <p style="margin-top:0.5rem;font-size:0.8rem">No server connection required. All trait calls execute locally via WebAssembly.</p>
</footer>

<script type="module">
// ── SDK (inlined for zero-dependency static hosting) ──

let wasm = null, wasmReady = false, wasmCallableSet = new Set();
let allTraits = [], wasmInfo = null;

async function loadWasm(wasmUrl, jsUrl) {
    const mod = await import(jsUrl);
    await mod.default(wasmUrl);
    const result = JSON.parse(mod.init());
    JSON.parse(mod.callable_traits()).forEach(p => wasmCallableSet.add(p));
    wasm = mod;
    wasmReady = true;
    return result;
}

function callTrait(path, args = []) {
    const t0 = performance.now();
    const raw = wasm.call(path, JSON.stringify(args));
    const ms = Math.round((performance.now() - t0) * 10) / 10;
    return { ok: true, result: JSON.parse(raw), ms, dispatch: 'wasm' };
}

// ── Helpers ──
const $ = s => document.querySelector(s);
const $$ = s => [...document.querySelectorAll(s)];

// ── Router ──
function showPage(name) {
    $$('section').forEach(s => s.classList.remove('active'));
    const page = $(`#page-${name}`);
    if (page) page.classList.add('active');
    $$('nav .links a').forEach(a => {
        a.classList.toggle('active', a.dataset.page === name);
    });
}

$$('nav .links a').forEach(a => {
    a.addEventListener('click', e => {
        e.preventDefault();
        const page = a.dataset.page;
        history.pushState({ page }, '', `#${page}`);
        showPage(page);
    });
});

// Also handle .btn links with data-page
document.addEventListener('click', e => {
    const a = e.target.closest('[data-page]');
    if (!a) return;
    e.preventDefault();
    const page = a.dataset.page;
    history.pushState({ page }, '', `#${page}`);
    showPage(page);
});

window.addEventListener('popstate', e => {
    showPage(e.state?.page || pageFromHash());
});

function pageFromHash() {
    const h = location.hash.replace('#', '');
    return ['home', 'traits', 'try'].includes(h) ? h : 'home';
}

// ── WASM Init ──
const dot = $('#statusDot');
const statusText = $('#statusText');

async function boot() {
    try {
        // Determine WASM file locations (relative to this page)
        const base = detectBase();
        wasmInfo = await loadWasm(base + 'traits_wasm_bg.wasm', base + 'traits_wasm.js');

        dot.className = 'dot ok';
        statusText.textContent = `${wasmInfo.traits_registered} traits · WASM`;

        // Load all traits from registry
        allTraits = JSON.parse(wasm.list_traits());

        populateStats();
        populateHome();
        populateTraitBrowser();
        populateTryPage();

    } catch (e) {
        console.error('WASM init failed:', e);
        dot.className = 'dot off';
        statusText.textContent = 'WASM unavailable';
    }
}

function detectBase() {
    // Try multiple locations for WASM files
    // 1. Same directory as the page
    // 2. /wasm/ path (when served by traits server)
    // 3. Relative ../wasm/pkg/ path
    // We'll try /wasm/ first (server), then same-dir (static hosting)
    if (location.protocol === 'file:') {
        return './wasm/';
    }
    return '/wasm/';
}

// ── Populate Stats ──
function populateStats() {
    const namespaces = new Set(allTraits.map(t => t.path.split('.')[0]));
    $('#statTraits').textContent = allTraits.length;
    $('#statCallable').textContent = wasmCallableSet.size;
    $('#statNs').textContent = namespaces.size;
    $('#statVersion').textContent = wasmInfo?.version || '–';
}

// ── Home: trait table ──
function populateHome() {
    $('#home-traits-sub').textContent =
        `${allTraits.length} traits across ${new Set(allTraits.map(t=>t.path.split('.')[0])).size} namespaces`;

    const groups = {};
    for (const t of allTraits) {
        const ns = t.path.split('.').slice(0, -1).join('.') || t.path;
        (groups[ns] ||= []).push(t);
    }

    let html = '';
    for (const [ns, traits] of Object.entries(groups).sort((a,b) => a[0].localeCompare(b[0]))) {
        html += `<h3 style="color:var(--accent2);margin:1.5rem 0 0.5rem;font-size:0.95rem">${esc(ns)} <span style="color:var(--muted);font-weight:400">(${traits.length})</span></h3>`;
        html += '<div class="grid grid-sm">';
        for (const t of traits) {
            const name = t.path.split('.').pop();
            const desc = t.description || '';
            const badge = wasmCallableSet.has(t.path) ? '<span class="badge wasm">wasm</span>' : '';
            html += `<div class="card" style="padding:1rem;cursor:pointer" data-trait-path="${esc(t.path)}">
                <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:0.3rem">
                    <code style="color:var(--accent2);background:none;padding:0">${esc(name)}</code>
                    ${badge}
                </div>
                <p style="font-size:0.82rem">${esc(desc)}</p>
            </div>`;
        }
        html += '</div>';
    }
    $('#home-trait-list').innerHTML = html;

    // Click to navigate to trait detail
    $('#home-trait-list').addEventListener('click', e => {
        const card = e.target.closest('[data-trait-path]');
        if (!card) return;
        selectedTrait = card.dataset.traitPath;
        history.pushState({ page: 'traits' }, '', '#traits');
        showPage('traits');
        $('#traitSearch').value = selectedTrait;
        filterTraits();
        showDetail(selectedTrait);
    });
}

// ── Traits Browser ──
let selectedTrait = null;

function populateTraitBrowser() {
    renderTraitList(allTraits);
    $('#traitSearch').addEventListener('input', filterTraits);
}

function filterTraits() {
    const q = $('#traitSearch').value.toLowerCase();
    const filtered = q
        ? allTraits.filter(t => t.path.toLowerCase().includes(q) || (t.description||'').toLowerCase().includes(q))
        : allTraits;
    renderTraitList(filtered);
}

function renderTraitList(traits) {
    $('#traitCount').textContent = `${traits.length} traits`;
    let html = '';
    for (const t of traits) {
        const sel = t.path === selectedTrait ? ' selected' : '';
        const badge = wasmCallableSet.has(t.path) ? '<span class="badge wasm">wasm</span>' : '<span class="badge">server</span>';
        html += `<div class="trait-item${sel}" data-path="${esc(t.path)}">
            <span class="path">${esc(t.path)}</span>
            <span class="desc">${esc(t.description || '')}</span>
            ${badge}
        </div>`;
    }
    $('#traitList').innerHTML = html;

    // Click handlers
    $$('#traitList .trait-item').forEach(el => {
        el.addEventListener('click', () => {
            selectedTrait = el.dataset.path;
            $$('#traitList .trait-item').forEach(x => x.classList.remove('selected'));
            el.classList.add('selected');
            showDetail(el.dataset.path);
        });
    });
}

function showDetail(path) {
    const info = wasm.get_trait_info(path);
    if (!info) { $('#traitDetail').classList.remove('open'); return; }

    const t = JSON.parse(info);
    const params = t.params || t.signature?.params || [];
    const ret = t.returns || t.signature?.returns;
    const callable = wasmCallableSet.has(path);

    let html = `<h2>${esc(t.path)}</h2>`;
    html += `<div class="meta">${esc(t.version || '')} · ${callable ? '🟢 WASM-callable' : '🔵 server-only'}${t.description ? ' · ' + esc(t.description) : ''}</div>`;

    if (params.length) {
        html += '<div class="params"><strong>Parameters:</strong>';
        for (const p of params) {
            const req = p.required ? ' <span style="color:#f44">*</span>' : '';
            html += `<div class="param">
                <span class="name">${esc(p.name)}${req}</span>
                <span class="type">${esc(p.type || 'any')}</span>
                <span class="pdesc">${esc(p.description || '')}</span>
            </div>`;
        }
        html += '</div>';
    }

    if (ret) {
        const rtype = typeof ret === 'string' ? ret : ret.type;
        const rdesc = typeof ret === 'string' ? '' : (ret.description || '');
        html += `<p style="margin-top:0.75rem"><strong>Returns:</strong> <code>${esc(rtype)}</code> ${rdesc ? '— ' + esc(rdesc) : ''}</p>`;
    }

    if (callable) {
        html += `<div class="call-panel" style="margin-top:1rem">
            <input type="text" id="detailArgs" placeholder='["hash", "hello"]' style="flex:1">
            <button onclick="window._callFromDetail('${esc(path)}')">Call</button>
        </div>
        <div class="result" id="detailResult"><pre id="detailOutput"></pre><div class="timing" id="detailTiming"></div></div>`;
    }

    const detail = $('#traitDetail');
    detail.innerHTML = html;
    detail.classList.add('open');
}

window._callFromDetail = function(path) {
    const argsRaw = $('#detailArgs')?.value || '[]';
    let args;
    try { args = JSON.parse(argsRaw); } catch { args = [argsRaw]; }
    if (!Array.isArray(args)) args = [args];

    const res = callTrait(path, args);
    const out = typeof res.result === 'string' ? res.result : JSON.stringify(res.result, null, 2);
    const resultEl = $('#detailResult');
    $('#detailOutput').textContent = out.length > 5000 ? out.slice(0, 5000) + '\n… (truncated)' : out;
    $('#detailTiming').textContent = `${res.ms}ms · wasm`;
    resultEl.classList.add('show');
};

// ── Try It page ──
function populateTryPage() {
    const callable = [...wasmCallableSet];

    // Callable trait cards
    let cardsHtml = '';
    for (const path of callable) {
        const t = allTraits.find(x => x.path === path);
        cardsHtml += `<div class="card" style="padding:1rem;cursor:pointer" data-try="${esc(path)}">
            <code style="color:var(--accent2);background:none;padding:0;font-size:0.95rem">${esc(path)}</code>
            <p style="font-size:0.82rem;margin-top:0.3rem">${esc(t?.description || '')}</p>
        </div>`;
    }
    $('#callableGrid').innerHTML = cardsHtml;

    // Datalist for autocomplete
    let opts = '';
    for (const p of callable) opts += `<option value="${esc(p)}">`;
    $('#callableList').innerHTML = opts;

    // Enable call button
    $('#callBtn').disabled = false;
    $('#callBtn').addEventListener('click', doCall);
    $('#callArgs').addEventListener('keydown', e => { if (e.key === 'Enter') doCall(); });
    $('#callPath').addEventListener('keydown', e => { if (e.key === 'Enter') { $('#callArgs').focus(); } });

    // Click a callable card → fill call form
    $$('#callableGrid [data-try]').forEach(el => {
        el.addEventListener('click', () => {
            $('#callPath').value = el.dataset.try;
            $('#callArgs').focus();
        });
    });

    // Examples
    const examples = [
        { path: 'sys.checksum', args: ['hash', 'hello world'], label: 'Hash a string' },
        { path: 'sys.checksum', args: ['hash', 'traits.build'], label: 'Hash "traits.build"' },
        { path: 'sys.version', args: ['date'], label: 'Today\'s version stamp' },
        { path: 'sys.version', args: ['hhmmss'], label: 'Version with time' },
        { path: 'sys.list', args: [], label: 'List all traits' },
        { path: 'sys.registry', args: ['count'], label: 'Count registered traits' },
        { path: 'sys.registry', args: ['namespaces'], label: 'List namespaces' },
        { path: 'sys.registry', args: ['search', 'checksum'], label: 'Search traits' },
        { path: 'sys.info', args: ['sys.checksum'], label: 'Get trait info' },
        { path: 'kernel.types', args: ['parse', 'list<map<string,int>>'], label: 'Parse a complex type' },
    ];

    let exHtml = '';
    for (const ex of examples) {
        exHtml += `<div class="card" style="padding:1rem;cursor:pointer" data-ex-path="${esc(ex.path)}" data-ex-args='${esc(JSON.stringify(ex.args))}'>
            <div style="font-weight:600;font-size:0.95rem;margin-bottom:0.3rem">${esc(ex.label)}</div>
            <code style="font-size:0.82rem;color:var(--muted)">${esc(ex.path)} ${ex.args.map(a => JSON.stringify(a)).join(' ')}</code>
        </div>`;
    }
    $('#examples').innerHTML = exHtml;

    $$('#examples [data-ex-path]').forEach(el => {
        el.addEventListener('click', () => {
            $('#callPath').value = el.dataset.exPath;
            $('#callArgs').value = el.dataset.exArgs;
            doCall();
        });
    });
}

function doCall() {
    const path = $('#callPath').value.trim();
    if (!path) return;

    const argsRaw = $('#callArgs').value.trim() || '[]';
    let args;
    try { args = JSON.parse(argsRaw); } catch { args = [argsRaw]; }
    if (!Array.isArray(args)) args = [args];

    if (!wasmCallableSet.has(path)) {
        $('#callOutput').textContent = `"${path}" is not WASM-callable.\nAvailable: ${[...wasmCallableSet].join(', ')}`;
        $('#callTiming').textContent = '';
        $('#callResult').classList.add('show');
        return;
    }

    try {
        const res = callTrait(path, args);
        const out = typeof res.result === 'string' ? res.result : JSON.stringify(res.result, null, 2);
        $('#callOutput').textContent = out.length > 10000 ? out.slice(0, 10000) + '\n… (truncated)' : out;
        $('#callTiming').textContent = `${res.ms}ms · wasm`;
    } catch (e) {
        $('#callOutput').textContent = `Error: ${e.message || e}`;
        $('#callTiming').textContent = '';
    }
    $('#callResult').classList.add('show');
}

// ── Escape HTML ──
function esc(s) {
    if (!s) return '';
    return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

// ── Boot ──
showPage(pageFromHash());
boot();
</script>
</body>
</html>"##;
