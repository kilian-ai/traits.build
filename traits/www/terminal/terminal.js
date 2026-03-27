// ═══════════════════════════════════════════
// ── Shared WASM-powered Terminal ──
// Thin display layer: all line editing, history,
// tab completion, and interactive mode live in the
// WASM kernel (kernel/cli CliSession).
// JS just pipes xterm.js data ↔ wasm.cli_input().
// ═══════════════════════════════════════════

const CLEAR_SENTINEL = '\x1b[CLEAR]';
const REST_RE = /\x1b\[REST\]([\s\S]*?)\x1b\[\/REST\]/;
const PROMPT = '\x1b[32mtraits \x1b[0m';

const LS_SCROLLBACK = 'traits.terminal.scrollback';
const LS_HISTORY    = 'traits.terminal.history';

let Terminal, FitAddon, WebLinksAddon, SerializeAddon;

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
        const serMod = await import('https://cdn.jsdelivr.net/npm/@xterm/addon-serialize@0.13/+esm');
        SerializeAddon = serMod.SerializeAddon;
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
    const serializeAddon = SerializeAddon ? new SerializeAddon() : null;
    term.loadAddon(fitAddon);
    if (serializeAddon) term.loadAddon(serializeAddon);
    term.loadAddon(new WebLinksAddon());
    term.open(mountEl);
    fitAddon.fit();

    // ── Persist scrollback + history to localStorage ──
    const saveState = () => {
        if (serializeAddon) {
            try { localStorage.setItem(LS_SCROLLBACK, serializeAddon.serialize()); } catch (_) {}
        }
        if (wasm && wasm.cli_get_history) {
            try { localStorage.setItem(LS_HISTORY, wasm.cli_get_history()); } catch (_) {}
        }
    };
    window.addEventListener('pagehide', saveState);
    window.addEventListener('hashchange', saveState);

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
        // Reuse WASM already loaded by SPA shell (window.TraitsWasm from wasm-runtime.js)
        if (window.TraitsWasm && window.TraitsWasm.cli_input) {
            wasm = window.TraitsWasm;
            const count = wasm.is_registered ? JSON.parse(wasm.callable_traits()).length : 0;
            setStatus(`WASM (SPA)`, 'ready');
            if (opts.onReady) opts.onReady({ wasm, traitCount: 0, wasmCount: count });
        } else {
            // Standalone mode — load WASM from server
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
        }
    } catch (e) {
        setStatus('WASM failed', 'error');
        console.error('WASM load failed:', e);
    }

    // ── Input → WASM session → output (with REST fallback) ──
    let restPending = false;

    // ── WebLLM progress — show model loading status inline ──
    window.addEventListener('webllm-progress', (e) => {
        if (restPending && e.detail) {
            term.write(`\r\x1b[K\x1b[90m⏳ ${e.detail}\x1b[0m`);
        }
    });

    term.onData(data => {
        if (!wasm || !wasm.cli_input) return;
        if (restPending) return; // Block input during REST calls

        const output = wasm.cli_input(data);
        if (!output) return;

        // Check for REST dispatch sentinel
        const restMatch = output.match(REST_RE);
        if (restMatch) {
            // Write visible part (loading message) without the sentinel
            const visible = output.replace(REST_RE, '');
            if (visible) term.write(visible);

            // Parse dispatch info and call via SDK cascade (WASM → helper → REST)
            try {
                const { p, a } = JSON.parse(restMatch[1]);
                restPending = true;

                const traitsSdk = window._traitsSDK;
                if (traitsSdk) {
                    traitsSdk.call(p, a).then(res => {
                        term.write('\r\x1b[K'); // Clear progress line
                        if (res.ok && res.result !== undefined) {
                            // Try WASM CLI formatter first, fall back to JSON
                            let text = '';
                            if (wasm && wasm.cli_format_rest_result) {
                                const resultJson = typeof res.result === 'string'
                                    ? JSON.stringify(res.result)
                                    : JSON.stringify(res.result);
                                text = wasm.cli_format_rest_result(p, JSON.stringify(a), resultJson);
                            }
                            if (!text) {
                                text = typeof res.result === 'string'
                                    ? res.result
                                    : JSON.stringify(res.result, null, 2);
                            }
                            term.write(text.replace(/\n/g, '\r\n'));
                            if (!text.endsWith('\n')) term.write('\r\n');
                        } else if (res.error) {
                            // Try WASM formatter with null result (local fallback)
                            let fallback = '';
                            if (wasm && wasm.cli_format_rest_result) {
                                fallback = wasm.cli_format_rest_result(p, JSON.stringify(a), 'null');
                            }
                            if (fallback) {
                                term.write(fallback.replace(/\n/g, '\r\n'));
                                if (!fallback.endsWith('\n')) term.write('\r\n');
                            } else {
                                term.write(`\x1b[31mError: ${res.error}\x1b[0m\r\n`);
                            }
                        }
                        term.write(PROMPT);
                    }).catch(e => {
                        term.write(`\x1b[31mDispatch error: ${e.message}\x1b[0m\r\n`);
                        term.write(PROMPT);
                    }).finally(() => { restPending = false; saveState(); });
                } else {
                    // Fallback: direct REST call (when not in SPA)
                    const restPath = p.replace(/\./g, '/');
                    fetch(`/traits/${restPath}`, {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ args: a }),
                    })
                    .then(r => r.json())
                    .then(data => {
                        if (data.result !== undefined) {
                            let text = '';
                            if (wasm && wasm.cli_format_rest_result) {
                                text = wasm.cli_format_rest_result(p, JSON.stringify(a),
                                    JSON.stringify(data.result));
                            }
                            if (!text) {
                                text = typeof data.result === 'string'
                                    ? data.result
                                    : JSON.stringify(data.result, null, 2);
                            }
                            term.write(text.replace(/\n/g, '\r\n'));
                            if (!text.endsWith('\n')) term.write('\r\n');
                        } else if (data.error) {
                            term.write(`\x1b[31mError: ${data.error}\x1b[0m\r\n`);
                        }
                        term.write(PROMPT);
                    })
                    .catch(e => {
                        term.write(`\x1b[31mREST error: ${e.message}\x1b[0m\r\n`);
                        term.write(PROMPT);
                    })
                    .finally(() => { restPending = false; saveState(); });
                }
            } catch (e) {
                term.write(`\x1b[31mREST parse error: ${e.message}\x1b[0m\r\n`);
                term.write(PROMPT);
                restPending = false;
                saveState();
            }
            return;
        }

        if (output.includes(CLEAR_SENTINEL)) {
            term.clear();
            const rest = output.replaceAll(CLEAR_SENTINEL, '');
            if (rest) term.write(rest);
            try { localStorage.removeItem(LS_SCROLLBACK); } catch (_) {}
        } else {
            term.write(output);
            // Save after a command completes (output contains newline from Enter)
            if (data.includes('\r') || data.includes('\n')) saveState();
        }
    });

    // ── Restore history into WASM session ──
    const savedHistory = localStorage.getItem(LS_HISTORY);
    if (savedHistory && wasm && wasm.cli_set_history) {
        try { wasm.cli_set_history(savedHistory); } catch (_) {}
    }

    // ── Restore scrollback or show welcome ──
    const savedScrollback = localStorage.getItem(LS_SCROLLBACK);
    if (savedScrollback) {
        term.write(savedScrollback);
    } else if (wasm && wasm.cli_welcome) {
        term.write(wasm.cli_welcome());
    } else {
        term.writeln('\x1b[33mWASM kernel not loaded — commands unavailable\x1b[0m');
    }

    return { term, fitAddon, wasm };
}
