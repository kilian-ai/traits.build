(function() {
  if (document.getElementById('_term-css')) return;
  var link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = 'https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.min.css';
  link.id = '_xterm-css';
  document.head.appendChild(link);
  var style = document.createElement('style');
  style.id = '_term-css';
  style.textContent = "/* \u2500\u2500 Shared Terminal Panel \u2500\u2500 */\n.terminal-wrap {\n    position: fixed;\n    bottom: 0;\n    left: 0;\n    right: 0;\n    z-index: 9999;\n    background: #0d1117;\n    border-top: 1px solid #30363d;\n}\n.terminal-header {\n    display: flex;\n    align-items: center;\n    gap: 1rem;\n    padding: 0.4rem 1rem;\n    background: #161b22;\n    cursor: pointer;\n    user-select: none;\n}\n.terminal-toggle {\n    background: none;\n    border: none;\n    color: #8b949e;\n    font-size: 0.85rem;\n    font-weight: 600;\n    cursor: pointer;\n    padding: 0;\n}\n.terminal-hint {\n    font-size: 0.75rem;\n    color: #484f58;\n}\n.terminal-status {\n    font-size: 0.7rem;\n    color: #484f58;\n    margin-left: auto;\n}\n.terminal-status.ready { color: #3fb950; }\n.terminal-status.loading { color: #d29922; }\n.terminal-status.error { color: #f85149; }\n.terminal-container {\n    height: 300px;\n    padding: 4px;\n    overflow: hidden;\n}\n.terminal-container.collapsed {\n    height: 0;\n    padding: 0;\n    overflow: hidden;\n}\n.xterm-mount {\n    height: 100%;\n}\n";
  document.head.appendChild(style);
})();
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

// ── ANSI helpers for terminal formatters ──
const _B = '\x1b[1m', _W = '\x1b[97m', _G = '\x1b[32m', _C = '\x1b[36m';
const _Y = '\x1b[33m', _R = '\x1b[31m', _D = '\x1b[90m', _0 = '\x1b[0m';

/** Format sys.info JSON response with ANSI colors for the terminal. */
function formatSystemStatus(info) {
    let o = `${_B}${_W}System Status${_0}\r\n\r\n`;
    const s = info.system;
    if (s) {
        o += `${_B}System${_0}\r\n`;
        o += `  ${_D}OS:${_0}      ${_C}${s.os||'?'}/${s.arch||'?'}${_0}\r\n`;
        o += `  ${_D}Build:${_0}   ${_C}${s.build_version||s.version||'?'}${_0}\r\n`;
    }
    const sv = info.server;
    if (sv) {
        o += `\r\n${_B}Server${_0}\r\n`;
        if (sv.bind === 'not running') {
            o += `  ${_D}Status:${_0}  ${_Y}not running${_0}\r\n`;
        } else {
            o += `  ${_D}Listen:${_0}  ${_G}${sv.bind}:${sv.port}${_0}\r\n`;
            o += `  ${_D}Uptime:${_0}  ${_C}${sv.uptime||'n/a'}${_0}\r\n`;
        }
    }
    const t = info.traits;
    if (t) {
        o += `\r\n${_B}Traits${_0}\r\n`;
        o += `  ${_D}Total:${_0}   ${_C}${t.total||0}${_0}\r\n`;
    }
    const r = info.relay;
    if (r) {
        o += `\r\n${_B}Relay${_0}\r\n`;
        if (r.enabled) {
            o += `  ${_D}URL:${_0}     ${_C}${r.url||'?'}${_0}\r\n`;
            if (r.code) o += `  ${_D}Code:${_0}    ${_G}${r.code}${_0}\r\n`;
            o += `  ${_D}Client:${_0}  ${r.client_connected ? `${_G}connected${_0}` : `${_Y}waiting${_0}`}\r\n`;
        } else {
            o += `  ${_D}Status:${_0}  ${_Y}disabled${_0} ${_D}(set RELAY_URL to enable)${_0}\r\n`;
        }
    }
    return o;
}

/** Format a REST dispatch result for terminal display. Returns null if no special formatter applies. */
function formatRestResult(traitPath, result) {
    if (traitPath === 'sys.info' && result && typeof result === 'object' && result.system) {
        return formatSystemStatus(result);
    }
    return null;
}

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
async function createTerminal(mountEl, opts = {}) {
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
                            const formatted = formatRestResult(p, res.result);
                            const text = formatted
                                || (typeof res.result === 'string'
                                    ? res.result
                                    : JSON.stringify(res.result, null, 2));
                            term.write(text.replace(/\n/g, '\r\n'));
                            if (!text.endsWith('\n') && !text.endsWith('\r\n')) term.write('\r\n');
                        } else if (res.error) {
                            term.write(`\x1b[31mError: ${res.error}\x1b[0m\r\n`);
                        }
                        term.write(PROMPT);
                    }).catch(e => {
                        term.write(`\x1b[31mDispatch error: ${e.message}\x1b[0m\r\n`);
                        term.write(PROMPT);
                    }).finally(() => { restPending = false; });
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
                            const text = typeof data.result === 'string'
                                ? data.result
                                : JSON.stringify(data.result, null, 2);
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
                    .finally(() => { restPending = false; });
                }
            } catch (e) {
                term.write(`\x1b[31mREST parse error: ${e.message}\x1b[0m\r\n`);
                term.write(PROMPT);
            }
            return;
        }

        if (output.includes(CLEAR_SENTINEL)) {
            term.clear();
            const rest = output.replaceAll(CLEAR_SENTINEL, '');
            if (rest) term.write(rest);
        } else {
            term.write(output);
        }
    });

    // ── Welcome ──
    if (wasm && wasm.cli_welcome) {
        term.write(wasm.cli_welcome());
    } else {
        term.writeln('\x1b[33mWASM kernel not loaded — commands unavailable\x1b[0m');
    }

    return { term, fitAddon, wasm };
}
if (typeof window !== "undefined") window.createTerminal = createTerminal;
