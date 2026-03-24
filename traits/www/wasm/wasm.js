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
// ── xterm.js Terminal ──
// ═══════════════════════════════════════════

import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';

// ANSI escape helpers
const C = {
    reset: '\x1b[0m',
    bold:  '\x1b[1m',
    dim:   '\x1b[2m',
    green: '\x1b[32m',
    red:   '\x1b[31m',
    yellow:'\x1b[33m',
    blue:  '\x1b[34m',
    magenta:'\x1b[35m',
    cyan:  '\x1b[36m',
    white: '\x1b[37m',
    gray:  '\x1b[90m',
    brightWhite: '\x1b[97m',
};
const PROMPT = `${C.green}traits ${C.reset}`;
const PROMPT_LEN = 7; // visible chars: "traits "

let term = null;
let fitAddon = null;
let lineBuffer = '';
let cursorPos = 0;
const termHistory = [];
let termHistoryIdx = -1;
let busy = false;

function initTerminal() {
    const header = document.querySelector('.terminal-header');
    const container = $('terminalContainer');

    term = new Terminal({
        cursorBlink: true,
        cursorStyle: 'bar',
        fontSize: 13,
        fontFamily: "'SF Mono', 'Fira Code', 'Cascadia Code', 'Menlo', monospace",
        lineHeight: 1.3,
        scrollback: 5000,
        theme: {
            background: '#0d1117',
            foreground: '#c9d1d9',
            cursor: '#58a6ff',
            cursorAccent: '#0d1117',
            selectionBackground: '#264f78',
            selectionForeground: '#ffffff',
            black: '#484f58',
            red: '#f85149',
            green: '#3fb950',
            yellow: '#d29922',
            blue: '#58a6ff',
            magenta: '#bc8cff',
            cyan: '#76e3ea',
            white: '#c9d1d9',
            brightBlack: '#6e7681',
            brightRed: '#ffa198',
            brightGreen: '#56d364',
            brightYellow: '#e3b341',
            brightBlue: '#79c0ff',
            brightMagenta: '#d2a8ff',
            brightCyan: '#b3f0ff',
            brightWhite: '#f0f6fc',
        },
    });

    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    term.open($('xterm'));
    fitAddon.fit();

    // Toggle collapse
    header.addEventListener('click', () => {
        container.classList.toggle('collapsed');
        const btn = $('btnToggleTerm');
        btn.textContent = container.classList.contains('collapsed') ? '▶ Terminal' : '▼ Terminal';
        if (!container.classList.contains('collapsed')) {
            setTimeout(() => { fitAddon.fit(); term.focus(); }, 50);
        }
    });

    // Resize
    const resizeObs = new ResizeObserver(() => {
        if (!container.classList.contains('collapsed')) fitAddon.fit();
    });
    resizeObs.observe(container);

    // Input handling
    term.onData(onTermData);

    // Welcome
    term.writeln(`${C.blue}${C.bold}traits.build${C.reset} terminal — WASM + REST hybrid`);
    term.writeln(`${C.gray}${allTraits.length} traits loaded. Type "help" for commands.${C.reset}`);
    term.writeln('');
    writePrompt();
}

function writePrompt() {
    term.write(PROMPT);
}

function refreshLine() {
    term.write('\x1b[2K\r');  // clear entire line, carriage return
    term.write(PROMPT + lineBuffer);
    const tail = lineBuffer.length - cursorPos;
    if (tail > 0) term.write(`\x1b[${tail}D`);
}

function setLine(text) {
    lineBuffer = text;
    cursorPos = text.length;
    refreshLine();
}

function onTermData(data) {
    if (busy) return;

    // Escape sequences
    if (data === '\x1b[A') {         // Up arrow — history back
        if (termHistoryIdx > 0) {
            termHistoryIdx--;
            setLine(termHistory[termHistoryIdx]);
        } else if (termHistory.length > 0 && termHistoryIdx === -1) {
            termHistoryIdx = termHistory.length - 1;
            setLine(termHistory[termHistoryIdx]);
        }
        return;
    }
    if (data === '\x1b[B') {         // Down arrow — history forward
        if (termHistoryIdx < termHistory.length - 1) {
            termHistoryIdx++;
            setLine(termHistory[termHistoryIdx]);
        } else {
            termHistoryIdx = termHistory.length;
            setLine('');
        }
        return;
    }
    if (data === '\x1b[C') {         // Right arrow
        if (cursorPos < lineBuffer.length) { cursorPos++; term.write(data); }
        return;
    }
    if (data === '\x1b[D') {         // Left arrow
        if (cursorPos > 0) { cursorPos--; term.write(data); }
        return;
    }
    if (data === '\x1b[H') return;   // Home — ignore
    if (data === '\x1b[F') return;   // End — ignore
    if (data.startsWith('\x1b')) return;  // Other escape sequences — ignore

    // Control characters
    for (let i = 0; i < data.length; i++) {
        const ch = data[i];
        const code = ch.charCodeAt(0);

        if (code === 13) {            // Enter
            term.writeln('');
            const cmd = lineBuffer.trim();
            lineBuffer = '';
            cursorPos = 0;
            if (cmd) {
                termHistory.push(cmd);
                termHistoryIdx = termHistory.length;
                busy = true;
                termExec(cmd).finally(() => { busy = false; writePrompt(); });
            } else {
                writePrompt();
            }
            return; // Don't process remaining chars after Enter
        }
        if (code === 127 || code === 8) {  // Backspace / BS
            if (cursorPos > 0) {
                lineBuffer = lineBuffer.slice(0, cursorPos - 1) + lineBuffer.slice(cursorPos);
                cursorPos--;
                refreshLine();
            }
        } else if (code === 9) {      // Tab
            termTabComplete();
        } else if (code === 12) {     // Ctrl+L
            term.clear();
            refreshLine();
        } else if (code === 3) {      // Ctrl+C
            lineBuffer = '';
            cursorPos = 0;
            term.write('^C');
            term.writeln('');
            writePrompt();
        } else if (code === 21) {     // Ctrl+U — clear line
            lineBuffer = '';
            cursorPos = 0;
            refreshLine();
        } else if (code === 23) {     // Ctrl+W — delete word back
            const before = lineBuffer.slice(0, cursorPos);
            const trimmed = before.replace(/\S+\s*$/, '');
            lineBuffer = trimmed + lineBuffer.slice(cursorPos);
            cursorPos = trimmed.length;
            refreshLine();
        } else if (code === 1) {      // Ctrl+A — home
            cursorPos = 0;
            refreshLine();
        } else if (code === 5) {      // Ctrl+E — end
            cursorPos = lineBuffer.length;
            refreshLine();
        } else if (code >= 32) {      // Printable
            lineBuffer = lineBuffer.slice(0, cursorPos) + ch + lineBuffer.slice(cursorPos);
            cursorPos++;
            refreshLine();
        }
    }
}

// ── Tab completion ──

function termTabComplete() {
    const parts = lineBuffer.split(/\s+/);
    let prefix;
    if (parts.length <= 1) {
        prefix = parts[0] || '';
    } else if (['call', 'info', 'c', 'i'].includes(parts[0])) {
        prefix = parts[parts.length - 1];
    } else {
        return;
    }

    const matches = allTraits.map(t => t.path).filter(p => p.startsWith(prefix));

    if (matches.length === 1) {
        if (parts.length <= 1) {
            setLine(matches[0] + ' ');
        } else {
            parts[parts.length - 1] = matches[0];
            setLine(parts.join(' ') + ' ');
        }
    } else if (matches.length > 1 && matches.length <= 40) {
        // Show matches
        term.writeln('');
        const cols = term.cols || 80;
        const maxLen = Math.max(...matches.map(m => m.length)) + 2;
        const perRow = Math.max(1, Math.floor(cols / maxLen));
        for (let i = 0; i < matches.length; i += perRow) {
            const row = matches.slice(i, i + perRow).map(m => m.padEnd(maxLen)).join('');
            term.writeln(`${C.cyan}${row}${C.reset}`);
        }
        // Common prefix
        let common = matches[0];
        for (const m of matches) {
            while (!m.startsWith(common)) common = common.slice(0, -1);
        }
        if (common.length > prefix.length) {
            if (parts.length <= 1) {
                setLine(common);
            } else {
                parts[parts.length - 1] = common;
                setLine(parts.join(' '));
            }
        } else {
            refreshLine();
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
            case 'help': case 'h': case '?':
                termShowHelp(); break;
            case 'list': case 'ls':
                termList(args[0]); break;
            case 'info': case 'i':
                if (!args[0]) { term.writeln(`${C.red}Usage: info <trait_path>${C.reset}`); break; }
                termInfo(args[0]); break;
            case 'call': case 'c':
                if (!args[0]) { term.writeln(`${C.red}Usage: call <trait_path> [args...]${C.reset}`); break; }
                await termCall(args[0], args.slice(1)); break;
            case 'search': case 's':
                termSearch(args.join(' ')); break;
            case 'version': case 'v':
                termWriteVersion(); break;
            case 'clear': case 'cls':
                term.clear(); break;
            case 'history':
                termHistory.forEach((h, i) => term.writeln(`  ${C.gray}${String(i + 1).padStart(3)}${C.reset}  ${h}`));
                break;
            default:
                if (allTraits.some(t => t.path === command)) {
                    await termCall(command, args);
                } else {
                    term.writeln(`${C.red}Unknown command: ${command}${C.reset}. Type ${C.blue}help${C.reset} for usage.`);
                }
        }
    } catch (e) {
        term.writeln(`${C.red}Error: ${e.message || String(e)}${C.reset}`);
    }
}

function parseCommand(cmd) {
    const parts = [];
    let current = '';
    let inQuote = false;
    for (const ch of cmd) {
        if (ch === '"') { inQuote = !inQuote; }
        else if (ch === ' ' && !inQuote) {
            if (current) { parts.push(current); current = ''; }
        } else { current += ch; }
    }
    if (current) parts.push(current);
    return parts;
}

// ── Terminal commands ──

function termShowHelp() {
    const lines = [
        [`${C.bold}${C.brightWhite}Commands${C.reset}`, ''],
        [`  ${C.green}list${C.reset} ${C.gray}[namespace]${C.reset}`, 'List traits (filter by namespace)'],
        [`  ${C.green}info${C.reset} ${C.gray}<path>${C.reset}`, 'Show trait details and parameters'],
        [`  ${C.green}call${C.reset} ${C.gray}<path> [args...]${C.reset}`, 'Call a trait (WASM or REST)'],
        [`  ${C.green}search${C.reset} ${C.gray}<query>${C.reset}`, 'Search by name or description'],
        [`  ${C.gray}<path> [args...]${C.reset}`, 'Shorthand — call trait directly'],
        [`  ${C.green}history${C.reset}`, 'Show command history'],
        [`  ${C.green}clear${C.reset}`, 'Clear terminal'],
        [`  ${C.green}version${C.reset}`, 'Show version info'],
        [`  ${C.green}help${C.reset}`, 'Show this help'],
        ['', ''],
        [`${C.bold}${C.brightWhite}Shortcuts${C.reset}`, ''],
        [`  ${C.cyan}Tab${C.reset}`, 'Auto-complete trait paths'],
        [`  ${C.cyan}↑ / ↓${C.reset}`, 'Navigate command history'],
        [`  ${C.cyan}Ctrl+L${C.reset}`, 'Clear terminal'],
        [`  ${C.cyan}Ctrl+C${C.reset}`, 'Cancel current line'],
        [`  ${C.cyan}Ctrl+U${C.reset}`, 'Clear entire line'],
        [`  ${C.cyan}Ctrl+W${C.reset}`, 'Delete word backward'],
        [`  ${C.cyan}Ctrl+A/E${C.reset}`, 'Jump to start/end of line'],
        ['', ''],
        [`${C.bold}${C.brightWhite}Examples${C.reset}`, ''],
        [`  ${C.gray}call sys.checksum hash "hello world"${C.reset}`, ''],
        [`  ${C.gray}sys.version${C.reset}`, ''],
        [`  ${C.gray}info sys.list${C.reset}`, ''],
        [`  ${C.gray}list sys${C.reset}`, ''],
        [`  ${C.gray}search checksum${C.reset}`, ''],
    ];
    for (const [left, right] of lines) {
        if (right) {
            term.writeln(`${left}  ${C.gray}${right}${C.reset}`);
        } else {
            term.writeln(left);
        }
    }
}

function termList(namespace) {
    let filtered = allTraits;
    if (namespace) {
        filtered = allTraits.filter(t => t.path.startsWith(namespace));
        if (filtered.length === 0) {
            term.writeln(`${C.yellow}No traits in namespace "${namespace}"${C.reset}`);
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
        term.writeln(`${C.bold}${C.brightWhite}${ns}${C.reset} ${C.gray}(${traits.length})${C.reset}`);
        for (const t of traits) {
            const name = t.path.split('.').pop();
            const badge = t.wasm_callable
                ? `${C.green}[WASM]${C.reset}`
                : `${C.yellow}[REST]${C.reset}`;
            term.writeln(`  ${badge} ${C.blue}${name}${C.reset}  ${C.gray}${t.description || ''}${C.reset}`);
        }
    }
    term.writeln(`${C.gray}${filtered.length} traits${C.reset}`);
}

function termInfo(path) {
    const raw = get_trait_info(path);
    if (!raw) {
        term.writeln(`${C.red}Trait "${path}" not found${C.reset}`);
        return;
    }
    const t = JSON.parse(raw);
    const badge = t.wasm_callable
        ? `${C.green}WASM${C.reset}`
        : `${C.yellow}REST${C.reset}`;
    term.writeln(`${C.bold}${C.brightWhite}${t.path}${C.reset}  ${badge}  ${C.gray}${t.version || ''}${C.reset}`);
    if (t.description) term.writeln(`  ${C.gray}${t.description}${C.reset}`);

    const params = t.params || [];
    if (params.length > 0) {
        term.writeln('');
        term.writeln(`${C.bold}Parameters:${C.reset}`);
        for (const p of params) {
            const req = p.required ? ` ${C.red}*${C.reset}` : '';
            term.writeln(`  ${C.blue}${p.name}${C.reset} ${C.magenta}(${p.type})${C.reset}${req}  ${C.gray}${p.description || ''}${C.reset}`);
        }
    }
    if (t.returns) {
        term.writeln('');
        term.writeln(`${C.bold}Returns:${C.reset} ${C.magenta}${t.returns}${C.reset}  ${C.gray}${t.returns_description || ''}${C.reset}`);
    }
}

async function termCall(path, argStrs) {
    const isLocal = wasmCallableSet.has(path);
    const tag = isLocal ? `${C.green}WASM${C.reset}` : `${C.yellow}REST${C.reset}`;

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
    const lines = formatted.split('\n');
    if (lines.length > 100) {
        for (const line of lines.slice(0, 80)) term.writeln(`${C.green}${line}${C.reset}`);
        term.writeln(`${C.gray}... (${lines.length - 80} more lines, ${formatted.length} bytes total)${C.reset}`);
    } else {
        for (const line of lines) term.writeln(`${C.green}${line}${C.reset}`);
    }
    term.writeln(`${C.gray}${dt}ms (${C.reset}${tag}${C.gray})${C.reset}`);
}

function termWriteVersion() {
    const status = $('status').textContent;
    term.writeln(`${C.green}${status}${C.reset}`);
}

function termSearch(query) {
    if (!query) { term.writeln(`${C.red}Usage: search <query>${C.reset}`); return; }
    const q = query.toLowerCase();
    const results = allTraits.filter(t =>
        t.path.toLowerCase().includes(q) ||
        (t.description || '').toLowerCase().includes(q)
    );
    if (results.length === 0) {
        term.writeln(`${C.yellow}No matches for "${query}"${C.reset}`);
        return;
    }
    for (const t of results) {
        const badge = t.wasm_callable
            ? `${C.green}[WASM]${C.reset}`
            : `${C.yellow}[REST]${C.reset}`;
        term.writeln(`${badge} ${C.blue}${t.path}${C.reset}  ${C.gray}${t.description || ''}${C.reset}`);
    }
    term.writeln(`${C.gray}${results.length} matches${C.reset}`);
}

// ── Go ──
boot();
