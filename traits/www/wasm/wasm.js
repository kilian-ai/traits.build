import init, { init as kernelInit, call, list_traits, get_trait_info, callable_traits } from '/wasm/traits_wasm.js';

const $ = id => document.getElementById(id);

let allTraits = [];
let currentTrait = null;
let wasmCallableSet = new Set();

// ── Bootstrap ──

async function boot() {
    const status = $('status');
    try {
        await init('/wasm/traits_wasm_bg.wasm');
        const result = JSON.parse(kernelInit());

        // Build set of WASM-callable traits
        const callableList = JSON.parse(callable_traits());
        wasmCallableSet = new Set(callableList);

        // Load trait list from WASM registry (has all traits' metadata)
        allTraits = JSON.parse(list_traits());

        // Also fetch the live server trait list to get accurate counts
        let serverTraitCount = 0;
        try {
            const resp = await fetch('/traits');
            if (resp.ok) {
                const tree = await resp.json();
                serverTraitCount = Object.values(tree).reduce((sum, ns) => sum + ns.length, 0);
            }
        } catch { /* server unreachable — WASM-only mode */ }

        const total = serverTraitCount || result.traits_registered;
        status.textContent = `Kernel ready — ${total} traits (${result.wasm_callable} local WASM, ${total - result.wasm_callable} via REST)`;
        status.classList.add('ok');

        renderKernelInfo({ ...result, server_traits: total });
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
        <tr><td>Total traits</td><td>${info.server_traits || info.traits_registered}</td></tr>
        <tr><td>WASM (local)</td><td>${info.wasm_callable}</td></tr>
        <tr><td>REST (server)</td><td>${(info.server_traits || info.traits_registered) - info.wasm_callable}</td></tr>
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
    const isLocal = wasmCallableSet.has(currentTrait.path);
    const tag = isLocal ? 'WASM' : 'REST';

    $('resultSection').hidden = false;
    $('resultOutput').textContent = `Calling via ${tag}...`;
    $('elapsed').textContent = '';

    const t0 = performance.now();
    try {
        let result;
        if (isLocal) {
            // Direct WASM dispatch — synchronous, sub-millisecond
            result = JSON.parse(call(currentTrait.path, JSON.stringify(args)));
        } else {
            // REST dispatch — async, hits the server
            result = await callTraitRest(currentTrait.path, args);
        }
        const dt = (performance.now() - t0).toFixed(1);
        $('elapsed').textContent = `${dt}ms (${tag})`;
        $('resultOutput').textContent = typeof result === 'string' ? result : JSON.stringify(result, null, 2);
    } catch (e) {
        const dt = (performance.now() - t0).toFixed(1);
        $('elapsed').textContent = `${dt}ms (${tag})`;
        $('resultOutput').textContent = `Error: ${e.message || e}`;
    }
}

// ── REST API dispatch ──

async function callTraitRest(traitPath, args) {
    const url = `/traits/${traitPath.replace(/\./g, '/')}`;
    const resp = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args }),
    });
    const data = await resp.json();
    if (data.error) throw new Error(data.error);
    return data.result;
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

    // Terminal
    initTerminal();
}

// ═══════════════════════════════════════════
// ── Terminal Emulator ──
// ═══════════════════════════════════════════

const termHistory = [];
let termHistoryIdx = -1;

function initTerminal() {
    const header = document.querySelector('.terminal-header');
    const container = $('terminalContainer');
    const input = $('termInput');

    // Toggle collapse
    header.addEventListener('click', () => {
        container.classList.toggle('collapsed');
        const btn = $('btnToggleTerm');
        btn.textContent = container.classList.contains('collapsed') ? '▶ Terminal' : '▼ Terminal';
        if (!container.classList.contains('collapsed')) input.focus();
    });

    // Command input
    input.addEventListener('keydown', async (e) => {
        if (e.key === 'Enter') {
            const cmd = input.value.trim();
            input.value = '';
            if (cmd) {
                termHistory.push(cmd);
                termHistoryIdx = termHistory.length;
                termWrite(`<span class="line-dim">traits </span><span class="line-cmd">${escHtml(cmd)}</span>`, 'line');
                await termExec(cmd);
            }
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            if (termHistoryIdx > 0) {
                termHistoryIdx--;
                input.value = termHistory[termHistoryIdx];
            }
        } else if (e.key === 'ArrowDown') {
            e.preventDefault();
            if (termHistoryIdx < termHistory.length - 1) {
                termHistoryIdx++;
                input.value = termHistory[termHistoryIdx];
            } else {
                termHistoryIdx = termHistory.length;
                input.value = '';
            }
        } else if (e.key === 'Tab') {
            e.preventDefault();
            termTabComplete(input);
        } else if (e.key === 'l' && e.ctrlKey) {
            e.preventDefault();
            $('termOutput').innerHTML = '';
        }
    });

    // Welcome message
    termWrite('traits.build terminal — WASM + REST hybrid', 'line-accent');
    termWrite(`${allTraits.length} traits loaded. Type "help" for commands.`, 'line-info');
    termWrite('', 'line');
}

function termWrite(text, cls = 'line') {
    const output = $('termOutput');
    const el = document.createElement('span');
    el.className = `line ${cls}`;
    el.innerHTML = text;
    output.appendChild(el);
    // Auto-scroll
    const container = $('terminalContainer');
    container.scrollTop = container.scrollHeight;
}

function escHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// ── Tab completion ──

function termTabComplete(input) {
    const val = input.value;
    const parts = val.split(/\s+/);
    
    // Complete trait paths (works for: call <path>, info <path>, or just <path>)
    let prefix;
    if (parts.length === 1) {
        // Could be a command or a trait path
        prefix = parts[0];
    } else if (parts.length === 2 && ['call', 'info', 'c', 'i'].includes(parts[0])) {
        prefix = parts[1];
    } else {
        return;
    }

    const matches = allTraits
        .map(t => t.path)
        .filter(p => p.startsWith(prefix));

    if (matches.length === 1) {
        if (parts.length === 1) {
            input.value = matches[0];
        } else {
            parts[parts.length - 1] = matches[0];
            input.value = parts.join(' ');
        }
    } else if (matches.length > 1 && matches.length <= 30) {
        // Show matches
        termWrite(matches.join('  '), 'line-info');
        // Complete common prefix
        let common = matches[0];
        for (const m of matches) {
            while (!m.startsWith(common)) common = common.slice(0, -1);
        }
        if (common.length > prefix.length) {
            if (parts.length === 1) {
                input.value = common;
            } else {
                parts[parts.length - 1] = common;
                input.value = parts.join(' ');
            }
        }
    }
}

// ── Command dispatch ──

async function termExec(cmd) {
    const parts = parseCommand(cmd);
    const command = parts[0];
    const args = parts.slice(1);

    try {
        switch (command) {
            case 'help':
            case 'h':
            case '?':
                termShowHelp();
                break;

            case 'list':
            case 'ls':
                termList(args[0]);
                break;

            case 'info':
            case 'i':
                if (!args[0]) { termWrite('Usage: info <trait_path>', 'line-err'); break; }
                termInfo(args[0]);
                break;

            case 'call':
            case 'c':
                if (!args[0]) { termWrite('Usage: call <trait_path> [args...]', 'line-err'); break; }
                await termCall(args[0], args.slice(1));
                break;

            case 'search':
            case 's':
                termSearch(args.join(' '));
                break;

            case 'version':
            case 'v':
                termWrite(`v${$('status').textContent}`, 'line-ok');
                break;

            case 'clear':
            case 'cls':
                $('termOutput').innerHTML = '';
                break;

            case 'history':
                termHistory.forEach((h, i) => termWrite(`  ${i + 1}  ${escHtml(h)}`, 'line'));
                break;

            default:
                // Try as a trait path directly: "sys.checksum hash hello"
                if (allTraits.some(t => t.path === command)) {
                    await termCall(command, args);
                } else {
                    termWrite(`Unknown command: ${escHtml(command)}. Type "help" for usage.`, 'line-err');
                }
        }
    } catch (e) {
        termWrite(`Error: ${escHtml(e.message || String(e))}`, 'line-err');
    }
}

function parseCommand(cmd) {
    // Simple shell-like parsing: respects double quotes
    const parts = [];
    let current = '';
    let inQuote = false;
    for (const ch of cmd) {
        if (ch === '"') {
            inQuote = !inQuote;
        } else if (ch === ' ' && !inQuote) {
            if (current) { parts.push(current); current = ''; }
        } else {
            current += ch;
        }
    }
    if (current) parts.push(current);
    return parts;
}

// ── Terminal commands ──

function termShowHelp() {
    const lines = [
        ['<span class="line-bold">Commands:</span>', ''],
        ['  list [namespace]', 'List traits (optionally filter by namespace)'],
        ['  info &lt;path&gt;', 'Show trait details and parameters'],
        ['  call &lt;path&gt; [args...]', 'Call a trait (WASM locally, REST otherwise)'],
        ['  search &lt;query&gt;', 'Search traits by name or description'],
        ['  &lt;path&gt; [args...]', 'Shorthand: call a trait directly by path'],
        ['  history', 'Show command history'],
        ['  clear', 'Clear the terminal'],
        ['  help', 'Show this help'],
        ['', ''],
        ['<span class="line-bold">Shortcuts:</span>', ''],
        ['  Tab', 'Auto-complete trait paths'],
        ['  ↑ / ↓', 'Navigate command history'],
        ['  Ctrl+L', 'Clear terminal'],
        ['', ''],
        ['<span class="line-bold">Examples:</span>', ''],
        ['  call sys.checksum hash "hello world"', ''],
        ['  sys.version', ''],
        ['  info sys.list', ''],
        ['  list sys', ''],
        ['  search checksum', ''],
    ];
    for (const [left, right] of lines) {
        if (right) {
            termWrite(`${left}  <span class="line-dim">${right}</span>`, '');
        } else {
            termWrite(left, '');
        }
    }
}

function termList(namespace) {
    let filtered = allTraits;
    if (namespace) {
        filtered = allTraits.filter(t => t.path.startsWith(namespace));
        if (filtered.length === 0) {
            termWrite(`No traits in namespace "${escHtml(namespace)}"`, 'line-warn');
            return;
        }
    }

    // Group by namespace
    const groups = {};
    for (const t of filtered) {
        const ns = t.path.split('.').slice(0, -1).join('.');
        (groups[ns] || (groups[ns] = [])).push(t);
    }

    for (const [ns, traits] of Object.entries(groups).sort((a, b) => a[0].localeCompare(b[0]))) {
        termWrite(`<span class="line-bold">${escHtml(ns)}</span> <span class="line-dim">(${traits.length})</span>`, '');
        for (const t of traits) {
            const name = t.path.split('.').pop();
            const badge = t.wasm_callable
                ? '<span class="line-ok">[WASM]</span>'
                : '<span class="line-warn">[REST]</span>';
            termWrite(`  ${badge} <span class="line-accent">${escHtml(name)}</span>  <span class="line-dim">${escHtml(t.description || '')}</span>`, '');
        }
    }
    termWrite(`<span class="line-dim">${filtered.length} traits</span>`, '');
}

function termInfo(path) {
    const raw = get_trait_info(path);
    if (!raw) {
        termWrite(`Trait "${escHtml(path)}" not found`, 'line-err');
        return;
    }
    const t = JSON.parse(raw);
    const badge = t.wasm_callable ? '<span class="line-ok">WASM</span>' : '<span class="line-warn">REST</span>';
    termWrite(`<span class="line-bold">${escHtml(t.path)}</span>  ${badge}  <span class="line-dim">${escHtml(t.version || '')}</span>`, '');
    if (t.description) termWrite(`  ${escHtml(t.description)}`, 'line-info');

    const params = t.params || [];
    if (params.length > 0) {
        termWrite('', '');
        termWrite('<span class="line-bold">Parameters:</span>', '');
        for (const p of params) {
            const req = p.required ? ' <span class="line-err">*</span>' : '';
            termWrite(`  <span class="line-accent">${escHtml(p.name)}</span> <span class="line-dim">(${escHtml(p.type)})</span>${req}  ${escHtml(p.description || '')}`, '');
        }
    }
    if (t.returns) {
        termWrite('', '');
        termWrite(`<span class="line-bold">Returns:</span> <span class="line-dim">${escHtml(t.returns)}</span>  ${escHtml(t.returns_description || '')}`, '');
    }
}

async function termCall(path, argStrs) {
    const isLocal = wasmCallableSet.has(path);
    const tag = isLocal ? 'WASM' : 'REST';

    // Parse args: try JSON, fall back to string
    const args = argStrs.map(a => {
        try { return JSON.parse(a); }
        catch { return a; }
    });

    const t0 = performance.now();
    let result;
    if (isLocal) {
        result = JSON.parse(call(path, JSON.stringify(args)));
    } else {
        result = await callTraitRest(path, args);
    }
    const dt = (performance.now() - t0).toFixed(1);

    // Format output
    const formatted = typeof result === 'string' ? result : JSON.stringify(result, null, 2);
    // Truncate very long outputs
    const lines = formatted.split('\n');
    if (lines.length > 100) {
        termWrite(escHtml(lines.slice(0, 80).join('\n')), 'line-ok');
        termWrite(`<span class="line-dim">... (${lines.length - 80} more lines, ${formatted.length} bytes total)</span>`, '');
    } else {
        termWrite(escHtml(formatted), 'line-ok');
    }
    termWrite(`<span class="line-dim">${dt}ms (${tag})</span>`, '');
}

function termSearch(query) {
    if (!query) { termWrite('Usage: search <query>', 'line-err'); return; }
    const q = query.toLowerCase();
    const results = allTraits.filter(t =>
        t.path.toLowerCase().includes(q) ||
        (t.description || '').toLowerCase().includes(q)
    );
    if (results.length === 0) {
        termWrite(`No matches for "${escHtml(query)}"`, 'line-warn');
        return;
    }
    for (const t of results) {
        const badge = t.wasm_callable
            ? '<span class="line-ok">[WASM]</span>'
            : '<span class="line-warn">[REST]</span>';
        termWrite(`${badge} <span class="line-accent">${escHtml(t.path)}</span>  <span class="line-dim">${escHtml(t.description || '')}</span>`, '');
    }
    termWrite(`<span class="line-dim">${results.length} matches</span>`, '');
}

// ── Go ──
boot();
