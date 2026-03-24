import init, { init as kernelInit, call, list_traits, get_trait_info, callable_traits } from '/wasm/traits_wasm.js';

const $ = id => document.getElementById(id);

let allTraits = [];
let currentTrait = null;

// ── Bootstrap ──

async function boot() {
    const status = $('status');
    try {
        await init('/wasm/traits_wasm_bg.wasm');
        const result = JSON.parse(kernelInit());
        status.textContent = `Kernel ready — ${result.traits_registered} traits, ${result.wasm_callable} callable in WASM`;
        status.classList.add('ok');

        allTraits = JSON.parse(list_traits());
        renderKernelInfo(result);
        renderTraitList();
        bindEvents();
    } catch (e) {
        status.textContent = `Failed to load WASM: ${e.message || e}`;
        status.classList.add('error');
        console.error(e);
    }
}

function renderKernelInfo(info) {
    const el = $('kernelInfo');
    el.innerHTML = `<table>
        <tr><td>Version</td><td>${info.version}</td></tr>
        <tr><td>Traits</td><td>${info.traits_registered}</td></tr>
        <tr><td>WASM callable</td><td>${info.wasm_callable}</td></tr>
        <tr><td>Runtime</td><td>wasm32 (in-browser)</td></tr>
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

function selectTrait(path) {
    const raw = get_trait_info(path);
    if (!raw) return;

    currentTrait = JSON.parse(raw);
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

    // Call section — only for WASM-callable traits
    const callEl = $('callSection');
    if (currentTrait.wasm_callable) {
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
    } else {
        callEl.hidden = true;
    }

    $('resultSection').hidden = true;
    renderTraitList(); // Update active state
}

// ── Call trait ──

function callTrait() {
    if (!currentTrait || !currentTrait.wasm_callable) return;

    const params = currentTrait.params || [];
    const args = [];
    for (const p of params) {
        const input = document.querySelector(`#arg_${p.name}`);
        const raw = input ? input.value : '';
        // Try to parse as JSON, fall back to string
        try {
            args.push(JSON.parse(raw));
        } catch {
            args.push(raw);
        }
    }

    const t0 = performance.now();
    try {
        const result = call(currentTrait.path, JSON.stringify(args));
        const dt = (performance.now() - t0).toFixed(1);
        $('elapsed').textContent = `${dt}ms`;
        $('resultSection').hidden = false;

        try {
            const parsed = JSON.parse(result);
            $('resultOutput').textContent = JSON.stringify(parsed, null, 2);
        } catch {
            $('resultOutput').textContent = result;
        }
    } catch (e) {
        const dt = (performance.now() - t0).toFixed(1);
        $('elapsed').textContent = `${dt}ms`;
        $('resultSection').hidden = false;
        $('resultOutput').textContent = `Error: ${e.message || e}`;
    }
}

// ── Events ──

function bindEvents() {
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
}

// ── Go ──
boot();
