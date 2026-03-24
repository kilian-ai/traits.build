// ── API Docs Terminal — test endpoints live ──

const $ = id => document.getElementById(id);

let Terminal, FitAddon, WebLinksAddon;
let term = null, fitAddon = null;
let lineBuffer = '', cursorPos = 0;
const history = [];
let histIdx = -1;
let busy = false;
let traitPaths = []; // for tab completion

const C = {
    reset: '\x1b[0m', bold: '\x1b[1m', dim: '\x1b[2m',
    green: '\x1b[32m', red: '\x1b[31m', yellow: '\x1b[33m',
    blue: '\x1b[34m', magenta: '\x1b[35m', cyan: '\x1b[36m',
    white: '\x1b[37m', gray: '\x1b[90m', brightWhite: '\x1b[97m',
};
const PROMPT = `${C.cyan}api ${C.reset}`;
const PROMPT_LEN = 4;

// ── REST helpers ──

async function restCall(path, args = []) {
    const url = '/traits/' + path.replace(/\./g, '/');
    const t0 = performance.now();
    const resp = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args }),
    });
    const ms = Math.round(performance.now() - t0);
    const data = await resp.json();
    return { ok: resp.ok, status: resp.status, ms, data };
}

async function loadTraitList() {
    try {
        const resp = await fetch('/traits');
        if (!resp.ok) return;
        const tree = await resp.json();
        traitPaths = [];
        for (const [ns, names] of Object.entries(tree)) {
            for (const name of names) traitPaths.push(`${ns}.${name}`);
        }
        traitPaths.sort();
    } catch { /* offline */ }
}

// ── Init ──

async function initTerminal() {
    const header = $('termHeader');
    const container = $('termContainer');

    try {
        const xtermMod = await import('https://cdn.jsdelivr.net/npm/@xterm/xterm@5/+esm');
        Terminal = xtermMod.Terminal;
        const fitMod = await import('https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.10/+esm');
        FitAddon = fitMod.FitAddon;
        const linksMod = await import('https://cdn.jsdelivr.net/npm/@xterm/addon-web-links@0.11/+esm');
        WebLinksAddon = linksMod.WebLinksAddon;
    } catch (e) {
        console.error('Failed to load xterm.js:', e);
        container.innerHTML = `<div style="padding:1rem;color:#f85149">Failed to load terminal: ${e.message}</div>`;
        return;
    }

    term = new Terminal({
        cursorBlink: true,
        cursorStyle: 'block',
        fontSize: 13,
        fontFamily: "'SF Mono', 'Fira Code', 'Cascadia Code', 'Menlo', monospace",
        lineHeight: 1.3,
        scrollback: 5000,
        theme: {
            background: '#0d1117',
            foreground: '#c9d1d9',
            cursor: '#c9d1d9',
            cursorAccent: '#0d1117',
            selectionBackground: '#264f78',
            selectionForeground: '#ffffff',
            black: '#484f58',   red: '#f85149',   green: '#3fb950',
            yellow: '#d29922',  blue: '#58a6ff',  magenta: '#bc8cff',
            cyan: '#76e3ea',    white: '#c9d1d9',
            brightBlack: '#6e7681',  brightRed: '#ffa198',   brightGreen: '#56d364',
            brightYellow: '#e3b341', brightBlue: '#79c0ff',  brightMagenta: '#d2a8ff',
            brightCyan: '#b3f0ff',   brightWhite: '#f0f6fc',
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
        btn.textContent = container.classList.contains('collapsed') ? '▶ API Terminal' : '▼ API Terminal';
        if (!container.classList.contains('collapsed')) {
            setTimeout(() => { fitAddon.fit(); term.focus(); }, 50);
        }
    });

    new ResizeObserver(() => {
        if (!container.classList.contains('collapsed')) fitAddon.fit();
    }).observe(container);

    term.onData(onData);

    await loadTraitList();

    term.writeln(`${C.cyan}${C.bold}traits.build${C.reset} API terminal — test endpoints live`);
    term.writeln(`${C.gray}${traitPaths.length} traits available. Type "help" for commands.${C.reset}`);
    term.writeln('');
    writePrompt();
}

// ── Prompt / line editing ──

function writePrompt() { term.write(PROMPT); }

function refreshLine() {
    term.write('\x1b[2K\r');
    term.write(PROMPT + lineBuffer);
    const tail = lineBuffer.length - cursorPos;
    if (tail > 0) term.write(`\x1b[${tail}D`);
}

function setLine(text) {
    lineBuffer = text; cursorPos = text.length; refreshLine();
}

function onData(data) {
    if (busy) return;

    if (data === '\x1b[A') { // Up
        if (histIdx > 0) { histIdx--; setLine(history[histIdx]); }
        else if (history.length && histIdx === -1) { histIdx = history.length - 1; setLine(history[histIdx]); }
        return;
    }
    if (data === '\x1b[B') { // Down
        if (histIdx < history.length - 1) { histIdx++; setLine(history[histIdx]); }
        else { histIdx = history.length; setLine(''); }
        return;
    }
    if (data === '\x1b[C') { if (cursorPos < lineBuffer.length) { cursorPos++; term.write(data); } return; }
    if (data === '\x1b[D') { if (cursorPos > 0) { cursorPos--; term.write(data); } return; }
    if (data.startsWith('\x1b')) return;

    for (let i = 0; i < data.length; i++) {
        const ch = data[i], code = ch.charCodeAt(0);

        if (code === 13) {          // Enter
            term.writeln('');
            const cmd = lineBuffer.trim();
            lineBuffer = ''; cursorPos = 0;
            if (cmd) {
                history.push(cmd);
                histIdx = history.length;
                busy = true;
                execCmd(cmd).finally(() => { busy = false; writePrompt(); });
            } else { writePrompt(); }
            return;
        }
        if (code === 127 || code === 8) { // Backspace
            if (cursorPos > 0) {
                lineBuffer = lineBuffer.slice(0, cursorPos - 1) + lineBuffer.slice(cursorPos);
                cursorPos--; refreshLine();
            }
        } else if (code === 9)  { tabComplete(); }
        else if (code === 12) { term.clear(); refreshLine(); }
        else if (code === 3)  { lineBuffer = ''; cursorPos = 0; term.write('^C'); term.writeln(''); writePrompt(); }
        else if (code === 21) { lineBuffer = ''; cursorPos = 0; refreshLine(); }
        else if (code === 23) {
            const before = lineBuffer.slice(0, cursorPos).replace(/\S+\s*$/, '');
            lineBuffer = before + lineBuffer.slice(cursorPos);
            cursorPos = before.length; refreshLine();
        }
        else if (code === 1) { cursorPos = 0; refreshLine(); }
        else if (code === 5) { cursorPos = lineBuffer.length; refreshLine(); }
        else if (code >= 32) {
            lineBuffer = lineBuffer.slice(0, cursorPos) + ch + lineBuffer.slice(cursorPos);
            cursorPos++; refreshLine();
        }
    }
}

// ── Tab completion ──

function tabComplete() {
    const parts = lineBuffer.split(/\s+/);
    let prefix;
    if (parts.length <= 1) prefix = parts[0] || '';
    else if (['post', 'get', 'call', 'info', 'c', 'i', 'p'].includes(parts[0].toLowerCase()))
        prefix = parts[parts.length - 1];
    else return;

    const matches = traitPaths.filter(p => p.startsWith(prefix));
    if (matches.length === 1) {
        if (parts.length <= 1) setLine(matches[0] + ' ');
        else { parts[parts.length - 1] = matches[0]; setLine(parts.join(' ') + ' '); }
    } else if (matches.length > 1 && matches.length <= 40) {
        term.writeln('');
        const cols = term.cols || 80;
        const maxLen = Math.max(...matches.map(m => m.length)) + 2;
        const perRow = Math.max(1, Math.floor(cols / maxLen));
        for (let i = 0; i < matches.length; i += perRow) {
            term.writeln(`${C.cyan}${matches.slice(i, i + perRow).map(m => m.padEnd(maxLen)).join('')}${C.reset}`);
        }
        let common = matches[0];
        for (const m of matches) { while (!m.startsWith(common)) common = common.slice(0, -1); }
        if (common.length > prefix.length) {
            if (parts.length <= 1) setLine(common);
            else { parts[parts.length - 1] = common; setLine(parts.join(' ')); }
        } else refreshLine();
    }
}

// ── Command dispatch ──

function parseArgs(cmd) {
    const parts = [];
    let cur = '', inQ = false;
    for (const ch of cmd) {
        if (ch === '"') inQ = !inQ;
        else if (ch === ' ' && !inQ) { if (cur) { parts.push(cur); cur = ''; } }
        else cur += ch;
    }
    if (cur) parts.push(cur);
    return parts;
}

async function execCmd(cmd) {
    const parts = parseArgs(cmd);
    const command = parts[0].toLowerCase();
    const args = parts.slice(1);

    try {
        switch (command) {
        case 'help': case 'h': case '?':
            showHelp(); break;
        case 'list': case 'ls':
            showList(args[0]); break;
        case 'info': case 'i':
            if (!args[0]) { term.writeln(`${C.red}Usage: info <trait_path>${C.reset}`); break; }
            await doInfo(args[0]); break;
        case 'post': case 'call': case 'c': case 'p':
            if (!args[0]) { term.writeln(`${C.red}Usage: post <trait_path> [args...]${C.reset}`); break; }
            await doPost(args[0], args.slice(1)); break;
        case 'get':
            if (!args[0]) { term.writeln(`${C.red}Usage: get <path>${C.reset}`); break; }
            await doGet(args[0]); break;
        case 'search': case 's':
            doSearch(args.join(' ')); break;
        case 'clear': case 'cls':
            term.clear(); break;
        case 'history':
            history.forEach((h, i) => term.writeln(`  ${C.gray}${String(i + 1).padStart(3)}${C.reset}  ${h}`));
            break;
        default:
            if (traitPaths.includes(command) || traitPaths.includes(parts[0])) {
                await doPost(parts[0], args);
            } else {
                term.writeln(`${C.red}Unknown: ${command}${C.reset}. Type ${C.blue}help${C.reset} for commands.`);
            }
        }
    } catch (e) {
        term.writeln(`${C.red}Error: ${e.message || String(e)}${C.reset}`);
    }
}

// ── Commands ──

function showHelp() {
    const lines = [
        [`${C.bold}${C.brightWhite}REST API Terminal${C.reset}`, ''],
        ['', ''],
        [`  ${C.green}post${C.reset} ${C.gray}<trait> [args...]${C.reset}`, 'POST /traits/{ns}/{name} { args }'],
        [`  ${C.green}get${C.reset} ${C.gray}<path>${C.reset}`, 'GET request to path'],
        [`  ${C.green}info${C.reset} ${C.gray}<trait>${C.reset}`, 'Show trait signature and params'],
        [`  ${C.green}list${C.reset} ${C.gray}[namespace]${C.reset}`, 'List traits (filter by namespace)'],
        [`  ${C.green}search${C.reset} ${C.gray}<query>${C.reset}`, 'Search traits by name/desc'],
        [`  ${C.gray}<trait> [args...]${C.reset}`, 'Shorthand — POST trait directly'],
        [`  ${C.green}history${C.reset}`, 'Show command history'],
        [`  ${C.green}clear${C.reset}`, 'Clear terminal'],
        [`  ${C.green}help${C.reset}`, 'Show this help'],
        ['', ''],
        [`${C.bold}${C.brightWhite}Examples${C.reset}`, ''],
        [`  ${C.gray}post sys.checksum hash "hello"${C.reset}`, '→ POST /traits/sys/checksum'],
        [`  ${C.gray}sys.list${C.reset}`, '→ POST /traits/sys/list'],
        [`  ${C.gray}get /health${C.reset}`, '→ GET /health'],
        [`  ${C.gray}info sys.openapi${C.reset}`, 'Show trait details'],
    ];
    for (const [left, right] of lines) {
        term.writeln(right ? `${left}  ${C.gray}${right}${C.reset}` : left);
    }
}

function showList(namespace) {
    let filtered = traitPaths;
    if (namespace) {
        filtered = traitPaths.filter(p => p.startsWith(namespace));
        if (!filtered.length) { term.writeln(`${C.yellow}No traits in "${namespace}"${C.reset}`); return; }
    }
    const groups = {};
    for (const p of filtered) {
        const ns = p.split('.').slice(0, -1).join('.');
        (groups[ns] || (groups[ns] = [])).push(p);
    }
    for (const [ns, paths] of Object.entries(groups).sort((a, b) => a[0].localeCompare(b[0]))) {
        term.writeln(`${C.bold}${C.brightWhite}${ns}${C.reset} ${C.gray}(${paths.length})${C.reset}`);
        for (const p of paths) {
            const name = p.split('.').pop();
            const url = '/traits/' + p.replace(/\./g, '/');
            term.writeln(`  ${C.blue}${name}${C.reset}  ${C.gray}POST ${url}${C.reset}`);
        }
    }
    term.writeln(`${C.gray}${filtered.length} traits${C.reset}`);
}

async function doInfo(path) {
    const res = await restCall('sys.info', [path]);
    if (!res.ok) { term.writeln(`${C.red}${res.status} — ${JSON.stringify(res.data)}${C.reset}`); return; }
    const t = res.data.result || res.data;
    if (!t || !t.path) { term.writeln(`${C.red}Trait "${path}" not found${C.reset}`); return; }

    const url = '/traits/' + t.path.replace(/\./g, '/');
    term.writeln(`${C.bold}${C.brightWhite}${t.path}${C.reset}  ${C.gray}${t.version || ''}${C.reset}`);
    term.writeln(`  ${C.cyan}POST ${url}${C.reset}`);
    if (t.description) term.writeln(`  ${C.gray}${t.description}${C.reset}`);

    const params = t.params || t.signature?.params || [];
    if (params.length) {
        term.writeln('');
        term.writeln(`${C.bold}Parameters:${C.reset}`);
        for (const p of params) {
            const req = p.required ? ` ${C.red}*${C.reset}` : '';
            term.writeln(`  ${C.blue}${p.name}${C.reset} ${C.magenta}(${p.type})${C.reset}${req}  ${C.gray}${p.description || ''}${C.reset}`);
        }
    }
    const ret = t.returns || t.signature?.returns;
    if (ret) {
        const rtype = typeof ret === 'string' ? ret : ret.type;
        const rdesc = typeof ret === 'string' ? '' : ret.description || '';
        term.writeln('');
        term.writeln(`${C.bold}Returns:${C.reset} ${C.magenta}${rtype}${C.reset}  ${C.gray}${rdesc}${C.reset}`);
    }

    term.writeln(`${C.gray}${res.ms}ms${C.reset}`);
}

async function doPost(path, argStrs) {
    const args = argStrs.map(a => { try { return JSON.parse(a); } catch { return a; } });
    const url = '/traits/' + path.replace(/\./g, '/');

    term.writeln(`${C.dim}POST ${url}${C.reset}${args.length ? `${C.dim} { args: ${JSON.stringify(args)} }${C.reset}` : ''}`);

    const res = await restCall(path, args);
    const status = res.ok ? `${C.green}${res.status}${C.reset}` : `${C.red}${res.status}${C.reset}`;

    const result = res.data.result !== undefined ? res.data.result : res.data;
    const formatted = typeof result === 'string' ? result : JSON.stringify(result, null, 2);
    const lines = formatted.split('\n');
    if (lines.length > 100) {
        for (const l of lines.slice(0, 80)) term.writeln(l);
        term.writeln(`${C.gray}... (${lines.length - 80} more lines)${C.reset}`);
    } else {
        for (const l of lines) term.writeln(l);
    }
    term.writeln(`${C.gray}${status} ${res.ms}ms${C.reset}`);
}

async function doGet(path) {
    if (!path.startsWith('/')) path = '/' + path;
    term.writeln(`${C.dim}GET ${path}${C.reset}`);
    const t0 = performance.now();
    const resp = await fetch(path);
    const ms = Math.round(performance.now() - t0);
    const status = resp.ok ? `${C.green}${resp.status}${C.reset}` : `${C.red}${resp.status}${C.reset}`;
    const ct = resp.headers.get('content-type') || '';

    if (ct.includes('json')) {
        const data = await resp.json();
        const formatted = JSON.stringify(data, null, 2);
        const lines = formatted.split('\n');
        if (lines.length > 100) {
            for (const l of lines.slice(0, 80)) term.writeln(l);
            term.writeln(`${C.gray}... (${lines.length - 80} more lines)${C.reset}`);
        } else {
            for (const l of lines) term.writeln(l);
        }
    } else {
        const text = await resp.text();
        const lines = text.split('\n');
        const show = lines.length > 20 ? lines.slice(0, 20) : lines;
        for (const l of show) term.writeln(l);
        if (lines.length > 20) term.writeln(`${C.gray}... (${lines.length - 20} more lines, ${text.length} bytes)${C.reset}`);
    }
    term.writeln(`${C.gray}${status} ${ms}ms${C.reset}`);
}

function doSearch(query) {
    if (!query) { term.writeln(`${C.red}Usage: search <query>${C.reset}`); return; }
    const q = query.toLowerCase();
    const matches = traitPaths.filter(p => p.toLowerCase().includes(q));
    if (!matches.length) { term.writeln(`${C.yellow}No matches for "${query}"${C.reset}`); return; }
    for (const p of matches) {
        const url = '/traits/' + p.replace(/\./g, '/');
        term.writeln(`  ${C.blue}${p}${C.reset}  ${C.gray}POST ${url}${C.reset}`);
    }
    term.writeln(`${C.gray}${matches.length} matches${C.reset}`);
}

// Boot
initTerminal();
