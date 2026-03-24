// ═══════════════════════════════════════════
// ── Shared WASM-powered Terminal ──
// Used by both /wasm and /docs/api pages.
// Loads the WASM kernel, provides xterm.js terminal
// with line editing, history, tab completion.
// Command execution goes through wasm.cli_exec().
// ═══════════════════════════════════════════

const C = {
    reset: '\x1b[0m', bold: '\x1b[1m', dim: '\x1b[2m',
    green: '\x1b[32m', red: '\x1b[31m', yellow: '\x1b[33m',
    blue: '\x1b[34m', magenta: '\x1b[35m', cyan: '\x1b[36m',
    white: '\x1b[37m', gray: '\x1b[90m', brightWhite: '\x1b[97m',
};
const PROMPT = `${C.green}traits ${C.reset}`;
const PROMPT_LEN = 7;

let Terminal, FitAddon, WebLinksAddon;

/**
 * Create and mount a WASM-powered terminal.
 * @param {HTMLElement} mountEl  — element to mount xterm.js into (e.g. #xterm)
 * @param {object} opts
 * @param {HTMLElement} [opts.header]    — clickable header for collapse/expand
 * @param {HTMLElement} [opts.container] — container to toggle .collapsed on
 * @param {HTMLElement} [opts.toggleBtn] — button whose text changes on collapse
 * @param {HTMLElement} [opts.statusEl]  — element for WASM status badge
 * @param {function}    [opts.onReady]   — called with { wasm, traitCount, wasmCount } when ready
 * @returns {Promise<{term, fitAddon, wasm}>}
 */
export async function createTerminal(mountEl, opts = {}) {
    // ── Load xterm.js ──
    try {
        const xtermMod = await import('https://cdn.jsdelivr.net/npm/@xterm/xterm@5/+esm');
        Terminal = xtermMod.Terminal;
        const fitMod = await import('https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.10/+esm');
        FitAddon = fitMod.FitAddon;
        const linksMod = await import('https://cdn.jsdelivr.net/npm/@xterm/addon-web-links@0.11/+esm');
        WebLinksAddon = linksMod.WebLinksAddon;
    } catch (e) {
        mountEl.innerHTML = `<div style="padding:1rem;color:#f85149">Failed to load terminal: ${e.message}</div>`;
        throw e;
    }

    const term = new Terminal({
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

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    term.open(mountEl);
    fitAddon.fit();

    // ── Collapse/expand ──
    if (opts.header && opts.container) {
        opts.header.addEventListener('click', () => {
            opts.container.classList.toggle('collapsed');
            if (opts.toggleBtn) {
                opts.toggleBtn.textContent = opts.container.classList.contains('collapsed')
                    ? '▶ Terminal' : '▼ Terminal';
            }
            if (!opts.container.classList.contains('collapsed')) {
                setTimeout(() => { fitAddon.fit(); term.focus(); }, 50);
            }
        });
    }
    if (opts.container) {
        new ResizeObserver(() => {
            if (!opts.container.classList.contains('collapsed')) fitAddon.fit();
        }).observe(opts.container);
    }

    // ── Status ──
    const setStatus = (text, cls) => {
        if (opts.statusEl) {
            opts.statusEl.textContent = text;
            opts.statusEl.className = 'terminal-status ' + (cls || '');
        }
    };
    setStatus('loading WASM…', 'loading');

    // ── Load WASM kernel ──
    let wasm = null;
    try {
        const wasmJsUrl = '/wasm/traits_wasm.js';
        const wasmBinUrl = '/wasm/traits_wasm_bg.wasm';
        const mod = await import(wasmJsUrl);
        await mod.default(wasmBinUrl);
        const initResult = JSON.parse(mod.init());
        wasm = mod;
        const count = initResult.traits_registered || 0;
        const wasmCount = initResult.wasm_callable || 0;
        setStatus(`${count} traits (${wasmCount} WASM)`, 'ready');
        if (opts.onReady) opts.onReady({ wasm, traitCount: count, wasmCount });
    } catch (e) {
        setStatus('WASM failed — REST only', 'error');
        console.error('WASM load failed:', e);
    }

    // ── Line editing state ──
    let lineBuffer = '';
    let cursorPos = 0;
    const history = [];
    let histIdx = -1;
    let busy = false;

    // ── Interactive mode state ──
    let interactive = null; // { path, params, values, idx }

    function writePrompt() { term.write(PROMPT); }

    function refreshLine() {
        term.write('\x1b[2K\r');
        if (interactive) {
            const p = interactive.params[interactive.idx];
            const name = p.name || '?';
            term.write(`  ${C.cyan}❯${C.reset} ` + lineBuffer);
        } else {
            term.write(PROMPT + lineBuffer);
        }
        const tail = lineBuffer.length - cursorPos;
        if (tail > 0) term.write(`\x1b[${tail}D`);
    }

    function setLine(text) {
        lineBuffer = text;
        cursorPos = text.length;
        refreshLine();
    }

    // ── Command execution (via WASM cli_exec) ──
    async function execCommand(cmd) {
        if (!wasm || !wasm.cli_exec) {
            // Fallback: no WASM, show error
            term.writeln(`${C.red}WASM kernel not loaded — cannot execute commands${C.reset}`);
            return;
        }

        // Check for interactive flag
        const parts = cmd.split(/\s+/);
        if (parts.includes('-i') || parts.includes('--interactive')) {
            const path = parts.find(p => p !== 'call' && p !== 'c' && p !== '-i' && p !== '--interactive');
            if (path) {
                startInteractive(path);
                return;
            }
        }

        const output = wasm.cli_exec(cmd);
        if (output === '\x1b[CLEAR]') {
            term.clear();
        } else if (output) {
            term.write(output);
            // Ensure output ends with newline
            if (!output.endsWith('\n') && !output.endsWith('\r\n')) {
                term.writeln('');
            }
        }
    }

    // ── Interactive mode ──
    function startInteractive(path) {
        if (!wasm || !wasm.cli_interactive_params) {
            term.writeln(`${C.red}Interactive mode not available${C.reset}`);
            return;
        }
        const paramsJson = wasm.cli_interactive_params(path);
        if (!paramsJson) {
            term.writeln(`${C.red}Trait "${path}" not found${C.reset}`);
            return;
        }
        const params = JSON.parse(paramsJson);
        if (!params || params.length === 0) {
            term.writeln(`${C.gray}No parameters — calling directly${C.reset}`);
            execCommand(`call ${path}`);
            return;
        }
        interactive = { path, params, values: [], idx: 0 };
        showParamPrompt();
    }

    function showParamPrompt() {
        const p = interactive.params[interactive.idx];
        const req = p.required ? `${C.red}*${C.reset}` : ' ';
        const type = p.type || 'any';
        const desc = p.description || '';
        term.writeln(`  ${req} ${C.bold}${p.name}${C.reset}  ${C.gray}${type}${C.reset}  ${C.gray}${desc}${C.reset}`);
        term.write(`  ${C.cyan}❯${C.reset} `);
    }

    function handleInteractiveInput(value) {
        const p = interactive.params[interactive.idx];
        if (!value && p.required) {
            term.writeln(`${C.red}  Required parameter${C.reset}`);
            term.write(`  ${C.cyan}❯${C.reset} `);
            return;
        }
        interactive.values.push(value || '');
        interactive.idx++;
        if (interactive.idx < interactive.params.length) {
            showParamPrompt();
        } else {
            // All params collected — dispatch
            const args = interactive.values.map(v => v.includes(' ') ? `"${v}"` : v).join(' ');
            const cmd = `call ${interactive.path} ${args}`;
            interactive = null;
            execCommand(cmd).finally(() => { busy = false; writePrompt(); });
        }
    }

    // ── Tab completion ──
    function tabComplete() {
        if (!wasm || !wasm.cli_complete) return;

        const parts = lineBuffer.split(/\s+/);
        let prefix;
        if (parts.length <= 1) {
            prefix = parts[0] || '';
        } else if (['call', 'info', 'c', 'i'].includes(parts[0].toLowerCase())) {
            prefix = parts[parts.length - 1];
        } else {
            return;
        }

        const result = JSON.parse(wasm.cli_complete(prefix));
        const matches = result.matches || [];
        const common = result.common || '';

        if (matches.length === 1) {
            if (parts.length <= 1) {
                setLine(matches[0] + ' ');
            } else {
                parts[parts.length - 1] = matches[0];
                setLine(parts.join(' ') + ' ');
            }
        } else if (matches.length > 1 && matches.length <= 40) {
            term.writeln('');
            const cols = term.cols || 80;
            const maxLen = Math.max(...matches.map(m => m.length)) + 2;
            const perRow = Math.max(1, Math.floor(cols / maxLen));
            for (let i = 0; i < matches.length; i += perRow) {
                const row = matches.slice(i, i + perRow).map(m => m.padEnd(maxLen)).join('');
                term.writeln(`${C.cyan}${row}${C.reset}`);
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

    // ── Input handling ──
    term.onData(data => {
        if (busy) return;

        // Escape sequences
        if (data === '\x1b[A') { // Up
            if (interactive) return;
            if (histIdx > 0) { histIdx--; setLine(history[histIdx]); }
            else if (history.length > 0 && histIdx === -1) { histIdx = history.length - 1; setLine(history[histIdx]); }
            return;
        }
        if (data === '\x1b[B') { // Down
            if (interactive) return;
            if (histIdx < history.length - 1) { histIdx++; setLine(history[histIdx]); }
            else { histIdx = history.length; setLine(''); }
            return;
        }
        if (data === '\x1b[C') { if (cursorPos < lineBuffer.length) { cursorPos++; term.write(data); } return; }
        if (data === '\x1b[D') { if (cursorPos > 0) { cursorPos--; term.write(data); } return; }
        if (data.startsWith('\x1b')) return;

        for (let i = 0; i < data.length; i++) {
            const ch = data[i];
            const code = ch.charCodeAt(0);

            if (code === 13) { // Enter
                term.writeln('');
                const input = lineBuffer.trim();
                lineBuffer = '';
                cursorPos = 0;

                if (interactive) {
                    handleInteractiveInput(input);
                    return;
                }

                if (input) {
                    history.push(input);
                    histIdx = history.length;
                    busy = true;
                    execCommand(input).finally(() => { busy = false; writePrompt(); });
                } else {
                    writePrompt();
                }
                return;
            }
            if (code === 127 || code === 8) { // Backspace
                if (cursorPos > 0) {
                    lineBuffer = lineBuffer.slice(0, cursorPos - 1) + lineBuffer.slice(cursorPos);
                    cursorPos--;
                    refreshLine();
                }
            } else if (code === 9) { tabComplete(); } // Tab
            else if (code === 12) { term.clear(); refreshLine(); } // Ctrl+L
            else if (code === 3) { // Ctrl+C
                if (interactive) {
                    interactive = null;
                    term.write('^C');
                    term.writeln('');
                    writePrompt();
                } else {
                    lineBuffer = '';
                    cursorPos = 0;
                    term.write('^C');
                    term.writeln('');
                    writePrompt();
                }
            }
            else if (code === 21) { lineBuffer = ''; cursorPos = 0; refreshLine(); } // Ctrl+U
            else if (code === 23) { // Ctrl+W
                const before = lineBuffer.slice(0, cursorPos).replace(/\S+\s*$/, '');
                lineBuffer = before + lineBuffer.slice(cursorPos);
                cursorPos = before.length;
                refreshLine();
            }
            else if (code === 1) { cursorPos = 0; refreshLine(); } // Ctrl+A
            else if (code === 5) { cursorPos = lineBuffer.length; refreshLine(); } // Ctrl+E
            else if (code >= 32) { // Printable
                lineBuffer = lineBuffer.slice(0, cursorPos) + ch + lineBuffer.slice(cursorPos);
                cursorPos++;
                refreshLine();
            }
        }
    });

    // ── Welcome ──
    term.writeln(`${C.blue}${C.bold}traits.build${C.reset} terminal`);
    if (wasm) {
        const info = JSON.parse(wasm.init());
        term.writeln(`${C.gray}${info.traits_registered} traits loaded (${info.wasm_callable} WASM). Type "help" for commands.${C.reset}`);
    } else {
        term.writeln(`${C.yellow}WASM kernel not loaded — commands unavailable${C.reset}`);
    }
    term.writeln('');
    writePrompt();

    return { term, fitAddon, wasm };
}
