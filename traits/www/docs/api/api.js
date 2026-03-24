// ── API Docs Terminal — real PTY via WebSocket ──

const $ = id => document.getElementById(id);

let Terminal, FitAddon, WebLinksAddon;
let term = null, fitAddon = null, ws = null;
let reconnectTimer = null;
const RECONNECT_MS = 2000;

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
        btn.textContent = container.classList.contains('collapsed') ? '▶ Terminal' : '▼ Terminal';
        if (!container.classList.contains('collapsed')) {
            setTimeout(() => { fitAddon.fit(); term.focus(); }, 50);
        }
    });

    new ResizeObserver(() => {
        if (!container.classList.contains('collapsed')) fitAddon.fit();
    }).observe(container);

    // Keyboard → WebSocket
    term.onData(data => {
        if (ws && ws.readyState === WebSocket.OPEN) ws.send(data);
    });

    // Resize → WebSocket
    term.onResize(({ cols, rows }) => {
        if (ws && ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ type: 'resize', cols, rows }));
        }
    });

    connectWS();
}

// ── WebSocket connection ──

function connectWS() {
    if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }

    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const url = `${proto}//${location.host}/ws/terminal`;

    ws = new WebSocket(url);
    ws.binaryType = 'arraybuffer';

    ws.addEventListener('open', () => {
        // Send initial size
        if (term) {
            ws.send(JSON.stringify({ type: 'resize', cols: term.cols, rows: term.rows }));
        }
    });

    ws.addEventListener('message', (e) => {
        if (!term) return;
        if (e.data instanceof ArrayBuffer) {
            term.write(new Uint8Array(e.data));
        } else {
            term.write(e.data);
        }
    });

    ws.addEventListener('close', () => {
        if (term) term.writeln('\r\n\x1b[90m[disconnected — reconnecting...]\x1b[0m');
        reconnectTimer = setTimeout(connectWS, RECONNECT_MS);
    });

    ws.addEventListener('error', () => {
        // close event will fire after error, triggering reconnect
    });
}

// Boot
initTerminal();
