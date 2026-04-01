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
const WEBLLM_RE = /\x1b\[WEBLLM\]([\s\S]*?)\x1b\[\/WEBLLM\]/;
const VOICE_RE = /\x1b\[VOICE\]([\s\S]*?)\x1b\[\/VOICE\]/;
// Source of truth: kernel/cli/cli.rs PROMPT constant. Must stay in sync.
const PROMPT = '\x1b[32mtraits \x1b[0m';

const LS_SCROLLBACK = 'traits.terminal.scrollback';
const LS_HISTORY    = 'traits.terminal.history';
const LS_VFS        = 'traits.terminal.vfs';

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
async function createTerminal(mountEl, opts = {}) {
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
    let wasm = null;
    let backgroundCall = null;
    let activeSdk = window._traitsSDK || null;

    const saveState = () => {
        if (serializeAddon) {
            try { localStorage.setItem(LS_SCROLLBACK, serializeAddon.serialize()); } catch (_) {}
        }
        if (backgroundCall) {
            backgroundCall('cli_get_history').then(res => {
                if (res?.ok && typeof res.result === 'string') {
                    try { localStorage.setItem(LS_HISTORY, res.result); } catch (_) {}
                }
            }).catch(() => {});
            backgroundCall('vfs_dump').then(res => {
                if (res?.ok && typeof res.result === 'string') {
                    try { localStorage.setItem(LS_VFS, res.result); } catch (_) {}
                }
            }).catch(() => {});
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

    // ── Load background runtime (preferred: SDK adapter; fallback: direct WASM) ──
    try {
        if (activeSdk && typeof activeSdk.backgroundCall === 'function') {
            await activeSdk.initWorkerPool();
            backgroundCall = (cmd, payload = {}) => activeSdk.backgroundCall(cmd, payload);
            const status = activeSdk.status || {};
            setStatus('WASM worker', 'ready');
            // Register terminal as a service for sys.ps
            if (window.TraitsWasm && window.TraitsWasm.register_task) {
                try { window.TraitsWasm.register_task('terminal', 'Terminal', 'service', Date.now(), 'xterm.js CLI session'); } catch(e) {}
            }
            if (opts.onReady) opts.onReady({ wasm: null, traitCount: status.traits || 0, wasmCount: status.callable || 0, background: true });
        } else {
            // Fallback: attach WASM to a local SDK instance and route through sdk.background.direct.
            if (window.TraitsWasm && window.TraitsWasm.cli_input) {
                wasm = window.TraitsWasm;
                const count = wasm.is_registered ? JSON.parse(wasm.callable_traits()).length : 0;
                setStatus('WASM (SPA)', 'ready');
                if (opts.onReady) opts.onReady({ wasm, traitCount: 0, wasmCount: count, background: false });
            } else {
                const wasmJsUrl = '/wasm/traits_wasm.js';
                const wasmBinUrl = '/wasm/traits_wasm_bg.wasm';
                const mod = await import(wasmJsUrl);
                await mod.default(wasmBinUrl);
                const initResult = JSON.parse(mod.init());
                wasm = mod;
                const count = initResult.traits_registered || 0;
                const wasmCount = initResult.wasm_callable || 0;
                setStatus(`${count} traits (${wasmCount} WASM)`, 'ready');
                if (opts.onReady) opts.onReady({ wasm, traitCount: count, wasmCount, background: false });
            }

            if (window.Traits) {
                activeSdk = new window.Traits({
                    useWasm: false,
                    useHelper: false,
                    server: '',
                });
                activeSdk.attachWasm(wasm);
                activeSdk.setBackgroundBinding('sdk.background.direct');
                backgroundCall = (cmd, payload = {}) => activeSdk.backgroundCall(cmd, payload, { impl: 'sdk.background.direct' });
            }
            // Register terminal as a service for sys.ps (fallback path)
            if (wasm && wasm.register_task) {
                try { wasm.register_task('terminal', 'Terminal', 'service', Date.now(), 'xterm.js CLI session'); } catch(e) {}
            }
        }
    } catch (e) {
        setStatus('background failed', 'error');
        console.error('Background runtime load failed:', e);
    }

    // ── Input → WASM session → output (with REST fallback) ──
    let restPending = false;

    // ── WebLLM progress — show model loading status inline ──
    window.addEventListener('webllm-progress', (e) => {
        if (restPending && e.detail) {
            term.write(`\r\x1b[K\x1b[90m⏳ ${e.detail}\x1b[0m`);
        }
    });

    let ioChain = Promise.resolve();
    term.onData(data => {
        if (!backgroundCall || restPending) return;
        ioChain = ioChain.then(async () => {
            const inputRes = await backgroundCall('cli_input', { data });
            if (!inputRes?.ok) {
                term.write(`\x1b[31mCLI error: ${inputRes?.error || 'unknown'}\x1b[0m\r\n`);
                term.write(PROMPT);
                return;
            }
            const output = inputRes.result || '';
            if (!output) return;

            // Check for REST dispatch sentinel
            const restMatch = output.match(REST_RE);
            if (restMatch) {
            // Write visible part (loading message) without the sentinel
                const visible = output.replace(REST_RE, '');
                if (visible) term.write(visible);

            // Parse dispatch info and call via SDK cascade (WASM → helper → REST)
            // Supports @target routing: sentinel JSON may contain "t" field (rest/relay/helper/wasm)
            // Chat mode: "rp" = return prompt (instead of PROMPT), "sid" = session ID for VFS storage
                try {
                    const { p, a, t, rp, sid, stream: useStream } = JSON.parse(restMatch[1]);
                    const returnPrompt = rp || PROMPT;
                    restPending = true;
                    const callOpts = t ? { force: t } : {};
                    if (useStream) callOpts.stream = true;

                    // Helper: store assistant response in WASM VFS for chat history
                    const storeChatResponse = (text) => {
                        if (!sid || !backgroundCall) return;
                        const vfsKey = `chat/${sid}.json`;
                        backgroundCall('vfs_read', { path: vfsKey }).then(res => {
                            let msgs = [];
                            try { if (res?.ok && res.result) msgs = JSON.parse(res.result); } catch (_) {}
                            msgs.push({ role: 'assistant', content: text });
                            backgroundCall('vfs_write', { path: vfsKey, content: JSON.stringify(msgs) });
                        }).catch(() => {});
                    };

                    if (activeSdk) {
                        activeSdk.call(p, a, callOpts).then(async res => {
                        // Streaming path: consume async generator, write tokens to terminal
                        if (res.ok && res.stream) {
                            let streamStarted = false;
                            let fullText = '';
                            try {
                                for await (const chunk of res.stream) {
                                    const text = typeof chunk === 'string' ? chunk : (chunk?.result ?? JSON.stringify(chunk));
                                    if (!streamStarted) {
                                        term.write('\r\x1b[K'); // Clear "thinking…" line
                                        streamStarted = true;
                                    }
                                    term.write(text.replace(/\n/g, '\r\n'));
                                    fullText += text;
                                }
                            } catch (e) {
                                term.write(`\r\n\x1b[31mStream error: ${e.message}\x1b[0m\r\n`);
                            }
                            if (!streamStarted) term.write('\r\x1b[K'); // Clear "thinking…" if no chunks
                            if (fullText && !fullText.endsWith('\n')) term.write('\r\n');
                            storeChatResponse(fullText);
                            term.write(returnPrompt);
                            return;
                        }
                        // Non-streaming path (fallback)
                        term.write('\r\x1b[K'); // Clear progress line
                        if (res.ok && res.result !== undefined) {
                            // Try WASM CLI formatter first, fall back to JSON
                            let text = '';
                            const resultJson = typeof res.result === 'string'
                                ? JSON.stringify(res.result)
                                : JSON.stringify(res.result);
                            const fmt = await backgroundCall('cli_format_rest_result', {
                                path: p,
                                args_json: JSON.stringify(a),
                                result_json: resultJson,
                            });
                            if (fmt?.ok) {
                                text = fmt.result || '';
                            }
                            if (!text) {
                                text = typeof res.result === 'string'
                                    ? res.result
                                    : JSON.stringify(res.result, null, 2);
                            }
                            term.write(text.replace(/\n/g, '\r\n'));
                            if (!text.endsWith('\n')) term.write('\r\n');
                            storeChatResponse(text);
                        } else if (res.error) {
                            // Try WASM formatter with null result (local fallback)
                            let fallback = '';
                            const fmt = await backgroundCall('cli_format_rest_result', {
                                path: p,
                                args_json: JSON.stringify(a),
                                result_json: 'null',
                            });
                            if (fmt?.ok) {
                                fallback = fmt.result || '';
                            }
                            if (fallback) {
                                term.write(fallback.replace(/\n/g, '\r\n'));
                                if (!fallback.endsWith('\n')) term.write('\r\n');
                            } else {
                                term.write(`\x1b[31mError: ${res.error}\x1b[0m\r\n`);
                            }
                        }
                        term.write(returnPrompt);
                        }).catch(e => {
                            term.write(`\x1b[31mDispatch error: ${e.message}\x1b[0m\r\n`);
                            term.write(returnPrompt);
                        }).finally(() => { restPending = false; requestAnimationFrame(saveState); });
                    } else {
                    // Last-resort REST fallback (SDK unavailable)
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
                            storeChatResponse(text);
                        } else if (data.error) {
                            term.write(`\x1b[31mError: ${data.error}\x1b[0m\r\n`);
                        }
                        term.write(returnPrompt);
                    })
                    .catch(e => {
                        term.write(`\x1b[31mREST error: ${e.message}\x1b[0m\r\n`);
                        term.write(returnPrompt);
                    })
                    .finally(() => { restPending = false; requestAnimationFrame(saveState); });
                    }
                } catch (e) {
                    term.write(`\x1b[31mREST parse error: ${e.message}\x1b[0m\r\n`);
                    term.write(PROMPT);
                    restPending = false;
                    requestAnimationFrame(saveState);
                }
                return;
            }

            // Check for WebLLM dispatch sentinel
            const webllmMatch = output.match(WEBLLM_RE);
            if (webllmMatch && activeSdk) {
                const visible = output.replace(WEBLLM_RE, '');
                if (visible) term.write(visible);
                try {
                    const { prompt, model } = JSON.parse(webllmMatch[1]);
                    restPending = true;
                    let streamStarted = false;
                    const onToken = (text) => {
                        if (!streamStarted) {
                            term.write('\r\x1b[K'); // Clear progress line on first token
                            streamStarted = true;
                        }
                        term.write(text.replace(/\n/g, '\r\n'));
                    };
                    activeSdk._callWebLLM(prompt, model, onToken).then(res => {
                        if (streamStarted) {
                            // Streaming completed — just add newline + prompt
                            if (res.ok) {
                                const text = typeof res.result === 'string' ? res.result : '';
                                if (!text.endsWith('\n')) term.write('\r\n');
                            } else if (res.error) {
                                term.write(`\r\n\x1b[31mWebLLM: ${res.error}\x1b[0m\r\n`);
                            }
                        } else {
                            // No tokens streamed (non-streaming fallback or empty result)
                            term.write('\r\x1b[K');
                            if (res.ok && res.result !== undefined) {
                                const text = typeof res.result === 'string'
                                    ? res.result : JSON.stringify(res.result, null, 2);
                                term.write(text.replace(/\n/g, '\r\n'));
                                if (!text.endsWith('\n')) term.write('\r\n');
                            } else if (res.error) {
                                term.write(`\x1b[31mWebLLM: ${res.error}\x1b[0m\r\n`);
                            } else {
                                term.write('\x1b[33mWebLLM returned empty result\x1b[0m\r\n');
                            }
                        }
                        term.write(PROMPT);
                    }).catch(e => {
                        console.error('[terminal] WebLLM dispatch error:', e);
                        term.write(`\r\x1b[K\x1b[31mWebLLM error: ${e.message || e}\x1b[0m\r\n`);
                        term.write(PROMPT);
                    }).finally(() => { restPending = false; requestAnimationFrame(saveState); });
                } catch (e) {
                    term.write(`\x1b[31mWebLLM parse error: ${e.message}\x1b[0m\r\n`);
                    term.write(PROMPT);
                    restPending = false;
                    requestAnimationFrame(saveState);
                }
                return;
            }

            // Check for Voice dispatch sentinel
            const voiceMatch = output.match(VOICE_RE);
            if (voiceMatch) {
                const visible = output.replace(VOICE_RE, '');
                if (visible) term.write(visible);
                try {
                    const { v: voiceName, m: model, a: agent, s: sessionId, rp: returnPrompt, local: localFlag, voxtral: voxtralFlag } = JSON.parse(voiceMatch[1]);
                    restPending = true;

                    // Check if helper is connected (required for native voice with sox)
                    const helperConnected = activeSdk && (activeSdk.helperConnected || activeSdk.helperUrl);
                    
                    // Check for browser voice support (WebAudio + getUserMedia)
                    const browserVoiceSupported = typeof navigator !== 'undefined' && 
                        navigator.mediaDevices && navigator.mediaDevices.getUserMedia;

                    // Check WebGPU support for local voice
                    const webgpuAvailable = typeof navigator !== 'undefined' && !!navigator.gpu;

                    if (!helperConnected && !browserVoiceSupported) {
                        term.write(`\r\n\x1b[33mVoice requires either:\x1b[0m\r\n`);
                        term.write(`  1. A local helper (native sox) — run traits CLI\r\n`);
                        term.write(`  2. Browser voice support — use Chrome/Edge/Safari\r\n`);
                        term.write(returnPrompt);
                        restPending = false;
                        requestAnimationFrame(saveState);
                        return;
                    }

                    // Detect whether to use local voice (WebGPU STT+LLM+TTS) or cloud voice (OpenAI Realtime)
                    // Priority:
                    //   1. Explicit local:true flag in sentinel → always local
                    //   2. User preference localStorage['traits.voice.mode'] = 'realtime' + API key → cloud
                    //   3. User preference localStorage['traits.voice.mode'] = 'local' → local
                    //   4. Auto-fallback: no API key + WebGPU available → local
                    let useLocalVoice = !!localFlag;
                    let useVoxtralVoice = !!voxtralFlag;
                    let hasApiKey = false;
                    try {
                        const settingsKey = (localStorage.getItem('traits.secret.OPENAI_API_KEY') || '').trim();
                        const legacyKey = (localStorage.getItem('traits.voice.api_key') || '').trim();
                        hasApiKey = !!(settingsKey || legacyKey);
                    } catch(_) {}

                    if (!useLocalVoice && !useVoxtralVoice) {
                        // Check stored voice mode preference
                        const storedMode = (localStorage.getItem('traits.voice.mode') || '').trim();
                        if (storedMode === 'realtime' && hasApiKey) {
                            useLocalVoice = false; // explicitly cloud
                        } else if (storedMode === 'local') {
                            useLocalVoice = true;
                        } else if (storedMode === 'local-realtime') {
                            useVoxtralVoice = true;
                        } else if (!helperConnected && browserVoiceSupported && webgpuAvailable && !hasApiKey) {
                            useLocalVoice = true; // auto-fallback
                        }
                    }

                    // ── Voxtral local-realtime mode (Voxtral ONNX STT → LLM → Kokoro TTS) ──
                    if (useVoxtralVoice && browserVoiceSupported) {
                        term.write(`\x1b[90mStarting Voxtral local-realtime voice…\x1b[0m\r\n`);
                        term.write(`\x1b[90mFirst run downloads ~1.5 GB Voxtral model + ~92 MB TTS.\x1b[0m\r\n`);

                        activeSdk.startVoxtralVoice({
                            voice: voiceName || 'af_heart',
                            instructions: agent
                                ? `You are the "${agent}" agent on traits.build. Keep responses very short (1-2 sentences). Be conversational.`
                                : undefined,
                            onTranscript: (text) => {
                                term.write(`\r\n\x1b[92m🎤 ${text}\x1b[0m\r\n`);
                            },
                            onResponse: (text) => {
                                term.write(`\x1b[96m💬 ${text}\x1b[0m\r\n`);
                            },
                            onToolCall: (name, args) => {
                                term.write(`\x1b[93m⚡ ${name.replace(/_/g, '.')}\x1b[0m\r\n`);
                            },
                            onToolResult: (name, resultStr) => {
                                if (name === 'sys_echo') {
                                    try {
                                        const r = JSON.parse(resultStr);
                                        const text = r.text || r.result?.text || '';
                                        if (text) term.write(`\x1b[97m📋 ${text}\x1b[0m\r\n`);
                                    } catch(_) {}
                                }
                            },
                            onProgress: (text) => {
                                if (text) term.write(`\r\x1b[K\x1b[90m⏳ ${text}\x1b[0m`);
                            },
                            onError: (msg) => {
                                term.write(`\r\n\x1b[31mVoxtral voice error: ${msg}\x1b[0m\r\n`);
                            },
                        }).then(result => {
                            if (result.ok) {
                                const toolMsg = result.tools ? `, ${result.tools} tools` : '';
                                term.write(`\r\x1b[K\x1b[90mVoxtral voice active! Speak to start${toolMsg}. Press Esc to stop.\x1b[0m\r\n`);
                                const onVoiceStopped = (e) => {
                                    if (e.detail && e.detail.type === 'stopped') {
                                        window.removeEventListener('voice-event', onVoiceStopped);
                                        term.write(`\r\n\x1b[90mVoxtral voice session ended.\x1b[0m\r\n`);
                                        term.write(returnPrompt);
                                        restPending = false;
                                        requestAnimationFrame(saveState);
                                    }
                                };
                                window.addEventListener('voice-event', onVoiceStopped);
                                const stopHandler = (data) => {
                                    if (data === '\x1b' || data === '\x03') {
                                        activeSdk.stopVoxtralVoice().then(() => {
                                            window.removeEventListener('voice-event', onVoiceStopped);
                                            term.write(`\r\n\x1b[90mVoxtral voice stopped.\x1b[0m\r\n`);
                                            term.write(returnPrompt);
                                            restPending = false;
                                            requestAnimationFrame(saveState);
                                        });
                                        term.offData(stopHandler);
                                    }
                                };
                                term.onData(stopHandler);
                            } else {
                                term.write(`\r\n\x1b[31mVoxtral voice error: ${result.error}\x1b[0m\r\n`);
                                term.write(returnPrompt);
                                restPending = false;
                                requestAnimationFrame(saveState);
                            }
                        });
                        return;
                    }

                    // ── Local voice mode (WebGPU: Whisper STT → LLM → Kokoro TTS) ──
                    if (useLocalVoice && browserVoiceSupported && webgpuAvailable) {
                        term.write(`\x1b[90mStarting local voice…\x1b[0m\r\n`);
                        term.write(`\x1b[90mFirst run downloads ~250 MB of AI models.\x1b[0m\r\n`);

                        activeSdk.startLocalVoice({
                            voice: voiceName || 'af_heart',
                            language: 'en',
                            instructions: agent
                                ? `You are the "${agent}" agent on traits.build. Keep responses very short (1-2 sentences). Be conversational.`
                                : undefined,
                            onTranscript: (text) => {
                                term.write(`\r\n\x1b[92m🎤 ${text}\x1b[0m\r\n`);
                            },
                            onResponse: (text) => {
                                term.write(`\x1b[96m💬 ${text}\x1b[0m\r\n`);
                            },
                            onToolCall: (name, args) => {
                                term.write(`\x1b[93m⚡ ${name.replace(/_/g, '.')}\x1b[0m\r\n`);
                            },
                            onToolResult: (name, resultStr) => {
                                // sys.echo: display the echoed text prominently
                                if (name === 'sys_echo') {
                                    try {
                                        const r = JSON.parse(resultStr);
                                        const text = r.text || r.result?.text || '';
                                        if (text) term.write(`\x1b[97m📋 ${text}\x1b[0m\r\n`);
                                    } catch(_) {}
                                }
                                // sys.canvas: show brief confirmation in terminal
                                if (name === 'sys_canvas') {
                                    try {
                                        const r = JSON.parse(resultStr);
                                        const act = r.action || r.result?.action || '';
                                        if (act) term.write(`\x1b[96m🎨 canvas ${act} (${r.length || r.result?.length || 0} bytes)\x1b[0m\r\n`);
                                    } catch(_) {}
                                }
                            },
                            onProgress: (text) => {
                                if (text) term.write(`\r\x1b[K\x1b[90m⏳ ${text}\x1b[0m`);
                            },
                            onError: (msg) => {
                                term.write(`\r\n\x1b[31mLocal voice error: ${msg}\x1b[0m\r\n`);
                            },
                        }).then(result => {
                            if (result.ok) {
                                const toolMsg = result.tools ? `, ${result.tools} tools` : '';
                                term.write(`\r\x1b[K\x1b[90mLocal voice active! Speak to start${toolMsg}. Press Esc to stop.\x1b[0m\r\n`);
                                // Listen for voice-event 'stopped'
                                const onVoiceStopped = (e) => {
                                    if (e.detail && e.detail.type === 'stopped') {
                                        window.removeEventListener('voice-event', onVoiceStopped);
                                        term.write(`\r\n\x1b[90mLocal voice session ended.\x1b[0m\r\n`);
                                        term.write(returnPrompt);
                                        restPending = false;
                                        requestAnimationFrame(saveState);
                                    }
                                };
                                window.addEventListener('voice-event', onVoiceStopped);
                                // Esc key handler
                                const stopHandler = (data) => {
                                    if (data === '\x1b' || data === '\x03') {
                                        activeSdk.stopLocalVoice().then(() => {
                                            window.removeEventListener('voice-event', onVoiceStopped);
                                            term.write(`\r\n\x1b[90mLocal voice stopped.\x1b[0m\r\n`);
                                            term.write(returnPrompt);
                                            restPending = false;
                                            requestAnimationFrame(saveState);
                                        });
                                        term.offData(stopHandler);
                                    }
                                };
                                term.onData(stopHandler);
                            } else {
                                term.write(`\r\n\x1b[31mLocal voice error: ${result.error}\x1b[0m\r\n`);
                                term.write(returnPrompt);
                                restPending = false;
                                requestAnimationFrame(saveState);
                            }
                        });
                        return;
                    }

                    // ── Cloud voice mode (OpenAI Realtime via WebRTC) ──
                    if (!helperConnected && browserVoiceSupported) {
                        term.write(`\x1b[90mStarting browser voice with ${voiceName}…\x1b[0m\r\n`);
                        activeSdk.startVoice({
                            voice: voiceName,
                            model: model || 'gpt-realtime-mini-2025-12-15',
                            agent: agent || '',
                            onTranscript: (text) => {
                                term.write(`\r\n\x1b[92m🎤 ${text}\x1b[0m\r\n`);
                            },
                            onResponse: (text) => {
                                term.write(`\x1b[96m💬 ${text}\x1b[0m\r\n`);
                            },
                            onToolCall: (name, args) => {
                                term.write(`\x1b[93m⚡ ${name.replace(/_/g, '.')}\x1b[0m\r\n`);
                            },
                            onToolResult: (name, resultStr) => {
                                // sys.echo: display the echoed text prominently
                                if (name === 'sys_echo') {
                                    try {
                                        const r = JSON.parse(resultStr);
                                        const text = r.text || r.result?.text || '';
                                        if (text) term.write(`\x1b[97m📋 ${text}\x1b[0m\r\n`);
                                    } catch(_) {}
                                }
                                // sys.canvas: show brief confirmation in terminal
                                if (name === 'sys_canvas') {
                                    try {
                                        const r = JSON.parse(resultStr);
                                        const act = r.action || r.result?.action || '';
                                        if (act) term.write(`\x1b[96m🎨 canvas ${act} (${r.length || r.result?.length || 0} bytes)\x1b[0m\r\n`);
                                    } catch(_) {}
                                }
                            },
                            onError: (msg) => {
                                term.write(`\x1b[31mVoice error: ${msg}\x1b[0m\r\n`);
                            },
                        }).then(result => {
                            if (result.ok) {
                                const toolMsg = result.tools ? `, ${result.tools} tools` : '';
                                term.write(`\x1b[90mVoice active! Speak to start conversation${toolMsg}. Press Esc to stop.\x1b[0m\r\n`);
                                // Listen for voice-event 'stopped' (model quit or disconnect)
                                const onVoiceStopped = (e) => {
                                    if (e.detail && e.detail.type === 'stopped') {
                                        window.removeEventListener('voice-event', onVoiceStopped);
                                        term.write(`\r\n\x1b[90mVoice session ended.\x1b[0m\r\n`);
                                        term.write(returnPrompt);
                                        restPending = false;
                                        requestAnimationFrame(saveState);
                                    }
                                };
                                window.addEventListener('voice-event', onVoiceStopped);
                                // Setup Esc key handler to stop voice
                                const stopVoiceHandler = (data) => {
                                    // Esc = \x1b (alone, not followed by [ which is an arrow key)
                                    if (data === '\x1b' || data === '\x03') {
                                        activeSdk.stopVoice().then(() => {
                                            window.removeEventListener('voice-event', onVoiceStopped);
                                            term.write(`\r\n\x1b[90mVoice stopped.\x1b[0m\r\n`);
                                            term.write(returnPrompt);
                                            restPending = false;
                                            requestAnimationFrame(saveState);
                                        });
                                        term.offData(stopVoiceHandler);
                                    }
                                };
                                term.onData(stopVoiceHandler);
                            } else {
                                term.write(`\x1b[31mVoice error: ${result.error}\x1b[0m\r\n`);
                                term.write(returnPrompt);
                                restPending = false;
                                requestAnimationFrame(saveState);
                            }
                        });
                        return;
                    }

                    // Helper is connected - dispatch native voice call
                    term.write(`\x1b[90mStarting voice with ${voiceName}…\x1b[0m\r\n`);
                    const args = [voiceName, model || 'gpt-realtime-mini-2025-12-15', agent || '', sessionId || ''];
                    activeSdk.call('sys.voice', args).then(res => {
                        term.write('\r\x1b[K');
                        if (res.ok && res.result !== undefined) {
                            const text = typeof res.result === 'string' ? res.result : JSON.stringify(res.result, null, 2);
                            term.write(text.replace(/\n/g, '\r\n'));
                            if (!text.endsWith('\n')) term.write('\r\n');
                        } else if (res.error) {
                            term.write(`\x1b[31mVoice error: ${res.error}\x1b[0m\r\n`);
                        }
                        term.write(returnPrompt);
                    }).catch(e => {
                        term.write(`\x1b[31mVoice dispatch error: ${e.message}\x1b[0m\r\n`);
                        term.write(returnPrompt);
                    }).finally(() => { restPending = false; requestAnimationFrame(saveState); });
                } catch (e) {
                    term.write(`\x1b[31mVoice parse error: ${e.message}\x1b[0m\r\n`);
                    term.write(PROMPT);
                    restPending = false;
                    requestAnimationFrame(saveState);
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
                if (data.includes('\r') || data.includes('\n')) requestAnimationFrame(saveState);
            }
        }).catch(e => {
            term.write(`\x1b[31mTerminal IO error: ${e.message}\x1b[0m\r\n`);
            term.write(PROMPT);
        });
    });

    // ── External terminal input (sys.spa "terminal" action) ──
    window.addEventListener('traits-terminal-input', (e) => {
        const text = e.detail?.data;
        if (!text || !backgroundCall || restPending) return;
        ioChain = ioChain.then(async () => {
            const inputRes = await backgroundCall('cli_input', { data: text });
            if (!inputRes?.ok) return;
            const output = inputRes.result || '';
            if (output) term.write(output);
        }).catch(e => {
            console.error('[terminal] external input error:', e);
        });
    });

    // ── Restore history + VFS into WASM session ──
    const savedHistory = localStorage.getItem(LS_HISTORY);
    if (savedHistory && backgroundCall) {
        try { await backgroundCall('cli_set_history', { history_json: savedHistory }); } catch (_) {}
    }
    const savedVfs = localStorage.getItem(LS_VFS);
    if (savedVfs && backgroundCall) {
        try { await backgroundCall('vfs_load', { json: savedVfs }); } catch (_) {}
    }

    // ── Restore scrollback or show welcome ──
    const savedScrollback = localStorage.getItem(LS_SCROLLBACK);
    if (savedScrollback) {
        term.write(savedScrollback);
    } else if (backgroundCall) {
        const welcome = await backgroundCall('cli_welcome');
        if (welcome?.ok && welcome.result) {
            term.write(welcome.result);
        } else {
            term.writeln('\x1b[33mWASM kernel not loaded — commands unavailable\x1b[0m');
        }
    } else {
        term.writeln('\x1b[33mWASM kernel not loaded — commands unavailable\x1b[0m');
    }

    return { term, fitAddon, wasm };
}
if (typeof window !== "undefined") window.createTerminal = createTerminal;
