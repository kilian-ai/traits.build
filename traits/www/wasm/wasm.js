import { Traits } from '/static/www/sdk/traits.js';

const $ = id => document.getElementById(id);
const traits = new Traits();

let allTraits = [];
let currentTrait = null;

// ── Bootstrap ──

async function boot() {
    const status = $('status');
    try {
        const info = await traits.init();

        allTraits = await traits.list();

        // Also fetch the live server trait list to get accurate counts
        let serverTraitCount = 0;
        try {
            const resp = await fetch('/traits');
            if (resp.ok) {
                const tree = await resp.json();
                serverTraitCount = Object.values(tree).reduce((sum, ns) => sum + ns.length, 0);
            }
        } catch { /* server unreachable — WASM-only mode */ }

        const total = serverTraitCount || info.traits;
        status.textContent = `Kernel ready — ${total} traits (${info.callable} local WASM, ${total - info.callable} via REST)`;
        status.classList.add('ok');

        renderKernelInfo({ ...info, server_traits: total });
        renderTraitList();
        await bindEvents();
    } catch (e) {
        status.textContent = `Failed to load: ${e.message || e}`;
        status.classList.add('error');
        console.error(e);
    }
}

function renderKernelInfo(info) {
    const el = $('kernelInfo');
    el.innerHTML = `<table>
        <tr><td>Version</td><td>${info.version || '—'}</td></tr>
        <tr><td>Total traits</td><td>${info.server_traits || info.traits}</td></tr>
        <tr><td>WASM (local)</td><td>${info.callable}</td></tr>
        <tr><td>REST (server)</td><td>${(info.server_traits || info.traits) - info.callable}</td></tr>
        <tr><td>Runtime</td><td>wasm32 + REST hybrid</td></tr>
    </table>`;
}

// ── Trait list ──

function renderTraitList() {
    const search = $('traitSearch').value.toLowerCase();
    const callableOnly = $('filterCallable').checked;

    const filtered = allTraits.filter(t => {
        if (callableOnly && !t.wasm_callable) return false;
        if (search && !t.path.toLowerCase().includes(search) && !t.description.toLowerCase().includes(search)) return false;
        return true;
    });

    const list = $('traitList');
    list.innerHTML = filtered.map(t => `
        <div class="trait-item${currentTrait && currentTrait.path === t.path ? ' active' : ''}"
             data-path="${t.path}">
            <div class="path">${t.path}</div>
            <div class="desc">${t.description || ''}</div>
            <div class="badges">
                ${t.wasm_callable ? '<span class="badge callable">WASM</span>' : '<span class="badge server">Server</span>'}
            </div>
        </div>
    `).join('');

    $('traitCount').textContent = `${filtered.length} / ${allTraits.length}`;
}

async function selectTrait(path) {
    currentTrait = await traits.info(path);
    if (!currentTrait) return;
    $('welcomePanel').hidden = true;
    $('traitDetail').hidden = false;

    $('detailName').textContent = currentTrait.path;
    $('detailVersion').textContent = currentTrait.version || '';
    $('detailDesc').textContent = currentTrait.description || '';

    const callableEl = $('detailCallable');
    if (currentTrait.wasm_callable) {
        callableEl.textContent = 'WASM';
        callableEl.className = 'badge callable';
    } else {
        callableEl.textContent = 'Server';
        callableEl.className = 'badge server';
    }

    // Params
    const paramsEl = $('detailParams');
    const params = currentTrait.params || [];
    if (params.length > 0) {
        paramsEl.innerHTML = '<h3>Parameters</h3>' + params.map(p => `
            <div class="param-row">
                <span class="param-name">${p.name}</span>
                <span class="param-type">${p.type}</span>
                <span class="param-desc">${p.description || ''}</span>
                ${p.required ? '<span class="param-required">required</span>' : ''}
            </div>
        `).join('');
    } else {
        paramsEl.innerHTML = '<h3>Parameters</h3><div class="param-row" style="color:#8b949e">No parameters</div>';
    }

    // Call section — available for ALL traits (WASM local or REST remote)
    const callEl = $('callSection');
    callEl.hidden = false;
    const form = $('argsForm');
    if (params.length > 0) {
        form.innerHTML = params.map(p => `
            <div class="arg-field">
                <label for="arg_${p.name}">${p.name} <span class="param-type">(${p.type})</span></label>
                <input id="arg_${p.name}" type="text" placeholder="${p.description || p.name}" data-name="${p.name}" data-type="${p.type}" />
            </div>
        `).join('');
    } else {
        form.innerHTML = '<div style="color:#8b949e;font-size:0.85rem">No arguments needed</div>';
    }

    $('resultSection').hidden = true;
    renderTraitList(); // Update active state
}

// ── Call trait ──

function collectArgs() {
    const params = currentTrait.params || [];
    const args = [];
    for (const p of params) {
        const input = document.querySelector(`#arg_${p.name}`);
        const raw = input ? input.value : '';
        try { args.push(JSON.parse(raw)); }
        catch { args.push(raw); }
    }
    return args;
}

async function callTrait() {
    if (!currentTrait) return;

    const args = collectArgs();
    const mode = traits.dispatchMode(currentTrait.path);
    const tag = mode === 'wasm' ? 'WASM' : 'REST';

    $('resultSection').hidden = false;
    $('resultOutput').textContent = `Calling via ${tag}...`;
    $('elapsed').textContent = '';

    const res = await traits.call(currentTrait.path, args);
    $('elapsed').textContent = `${res.ms || 0}ms (${res.dispatch || tag})`;
    if (res.ok) {
        const formatted = typeof res.result === 'string' ? res.result : JSON.stringify(res.result, null, 2);
        $('resultOutput').textContent = formatted;
    } else {
        $('resultOutput').textContent = `Error: ${res.error}`;
    }
}

// ── Events ──

async function bindEvents() {
    $('traitSearch').addEventListener('input', renderTraitList);
    $('filterCallable').addEventListener('change', renderTraitList);

    $('traitList').addEventListener('click', e => {
        const item = e.target.closest('.trait-item');
        if (item) selectTrait(item.dataset.path);
    });

    $('btnCall').addEventListener('click', callTrait);

    $('btnCopy').addEventListener('click', () => {
        const text = $('resultOutput').textContent;
        navigator.clipboard.writeText(text).catch(() => {});
    });

    // Enter key in args form triggers call
    $('argsForm').addEventListener('keydown', e => {
        if (e.key === 'Enter') callTrait();
    });

    // Terminal
    await initTerminal();
}

// ═══════════════════════════════════════════
// ── Terminal (powered by shared terminal.js) ──
// ═══════════════════════════════════════════

async function initTerminal() {
    const { createTerminal } = await import('/static/www/terminal/terminal.js');
    await createTerminal($('xterm'), {
        header: document.querySelector('.terminal-header'),
        container: $('terminalContainer'),
        toggleBtn: $('btnToggleTerm'),
    });
}

// ── Go ──
boot();
