// SPDX-License-Identifier: GPL-2.0-only (runtime js from kilian-ai/linux-wasm, forked from joelseverin/linux-wasm)
use serde_json::Value;
use maud::{html, DOCTYPE, PreEscaped};

const LINUX_JS: &str = include_str!("linux.js");
const LINUX_WORKER_JS: &str = include_str!("linux-worker.js");
const NET_PROXY_JS: &str = include_str!("net-proxy.js");

pub fn linux_page(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Linux / WASM — traits.build" }
                link rel="stylesheet"
                    href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.min.css";
                style { (PreEscaped(LINUX_CSS)) }
            }
            body {
                div #linux-root {
                    // ── Header bar ──
                    div #linux-header {
                        div .linux-title {
                            span .accent { "Linux" }
                            " / WASM"
                        }
                        div .linux-meta {
                            span #kernel-badge .badge { "vmlinux 6.4.16" }
                            span .badge { "BusyBox 1.36.1" }
                            span .badge { "musl 1.2.5" }
                        }
                        div .linux-actions {
                            button #btn-reboot title="Reboot Linux (reload page)" { "⟳ Reboot" }
                        }
                    }

                    // ── Status / loading overlay ──
                    div #linux-status {
                        div .status-inner {
                            div .spinner {}
                            p #status-text { "Initializing…" }
                            div #progress-bar { div #progress-fill {} }
                            p #status-note .note {}
                        }
                    }

                    // ── Terminal container ──
                    div #xterm-wrap {
                        div #xterm-container {}
                    }
                }

                // ── Embedded linux-worker.js source (read as text, used for Blob worker) ──
                (PreEscaped(format!(
                    r#"<script id="linux-worker-src" type="text/plain">{}</script>"#,
                    LINUX_WORKER_JS
                )))

                // ── Embedded net-proxy.js (TCP/IP proxy for guest networking) ──
                (PreEscaped(format!(
                    "<script>\n{}\n</script>",
                    NET_PROXY_JS
                )))

                // ── Embedded linux.js runtime + boot logic ──
                (PreEscaped(format!(
                    "<script>\n{}\n{}\n</script>",
                    LINUX_JS,
                    BOOT_SCRIPT
                )))
            }
        }
    };
    Value::String(markup.into_string())
}

// ── Styles ──────────────────────────────────────────────────────────────────
const LINUX_CSS: &str = r#"
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

body {
    background: #000;
    color: #e0e5ef;
    font-family: 'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace;
    overflow: hidden;
}

#linux-root {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    z-index: 9999;
    display: flex;
    flex-direction: column;
    background: #0a0e1a;
}

/* ── Header ── */
#linux-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    background: #0d1117;
    border-bottom: 1px solid #1a2640;
    flex-shrink: 0;
    height: 44px;
}

.linux-title {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.5px;
    color: #c9d1d9;
}

.linux-title .accent { color: #00c07f; }

.linux-meta { display: flex; gap: 6px; flex: 1; }

.badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 10px;
    background: #161b22;
    border: 1px solid #21262d;
    color: #8b949e;
}

.linux-actions { display: flex; gap: 6px; }

.linux-actions button {
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 4px;
    background: transparent;
    border: 1px solid #21262d;
    color: #8b949e;
    cursor: pointer;
    font-family: inherit;
}
.linux-actions button:hover { border-color: #00c07f; color: #00c07f; }

/* ── Status overlay ── */
#linux-status {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #0a0e1a;
}

#linux-status.hidden { display: none; }

.status-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    max-width: 360px;
}

.spinner {
    width: 36px; height: 36px;
    border: 3px solid #1a2640;
    border-top-color: #00c07f;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

#status-text { font-size: 14px; color: #8b949e; text-align: center; }
#status-note  { font-size: 12px; color: #484f58; text-align: center; min-height: 20px; }

#progress-bar {
    width: 280px;
    height: 4px;
    background: #161b22;
    border-radius: 2px;
    overflow: hidden;
}
#progress-fill {
    height: 100%;
    width: 0%;
    background: #00c07f;
    border-radius: 2px;
    transition: width 0.3s ease;
}

/* ── Terminal ── */
#xterm-wrap {
    flex: 1;
    display: none;
    overflow: hidden;
    padding: 4px;
    background: #0a0e1a;
}

#xterm-wrap.visible { display: flex; }

#xterm-container {
    flex: 1;
    width: 100%;
    height: 100%;
}

/* Xterm theme overrides */
.xterm-viewport { background: transparent !important; }
"#;

// ── Boot Script ─────────────────────────────────────────────────────────────
const BOOT_SCRIPT: &str = r#"
// ─────────────────────────────────────────────────────────────────────────────
// Linux/WASM Boot Script
// Adapts kilian-ai/linux-wasm runtime into the traits.build SPA.
// Fetches vmlinux.wasm + initramfs.cpio.gz from the kilian-ai CDN (includes QuickJS).
// ─────────────────────────────────────────────────────────────────────────────

(async function bootLinuxWasm() {
    const CDN = 'https://kilian-ai.github.io/linux-wasm';
    const ASSET_REV = '298df31';
    const assetUrl = (name) => `${CDN}/${name}?rev=${ASSET_REV}`;
    const COI_SW_RELOAD_KEY = 'linux-wasm-coi-v1';

    // ── Shell history persistence (survives page refresh via localStorage) ──
    const HIST_KEY = 'linux-wasm-history';
    const HIST_MAX = 500;
    let savedHistory = [];
    try { savedHistory = JSON.parse(localStorage.getItem(HIST_KEY) || '[]'); } catch(e) {}
    if (!Array.isArray(savedHistory)) savedHistory = [];
    // Drop malformed/oversized entries and old bootstrap-injection artifacts.
    savedHistory = savedHistory
        .filter((c) => typeof c === 'string' && c.trim() && c.length <= 400)
        .filter((c) => {
            const s = String(c).replace(/\s+/g, ' ').trim();
            // Drop legacy boot-time history injection command (all known variants).
            if (s.includes('export HISTFILE=/home/.history')) return false;
            if (s.startsWith('mkdir -p /home &&')) return false;
            if (s.includes("printf '%s\\n'")) return false;
            return true;
        })
        .slice(-HIST_MAX);
    try { localStorage.setItem(HIST_KEY, JSON.stringify(savedHistory)); } catch(e) {}
    let historyNavIndex = null;
    let currentInput = '';  // Tracks typed characters for agent interception
                            // (xterm buffer unreliable due to net stats output)

    const statusEl  = document.getElementById('linux-status');
    const statusTxt = document.getElementById('status-text');
    const statusNote = document.getElementById('status-note');
    const progressFill = document.getElementById('progress-fill');
    const xtermWrap = document.getElementById('xterm-wrap');
    const xtermContainer = document.getElementById('xterm-container');

    function setStatus(msg, note) {
        if (statusTxt) statusTxt.textContent = msg;
        if (statusNote) statusNote.textContent = note || '';
    }

    function setProgress(pct) {
        if (progressFill) progressFill.style.width = Math.min(100, Math.max(0, pct)) + '%';
    }

    function showStatus()   { statusEl && (statusEl.className = ''); }
    function hideStatus()   { statusEl && statusEl.classList.add('hidden'); }
    function showTerminal() { xtermWrap && xtermWrap.classList.add('visible'); }

    function setError(msg) {
        setStatus(msg);
        if (progressFill) {
            progressFill.style.width = '100%';
            progressFill.style.background = '#e74c3c';
        }
        const spinner = document.querySelector('.spinner');
        if (spinner) spinner.style.display = 'none';
    }

    // ── Step 1: Ensure cross-origin isolation (needed for SharedArrayBuffer) ──
    async function ensureCOI() {
        if (crossOriginIsolated) return true;  // already good

        if (!('serviceWorker' in navigator)) {
            setError('SharedArrayBuffer requires a modern browser with Service Worker support.');
            return false;
        }

        // If we already reloaded once and still not isolated, give up.
        if (sessionStorage.getItem(COI_SW_RELOAD_KEY) === '1') {
            setError('⚠ Cross-origin isolation failed. Try opening this page in Chrome or Firefox (latest).');
            return false;
        }

        setStatus('Enabling cross-origin isolation… (page will reload once)');
        setProgress(5);

        try {
            await navigator.serviceWorker.register('/coi-sw.js', { scope: '/' });
            await navigator.serviceWorker.ready;
            sessionStorage.setItem(COI_SW_RELOAD_KEY, '1');
            location.reload();
        } catch (err) {
            setError('Service worker registration failed: ' + err.message);
        }
        return false;  // page is reloading
    }

    // ── Step 2: Fetch with progress tracking ──
    async function fetchWithProgress(url, label, progressStart, progressEnd) {
        setStatus(label);
        const resp = await fetch(url);
        if (!resp.ok) throw new Error('HTTP ' + resp.status + ' fetching ' + url);

        const totalSize = parseInt(resp.headers.get('content-length') || '0', 10);
        let loaded = 0;
        const chunks = [];

        const reader = resp.body.getReader();
        while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            chunks.push(value);
            loaded += value.byteLength;
            if (totalSize > 0) {
                const pct = progressStart + (progressEnd - progressStart) * (loaded / totalSize);
                setProgress(pct);
                const mb = (loaded / 1024 / 1024).toFixed(1);
                const tot = (totalSize / 1024 / 1024).toFixed(1);
                statusNote && (statusNote.textContent = mb + ' / ' + tot + ' MB');
            }
        }

        // Combine chunks into single ArrayBuffer
        const combined = new Uint8Array(loaded);
        let offset = 0;
        for (const chunk of chunks) { combined.set(chunk, offset); offset += chunk.byteLength; }
        return combined.buffer;
    }

    // ── Main boot sequence ──
    showStatus();
    setStatus('Initializing…');
    setProgress(0);

    // Clear the "already reloaded" flag if COI is now active
    if (crossOriginIsolated) {
        sessionStorage.removeItem(COI_SW_RELOAD_KEY);
    }

    const ready = await ensureCOI();
    if (!ready) return;  // either failed or reloading

    setProgress(10);
    setStatus('Cross-origin isolation active ✓');
    statusNote && (statusNote.textContent = 'Loading terminal library…');

    // ── Load xterm.js via dynamic import (correct scoped packages) ──
    let Terminal, FitAddon;
    try {
        const [xtermMod, fitMod] = await Promise.all([
            import('https://cdn.jsdelivr.net/npm/@xterm/xterm@5/+esm'),
            import('https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.10/+esm'),
        ]);
        Terminal = xtermMod.Terminal;
        FitAddon = fitMod.FitAddon;
        setProgress(15);
    } catch (err) {
        setError('Failed to load xterm.js: ' + err.message);
        return;
    }

    statusNote && (statusNote.textContent = 'Preparing to download Linux kernel…');
    await new Promise(r => setTimeout(r, 200));

    // ── Download vmlinux.wasm ──
    let vmlinux;
    try {
        setStatus('Downloading vmlinux.wasm (Linux kernel)…');
        statusNote && (statusNote.textContent = '~8 MB — this may take a moment');
        setProgress(12);
        const vmlinuxBuf = await fetchWithProgress(
            assetUrl('vmlinux.wasm'),
            'Downloading vmlinux.wasm (Linux kernel)…',
            12, 60
        );
        setStatus('Compiling Linux kernel…');
        setProgress(62);
        vmlinux = await WebAssembly.compile(vmlinuxBuf);
        setProgress(70);
    } catch (err) {
        setError('Failed to load vmlinux.wasm: ' + err.message);
        return;
    }

    // ── Download initramfs.cpio.gz ──
    let initrd;
    try {
        const initrdBuf = await fetchWithProgress(
            assetUrl('initramfs.cpio.gz'),
            'Downloading initramfs (BusyBox + musl)…',
            70, 90
        );
        initrd = initrdBuf;
    } catch (err) {
        setError('Failed to load initramfs.cpio.gz: ' + err.message);
        return;
    }

    setProgress(92);
    setStatus('Booting Linux kernel…');
    statusNote && (statusNote.textContent = 'Starting on 3 virtual CPUs');

    // ── Show terminal wrapper FIRST so xterm gets real layout dimensions ──
    // If we call term.open() while display:none, fitAddon gets 0×0 and the
    // terminal renders as a tiny square in the corner.
    hideStatus();
    showTerminal();
    // Wait one animation frame so the flex layout settles before fit()
    await new Promise(r => requestAnimationFrame(r));

    // ── Create xterm.js terminal ──
    let term, fitAddon;
    try {
        term = new Terminal({
            theme: {
                background:   '#0a0e1a',
                foreground:   '#e0e5ef',
                cursor:       '#00ff88',
                cursorAccent: '#0a0e1a',
                selectionBackground: '#264f78',
                black:   '#0a0e1a', brightBlack:   '#484f58',
                red:     '#e74c3c', brightRed:     '#ff6248',
                green:   '#00c07f', brightGreen:   '#00ff88',
                yellow:  '#f39c12', brightYellow:  '#ffd700',
                blue:    '#4e9af1', brightBlue:    '#79b8ff',
                magenta: '#a855f7', brightMagenta: '#d2a8ff',
                cyan:    '#00bcd4', brightCyan:    '#56d3e8',
                white:   '#c9d1d9', brightWhite:   '#ffffff',
            },
            fontFamily: "'Cascadia Code', 'Fira Code', 'JetBrains Mono', 'Courier New', monospace",
            fontSize: 14,
            lineHeight: 1.3,
            cursorStyle: 'bar',
            cursorBlink: true,
            scrollback: 5000,
            allowProposedApi: true,
        });

        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(xtermContainer);
        fitAddon.fit();
        // Focus immediately so keyboard input works right away
        term.focus();
    } catch (err) {
        setError('Terminal init failed: ' + err.message);
        console.error('xterm init error:', err);
        return;
    }

    // Re-focus on click (user may click elsewhere and lose focus)
    xtermContainer.addEventListener('click', () => term.focus());

    setProgress(95);

    term.write('\x1B[2m[traits.build] Booting Linux 6.4.16 via WebAssembly...\x1B[0m\r\n');
    term.write('\x1B[2m[traits.build] vmlinux compiled for wasm32 (NOMMU, -fPIC)\x1B[0m\r\n');
    term.write('\x1B[2m[traits.build] BusyBox 1.36.1 + musl 1.2.5 initramfs\x1B[0m\r\n\r\n');

    // ── Boot Linux ──
    const workerSrc = document.getElementById('linux-worker-src').textContent;
    const workerBlob = new Blob([workerSrc], { type: 'application/javascript' });
    const workerUrl = URL.createObjectURL(workerBlob);

    const normalizeTunnelUrl = (raw) => {
        const v = String(raw || '').trim();
        if (!v) return '';
        // Migrate legacy relay worker hosts to the canonical custom-domain endpoint.
        if (v.includes('traits-relay.kiliannc.workers.dev')) {
            return 'wss://relay.traits.build/linux/tunnel';
        }
        return v;
    };

    const resolveTunnelUrl = () => {
        try {
            const params = new URLSearchParams(location.search);
            const fromQuery = params.get('linux_tunnel');
            if (fromQuery) return normalizeTunnelUrl(fromQuery);
        } catch (e) {}

        try {
            const fromStorage = localStorage.getItem('linux-wasm.tunnel-url') || '';
            if (fromStorage) {
                const normalized = normalizeTunnelUrl(fromStorage);
                if (normalized && normalized !== fromStorage) {
                    try { localStorage.setItem('linux-wasm.tunnel-url', normalized); } catch (_) {}
                }
                return normalized;
            }
        } catch (e) {}

        // Default: use the global relay tunnel endpoint.
        return 'wss://relay.traits.build/linux/tunnel';
    };

    const tunnelUrl = resolveTunnelUrl();

    const NET_POLICY_KEY = 'linux-wasm.net-policy';
    const resolveNetPolicy = () => {
        try {
            const params = new URLSearchParams(location.search);
            const fromQuery = String(params.get('linux_net') || '').trim().toLowerCase();
            if (fromQuery === 'relay-only' || fromQuery === 'hybrid') return fromQuery;
        } catch (e) {}

        try {
            const fromStorage = String(localStorage.getItem(NET_POLICY_KEY) || '').trim().toLowerCase();
            if (fromStorage === 'relay-only' || fromStorage === 'hybrid') return fromStorage;
        } catch (e) {}

        return 'hybrid';
    };

    let netPolicy = resolveNetPolicy();
    if (typeof NetProxy !== 'undefined' && NetProxy.setRelayOnly) {
        try { NetProxy.setRelayOnly(netPolicy === 'relay-only'); } catch (_) {}
    }

    // Defensive boot guard: stale SPA state can keep an old relay socket alive
    // across route transitions. Force fallback before kernel boot so no tunnel
    // traffic runs during SMP bring-up.
    if (netPolicy === 'hybrid' && typeof NetProxy !== 'undefined' && NetProxy.forceBrowserFallback) {
        try { NetProxy.forceBrowserFallback(); } catch (_) {}
    }

    let bootNetMode = 'browser-fallback';
    let tunnelReady = null;

    // DEFER tunnel initialization until after SMP bring-up completes.
    // During kernel secondary CPU initialization, relay polling interferes with interrupt
    // handlers, causing context tracking violations and kernel panics.
    // We'll enable tunnel polling after the shell prompt appears (shellReady = true).
    function deferTunnelSetup() {
        if (!tunnelUrl) return;
        console.log('[linux.rs] deferred tunnel: setting up relay tunnel...');
        if (typeof NetProxy !== 'undefined' && NetProxy.setTunnelURL && tunnelUrl) {
            NetProxy.setTunnelURL(tunnelUrl);
        }
        if (typeof NetProxy !== 'undefined' && NetProxy.waitForTunnelReady) {
            NetProxy.waitForTunnelReady(5000).then(() => {
                const mode = (typeof NetProxy.getMode) ? NetProxy.getMode() : 'unknown';
                const suffix = tunnelUrl ? ` (${tunnelUrl})` : '';
                console.log('[linux.rs] deferred tunnel ready: mode=' + mode);
                if (mode === 'browser-fallback') {
                    const errMsg = (typeof NetProxy.getTunnelError) ? NetProxy.getTunnelError() : null;
                    console.log('[linux.rs] tunnel error (fallback):', errMsg);
                } else if (mode === 'tunnel') {
                    console.info('[linux.rs] relay tunnel connected');
                }
                tunnelReady = mode;
            }).catch(e => {
                console.warn('[linux.rs] deferred tunnel setup failed:', e);
                tunnelReady = 'browser-fallback';
            });
        }
    }
    
    // Boot with browser emulation first; enable relay tunnel after SMP is ready.
    const bootModeText = netPolicy === 'relay-only'
        ? 'relay-only (tunnel hard-deferred until after shell-ready; no browser fallback)'
        : 'browser-fallback (relay hard-deferred until after shell-ready)';
    term.write(`\x1B[2m[traits.build] NET mode: ${bootModeText}\x1B[0m\r\n`);

    const resolveInitProgram = () => {
        try {
            const params = new URLSearchParams(location.search);
            const fromQuery = params.get('linux_init');
            if (fromQuery) return fromQuery;
        } catch (e) {}

        try {
            const fromStorage = localStorage.getItem('linux-wasm.init') || '';
            if (fromStorage) return fromStorage;
        } catch (e) {}

        // Default to full initramfs script which sets up /proc, /sys, networking,
        // and drops into BusyBox init. Requires maxcpus>=3 (2 user CPUs) for fork to work.
        return '/init';
    };

    const initProgram = resolveInitProgram();

    const resolveMaxCpus = () => {
        const HARD_MAX = 100;
        const DEFAULT = 10;

        const clamp = (n) => {
            if (!Number.isFinite(n)) return DEFAULT;
            return Math.max(3, Math.min(HARD_MAX, Math.floor(n)));
        };

        try {
            const params = new URLSearchParams(location.search);
            const fromQuery = params.get('linux_cpus');
            if (fromQuery) return clamp(Number(fromQuery));
        } catch (e) {}

        try {
            const fromStorage = localStorage.getItem('linux-wasm.maxcpus') || '';
            if (fromStorage) return clamp(Number(fromStorage));
        } catch (e) {}

        return DEFAULT;
    };

    const maxCpus = resolveMaxCpus();

    // WASM kernel model: each user task (non-kthread) needs its own dedicated CPU.
    // The kernel's user_task_set_affinity() in arch/wasm/kernel/process.c pins each
    // forked user process to a unique CPU. CPU 1 is reserved as IRQ_CPU.
    // With maxcpus=N, we get (N-1) usable user CPUs (minus IRQ_CPU).
    // Too few CPUs → clone() returns -EBUSY ("Resource busy") when shell forks.
    // CPUs are recycled when tasks exit (release_thread clears user_cpus bitmask).
    // maxcpus=10 gives 8 user CPUs (0,2..9): highest stable setting in this runtime.
    // Override with ?linux_cpus=N or localStorage['linux-wasm.maxcpus'].
    const boot_cmdline = `maxcpus=${maxCpus} root=/dev/ram0 rootfstype=ramfs rdinit=${initProgram} console=hvc console=ttyS0`;
    term.write(`\x1B[2m[traits.build] CPU mode: maxcpus=${maxCpus}\x1B[0m\r\n`);
    if (initProgram !== '/init') {
        term.write(`\x1B[2m[traits.build] INIT mode: minimal (${initProgram})\x1B[0m\r\n`);
    }

    const logLine = (text) => {
        // Keep noisy kernel/runtime diagnostics in DevTools only.
        console.log('[linux/log]', text);
    };
    // Track whether the first shell prompt has appeared (for secret injection)
    let shellReady = false;
    let shellReadyResolve = null;
    const shellReadyPromise = new Promise(r => { shellReadyResolve = r; });
    let consoleBuffer = '';

    // ── JS Agent state ──
    // The agent loop runs entirely in JavaScript: fetch() for LLM API calls (no fork),
    // shell only used to execute commands returned by the LLM.
    // This avoids CLONE_VM heap corruption that crashes restore_redirects→free().
    let agentCapture = null;      // function(data) callback during command capture
    let agentRunning = false;     // true while agent loop is active
    let agentSuppressOutput = false; // suppress shell echo during agent command exec
    let autoFixOnErrorEnabled = false;
    let autoFixInFlight = false;
    let shellRelayRunning = false;
    let shellRelayStop = false;
    let shellRelayCode = '';
    const SHELL_RELAY_URL = 'https://relay.traits.build';
    const SHELL_RELAY_CODE_KEY = 'linux-wasm.shell-relay.code';
    const AUTOFIX_SENTINEL_RE = /__AFXEC:(-?\d+)__/;

    function parseAgentTask(cmd) {
        const trimmed = (cmd || '').trim();
        let task = null;
        if (trimmed === 'agent') task = '';
        else if (trimmed.startsWith('agent ')) task = trimmed.slice(6).trim();
        else if (trimmed.startsWith('qjs /bin/agent.js ')) task = trimmed.slice('qjs /bin/agent.js '.length).trim();
        else if (trimmed.startsWith('/bin/agent.sh ')) task = trimmed.slice('/bin/agent.sh '.length).trim();
        else if (trimmed.startsWith('agent.sh ')) task = trimmed.slice('agent.sh '.length).trim();
        else return null;

        if ((task.startsWith('"') && task.endsWith('"')) || (task.startsWith("'") && task.endsWith("'"))) {
            task = task.slice(1, -1).trim();
        }
        return task;
    }

    function parseTraitCall(cmd) {
        const trimmed = (cmd || '').trim();
        let body = null;
        if (trimmed === 'traits-call' || trimmed === 'trait') body = '';
        else if (trimmed.startsWith('traits-call ')) body = trimmed.slice('traits-call '.length).trim();
        else if (trimmed.startsWith('trait ')) body = trimmed.slice('trait '.length).trim();
        else if (trimmed.startsWith('qjs /bin/traits-call.js ')) body = trimmed.slice('qjs /bin/traits-call.js '.length).trim();
        else if (trimmed.startsWith('/bin/traits-call.js ')) body = trimmed.slice('/bin/traits-call.js '.length).trim();
        else return null;

        if ((body.startsWith('"') && body.endsWith('"')) || (body.startsWith("'") && body.endsWith("'"))) {
            body = body.slice(1, -1).trim();
        }
        return body;
    }

    async function runTraitCallCommand(cmd) {
        const body = parseTraitCall(cmd);
        if (body === null) return false;

        if (!body) {
            term.write('Usage: qjs /bin/traits-call.js <trait.path> [args-json]\r\n');
            term.write('       /bin/traits-call.js <trait.path> [args-json]\r\n');
            term.write('       traits-call <trait.path> [args-json]\r\n');
            term.write('Examples:\r\n');
            term.write('  qjs /bin/traits-call.js sys.version\r\n');
            term.write('  qjs /bin/traits-call.js sys.registry ["count"]\r\n');
            term.write('  traits-call kernel.call ["sys.version"]\r\n');
            term.write('Note: uses <trait>@wasm first, then falls back to <trait>.\r\n');
            return true;
        }

        const firstSpace = body.indexOf(' ');
        const rawPath = (firstSpace === -1 ? body : body.slice(0, firstSpace)).trim();
        const rawArgs = (firstSpace === -1 ? '' : body.slice(firstSpace + 1).trim());
        if (!rawPath) {
            term.write('\x1b[31m[traits-call] missing trait path\x1b[0m\r\n');
            return true;
        }

        let args = [];
        if (rawArgs) {
            try {
                const parsed = JSON.parse(rawArgs);
                args = Array.isArray(parsed) ? parsed : [parsed];
            } catch (e) {
                term.write('\x1b[31m[traits-call] args must be valid JSON (prefer a JSON array)\x1b[0m\r\n');
                return true;
            }
        }

        const sdk = window._traitsSDK;
        if (!sdk || typeof sdk.call !== 'function') {
            term.write('\x1b[31m[traits-call] SDK not ready\x1b[0m\r\n');
            return true;
        }

        const wasmPath = rawPath.includes('@wasm') ? rawPath : (rawPath + '@wasm');
        try {
            let result;
            let usedPath = wasmPath;
            try {
                result = await sdk.call(wasmPath, args);
            } catch (e) {
                usedPath = rawPath;
                result = await sdk.call(rawPath, args);
            }
            term.write('\x1b[2m[traits-call] ' + usedPath + '\x1b[0m\r\n');
            const printed = (typeof result === 'string') ? result : JSON.stringify(result, null, 2);
            term.write((printed || 'null') + '\r\n');
        } catch (e) {
            term.write('\x1b[31m[traits-call] ' + (e && e.message ? e.message : String(e)) + '\x1b[0m\r\n');
        }
        return true;
    }

    const PERSIST_KEY = 'linux-wasm.persist.files';
    const PERSIST_PVFS_KEY = 'traits.pvfs';
    const PERSIST_PVFS_PREFIX = 'linux-wasm.persist';
    const PERSIST_DIRS_META = '__linux_wasm_persist_dirs__';
    const PERSIST_DIRS_META_LEGACY = PERSIST_PVFS_PREFIX + '/.dirs.json';
    const PERSIST_MOUNT_KEY = 'linux-wasm.persist.mounts';
    const PERSIST_AUTOSYNC_KEY = 'linux-wasm.persist.autosync';

    function persistInitDefaults() {
        try {
            if (!localStorage.getItem(PERSIST_MOUNT_KEY)) {
                localStorage.setItem(PERSIST_MOUNT_KEY, '/tmp,/home');
            }
            if (!localStorage.getItem(PERSIST_AUTOSYNC_KEY)) {
                localStorage.setItem(PERSIST_AUTOSYNC_KEY, '1');
            }
        } catch (e) {}
    }

    function persistNormalizePath(path) {
        const p = String(path || '').trim();
        if (!p) return '';
        return p.startsWith('/') ? p : ('/' + p);
    }

    function persistNormalizeMounts(parts) {
        const out = [];
        for (const raw of (parts || [])) {
            const p = persistNormalizePath(raw);
            if (!p) continue;
            if (!out.includes(p)) out.push(p);
        }
        return out;
    }

    function persistDefaultMount() {
        const m = persistMountPaths();
        return m[0] || '/tmp';
    }

    function persistResolvePath(path) {
        const p = String(path || '').trim();
        if (!p) return '';
        return p.startsWith('/') ? persistNormalizePath(p) : (persistDefaultMount() + '/' + p);
    }

    function persistIsValidPath(path) {
        const p = persistNormalizePath(path);
        if (!p) return false;
        if (/['"\\;`\r\n]/.test(p)) return false;
        return /^\/[A-Za-z0-9._\-/+:@=]*$/.test(p);
    }

    function persistPathMounted(path) {
        const p = persistNormalizePath(path);
        if (!p) return false;
        const mounts = persistMountPaths();
        return mounts.some(m => p === m || p.startsWith(m + '/'));
    }

    function persistPvfsLoadAll() {
        try {
            const raw = localStorage.getItem(PERSIST_PVFS_KEY) || '{}';
            const parsed = JSON.parse(raw);
            if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) return parsed;
        } catch (e) {}
        return {};
    }

    function persistPvfsSaveAll(map) {
        try { localStorage.setItem(PERSIST_PVFS_KEY, JSON.stringify(map || {})); } catch (e) {}
    }

    function persistToPvfsPath(path) {
        const p = persistNormalizePath(path);
        if (!p) return '';
        return p;
    }

    function persistFromPvfsPath(path) {
        const text = String(path || '');
        // New format: direct absolute guest path in PVFS key
        if (text.startsWith('/')) return persistNormalizePath(text);

        // Legacy format: linux-wasm.persist/<absolute/path>
        const prefix = PERSIST_PVFS_PREFIX + '/';
        if (!text.startsWith(prefix)) return '';
        return persistNormalizePath(text.slice(PERSIST_PVFS_PREFIX.length));
    }

    function persistPvfsKeyIsGuestPath(key) {
        const k = persistNormalizePath(key);
        return !!(k && persistIsValidPath(k));
    }

    function persistPvfsKeyIsMountedGuestPath(key) {
        if (!persistPvfsKeyIsGuestPath(key)) return false;
        return persistPathMounted(key);
    }

    function persistLoadLegacy() {
        try {
            const raw = localStorage.getItem(PERSIST_KEY) || '{}';
            const parsed = JSON.parse(raw);
            if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return { files: {}, dirs: [] };

            if (parsed.files && typeof parsed.files === 'object' && !Array.isArray(parsed.files)) {
                const files = {};
                for (const k of Object.keys(parsed.files)) {
                    const pk = persistNormalizePath(k);
                    if (!persistIsValidPath(pk) || !persistPathMounted(pk)) continue;
                    files[pk] = String(parsed.files[k] || '');
                }
                const dirsRaw = Array.isArray(parsed.dirs) ? parsed.dirs : [];
                const dirs = persistNormalizeMounts(dirsRaw).filter(d => persistIsValidPath(d) && persistPathMounted(d));
                return { files, dirs };
            }

            const files = {};
            const base = persistDefaultMount();
            for (const name of Object.keys(parsed)) {
                const rel = String(name || '').replace(/^\/+/, '');
                if (!rel) continue;
                const migrated = base + '/' + rel;
                if (!persistIsValidPath(migrated) || !persistPathMounted(migrated)) continue;
                files[migrated] = String(parsed[name] || '');
            }
            return { files, dirs: [] };
        } catch (e) {}
        return { files: {}, dirs: [] };
    }

    function persistLoad() {
        try {
            const pvfs = persistPvfsLoadAll();
            const files = {};
            for (const k of Object.keys(pvfs)) {
                if (k === PERSIST_DIRS_META || k === PERSIST_DIRS_META_LEGACY) continue;
                const guestPath = persistFromPvfsPath(k);
                if (!persistIsValidPath(guestPath) || !persistPathMounted(guestPath)) continue;
                files[guestPath] = String(pvfs[k] || '');
            }
            let dirs = [];
            try {
                const parsedDirs = JSON.parse(String(pvfs[PERSIST_DIRS_META] || pvfs[PERSIST_DIRS_META_LEGACY] || '[]'));
                if (Array.isArray(parsedDirs)) {
                    dirs = persistNormalizeMounts(parsedDirs).filter(d => persistIsValidPath(d) && persistPathMounted(d));
                }
            } catch (e) {}
            if (Object.keys(files).length || dirs.length) {
                return { files, dirs };
            }

            const legacy = persistLoadLegacy();
            if (Object.keys(legacy.files).length || legacy.dirs.length) {
                persistSave(legacy);
                try { localStorage.removeItem(PERSIST_KEY); } catch (e) {}
                return legacy;
            }
        } catch (e) {}
        return { files: {}, dirs: [] };
    }

    function persistSave(state) {
        const safe = {
            files: (state && state.files && typeof state.files === 'object') ? state.files : {},
            dirs: Array.isArray(state && state.dirs) ? state.dirs : [],
        };
        const pvfs = persistPvfsLoadAll();
        for (const k of Object.keys(pvfs)) {
            // Remove linux-wasm persist payload only for mounted guest paths.
            // Keep unrelated pvfs entries owned by other features.
            if (k === PERSIST_DIRS_META || k === PERSIST_DIRS_META_LEGACY) {
                delete pvfs[k];
                continue;
            }
            if (k.startsWith(PERSIST_PVFS_PREFIX + '/')) {
                delete pvfs[k];
                continue;
            }
            if (persistPvfsKeyIsMountedGuestPath(k)) {
                delete pvfs[k];
            }
        }
        for (const path of Object.keys(safe.files)) {
            if (!persistIsValidPath(path) || !persistPathMounted(path)) continue;
            pvfs[persistToPvfsPath(path)] = String(safe.files[path] || '');
        }
        const dirs = safe.dirs.filter(d => persistIsValidPath(d) && persistPathMounted(d));
        if (dirs.length) pvfs[PERSIST_DIRS_META] = JSON.stringify(dirs);
        persistPvfsSaveAll(pvfs);
    }

    function persistMountPaths() {
        try {
            const raw = (localStorage.getItem(PERSIST_MOUNT_KEY) || '/tmp,/home').trim();
            const parts = raw.split(',').map(s => s.trim()).filter(Boolean);
            const mounts = persistNormalizeMounts(parts);
            return mounts.length ? mounts : ['/tmp', '/home'];
        } catch (e) {
            return ['/tmp', '/home'];
        }
    }

    function persistSetMountPaths(paths) {
        const mounts = persistNormalizeMounts(paths);
        const finalMounts = mounts.length ? mounts : ['/tmp', '/home'];
        try { localStorage.setItem(PERSIST_MOUNT_KEY, finalMounts.join(',')); } catch (e) {}
    }

    function persistAutosyncEnabled() {
        try { return (localStorage.getItem(PERSIST_AUTOSYNC_KEY) || '') === '1'; } catch (e) { return false; }
    }

    function persistSetAutosync(enabled) {
        try { localStorage.setItem(PERSIST_AUTOSYNC_KEY, enabled ? '1' : '0'); } catch (e) {}
    }

    persistInitDefaults();

    function shellSingleQuote(s) {
        return String(s).replace(/'/g, "'\\''");
    }

    function persistTrackMkdir(cmd) {
        const text = String(cmd || '').trim();
        if (!text.startsWith('mkdir')) return;
        const tokens = text.split(/\s+/).slice(1);
        const paths = [];
        for (const t of tokens) {
            if (!t) continue;
            if (t.startsWith('-')) continue;
            const p = persistResolvePath(t);
            if (!p) continue;
            paths.push(p);
        }
        if (!paths.length) return;
        const state = persistLoad();
        for (const d of paths) {
            if (!state.dirs.includes(d)) state.dirs.push(d);
        }
        persistSave(state);
    }

    async function persistSyncToGuest(silent) {
        const state = persistLoad();
        const mounts = persistMountPaths();
        const dirs = Array.isArray(state.dirs) ? state.dirs.slice().sort() : [];
        const names = Object.keys(state.files || {}).sort();
        if (!silent) {
            term.write('\x1b[2m[persist] sync -> ' + mounts.join(', ') + ' (' + names.length + ' file(s), ' + dirs.length + ' dir(s))\x1b[0m\r\n');
        }

        // Ensure persisted directories exist first.
        for (const dir of dirs) {
            const cmd = "mkdir -p '" + shellSingleQuote(dir) + "'";
            const { exitCode, crashed } = await shellExec(cmd);
            if (crashed) {
                term.write('\x1b[31m[persist] kernel crashed while creating ' + dir + '\x1b[0m\r\n');
                return;
            }
            if (exitCode !== 0 && !silent) {
                term.write('\x1b[33m[persist] mkdir failed: ' + dir + ' (exit ' + exitCode + ')\x1b[0m\r\n');
            }
        }

        for (const target of names) {
            const content = String(state.files[target] || '');
            const cmd = "printf '%s' '" + shellSingleQuote(content) + "' > '" + shellSingleQuote(target) + "'";
            const { exitCode, crashed } = await shellExec(cmd);
            if (crashed) {
                term.write('\x1b[31m[persist] kernel crashed while syncing ' + target + '\x1b[0m\r\n');
                return;
            }
            if (exitCode !== 0) {
                term.write('\x1b[33m[persist] failed: ' + target + ' (exit ' + exitCode + ')\x1b[0m\r\n');
            }
        }
        if (!silent) term.write('\x1b[32m[persist] sync done\x1b[0m\r\n');
    }

    // ── Persist pull: snapshot guest FS back to localStorage ──
    let persistPullRunning = false;

    async function persistPullFromGuest(silent) {
        if (persistPullRunning) return;
        persistPullRunning = true;
        try {
            const mounts = persistMountPaths();
            const mountArgs = mounts.map(m => "'" + shellSingleQuote(m) + "'").join(' ');

            // Discover dirs
            const dirsResult = await shellExec('find ' + mountArgs + ' -type d');
            if (dirsResult.crashed) return;
            const mountSet = new Set(mounts);
            const dirs = dirsResult.exitCode === 0
                ? dirsResult.output.split('\n').map(d => persistNormalizePath(d.trim())).filter(d => d && !mountSet.has(d))
                : [];

            // Discover files + read contents in one compound command
            const filesResult = await shellExec('find ' + mountArgs + ' -type f');
            if (filesResult.crashed) return;
            const filePaths = filesResult.exitCode === 0
                ? filesResult.output.split('\n').map(f => persistNormalizePath(f.trim())).filter(f => persistIsValidPath(f) && persistPathMounted(f))
                : [];

            const newFiles = {};
            if (filePaths.length) {
                for (const f of filePaths) {
                    const r = await shellExec("cat '" + shellSingleQuote(f) + "'");
                    if (r.crashed) break;
                    if (r.exitCode === 0) newFiles[f] = r.output;
                }
            }

            const state = { files: newFiles, dirs };
            persistSave(state);
            if (!silent) {
                term.write('[persist] pull: ' + Object.keys(newFiles).length + ' file(s), ' + dirs.length + ' dir(s)\r\n');
            }
        } finally {
            persistPullRunning = false;
        }
    }

    // Track file writes by intercepting redirect patterns in user commands.
    // Parses echo/printf > /path commands and saves content to persist state.
    function persistTrackRedirect(cmd) {
        const text = String(cmd || '').trim();
        // Match: ... > /path or ... >> /path (avoid heredoc << and stderr 2>)
        const redir = text.match(/(?:^|[^<>2])\s*(>{1,2})\s*(\/\S+)\s*$/);
        if (!redir) return;
        const isAppend = redir[1] === '>>';
        const target = persistResolvePath(redir[2]);
        if (!target) return;
        // Only track files under mounted paths
        const mounts = persistMountPaths();
        if (!mounts.some(m => target.startsWith(m + '/') || target === m)) return;
        // Extract content from simple echo/printf patterns
        const beforeRedir = text.slice(0, text.lastIndexOf(redir[1])).trim();
        let content = null;
        // echo 'content' or echo "content"
        const echoQ = beforeRedir.match(/^echo\s+['"]([\s\S]*)['"]\s*$/);
        if (echoQ) content = echoQ[1];
        // echo content (unquoted)
        if (content === null) {
            const echoU = beforeRedir.match(/^echo\s+([\s\S]+)$/);
            if (echoU) content = echoU[1].trim();
        }
        // printf '%s' 'content'
        if (content === null) {
            const pf = beforeRedir.match(/^printf\s+'%s'\s+'([\s\S]*)'/)
                     || beforeRedir.match(/^printf\s+'%s'\s+"([\s\S]*)"/);
            if (pf) content = pf[1];
        }
        if (content === null) return;
        const state = persistLoad();
        if (isAppend) {
            state.files[target] = (state.files[target] || '') + content;
        } else {
            state.files[target] = content;
        }
        persistSave(state);
    }

    async function runPersistCommand(cmd) {
        const trimmed = (cmd || '').trim();
        const body = trimmed === 'persist' ? 'help' : trimmed.slice('persist'.length).trim();
        const parts = body ? body.split(/\s+/) : ['help'];
        const sub = (parts[0] || 'help').toLowerCase();
        const state = persistLoad();

        if (sub === 'help') {
            term.write('persist commands:\r\n');
            term.write('  persist ls\r\n');
            term.write('  persist get <path>\r\n');
            term.write('  persist put <path> <content...>\r\n');
            term.write('  persist rm <path>\r\n');
            term.write('  persist pull              scan guest FS -> localStorage\r\n');
            term.write('  persist mount [path1 path2 ...]\r\n');
            term.write('  persist sync              localStorage -> guest FS\r\n');
            term.write('  persist autosync on|off|status\r\n');
            term.write('  persist secret <path> [SECRET_KEY]\r\n');
            return;
        }

        if (sub === 'ls') {
            const names = Object.keys(state.files || {}).sort();
            const dirs = Array.isArray(state.dirs) ? state.dirs.slice().sort() : [];
            term.write('[persist] mounts=' + persistMountPaths().join(', ') + '\r\n');
            if (!names.length) term.write('[persist] (empty)\r\n');
            if (dirs.length) {
                term.write('[persist] dirs:\r\n');
                for (const d of dirs) term.write('  ' + d + '\r\n');
            }
            if (names.length) {
                term.write('[persist] files:\r\n');
                for (const n of names) term.write('  ' + n + ' (' + String(state.files[n] || '').length + ' bytes)\r\n');
            }
            return;
        }

        if (sub === 'get') {
            const p = persistResolvePath(parts[1] || '');
            if (!p) { term.write('usage: persist get <path>\r\n'); return; }
            if (!(p in (state.files || {}))) { term.write('[persist] missing: ' + p + '\r\n'); return; }
            term.write(String(state.files[p] || '') + '\r\n');
            return;
        }

        if (sub === 'put') {
            const p = persistResolvePath(parts[1] || '');
            if (!p) { term.write('usage: persist put <path> <content...>\r\n'); return; }
            const content = body.split(/\s+/).slice(2).join(' ');
            state.files[p] = content;
            persistSave(state);
            term.write('[persist] saved: ' + p + ' (' + content.length + ' bytes)\r\n');
            return;
        }

        if (sub === 'rm') {
            const p = persistResolvePath(parts[1] || '');
            if (!p) { term.write('usage: persist rm <path>\r\n'); return; }
            const existed = Object.prototype.hasOwnProperty.call(state.files || {}, p);
            if (existed) delete state.files[p];
            state.dirs = (state.dirs || []).filter(d => d !== p);
            persistSave(state);
            term.write(existed ? ('[persist] removed: ' + p + '\r\n') : ('[persist] missing: ' + p + '\r\n'));
            return;
        }

        if (sub === 'mount') {
            const paths = parts.slice(1);
            if (!paths.length) {
                term.write('[persist] mounts=' + persistMountPaths().join(', ') + '\r\n');
                return;
            }
            persistSetMountPaths(paths);
            term.write('[persist] mounts set to ' + persistMountPaths().join(', ') + '\r\n');
            return;
        }

        if (sub === 'pull') {
            await persistPullFromGuest(false);
            return;
        }

        if (sub === 'sync') {
            await persistSyncToGuest(false);
            return;
        }

        if (sub === 'autosync') {
            const val = (parts[1] || 'status').toLowerCase();
            if (val === 'status') {
                term.write('[persist] autosync=' + (persistAutosyncEnabled() ? 'on' : 'off') + '\r\n');
                return;
            }
            if (val !== 'on' && val !== 'off') {
                term.write('usage: persist autosync on|off|status\r\n');
                return;
            }
            persistSetAutosync(val === 'on');
            term.write('[persist] autosync=' + val + '\r\n');
            return;
        }

        if (sub === 'secret') {
            const p = persistResolvePath(parts[1] || '');
            const key = parts[2] || 'OPENAI_API_KEY';
            if (!p) {
                term.write('usage: persist secret <path> [SECRET_KEY]\r\n');
                return;
            }
            const v = localStorage.getItem('traits.secret.' + key) || localStorage.getItem('traits.secret.' + key.toUpperCase()) || '';
            if (!v) {
                term.write('[persist] secret not found: ' + key + '\r\n');
                return;
            }
            state.files[p] = v;
            persistSave(state);
            term.write('[persist] saved secret into ' + p + ' (' + v.length + ' bytes)\r\n');
            return;
        }

        term.write('[persist] unknown command: ' + sub + '\r\n');
    }

    function shQuote(s) {
        return "'" + String(s).replace(/'/g, "'\\''") + "'";
    }

    function sleepMs(ms) {
        return new Promise((resolve) => setTimeout(resolve, ms));
    }

    function shellRelayLoadCode() {
        try {
            return (localStorage.getItem(SHELL_RELAY_CODE_KEY) || '').trim().toUpperCase();
        } catch (e) {
            return '';
        }
    }

    function shellRelaySaveCode(code) {
        try {
            if (code) localStorage.setItem(SHELL_RELAY_CODE_KEY, code);
            else localStorage.removeItem(SHELL_RELAY_CODE_KEY);
        } catch (e) {}
    }

    async function shellRelayRegister(preferredCode) {
        const body = preferredCode ? JSON.stringify({ code: preferredCode }) : '{}';
        const resp = await fetch(SHELL_RELAY_URL + '/relay/register', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body,
            signal: AbortSignal.timeout(8000),
        });
        if (!resp.ok) throw new Error('register failed: HTTP ' + resp.status);
        const json = await resp.json();
        const code = String((json && json.code) || '').toUpperCase();
        if (!code) throw new Error('register failed: missing code');
        return code;
    }

    async function shellRelayRespond(code, id, result, error) {
        try {
            await fetch(SHELL_RELAY_URL + '/relay/respond', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ code, id, result, error: error || null }),
                signal: AbortSignal.timeout(8000),
            });
        } catch (e) {
            console.warn('[shell-relay] respond failed:', e && e.message ? e.message : String(e));
        }
    }

    async function shellRelayHandleRequest(code, req) {
        const id = String(req && req.id ? req.id : '');
        const path = String(req && req.path ? req.path : '');
        const args = Array.isArray(req && req.args) ? req.args : [];
        if (!id) return;

        if (agentRunning) {
            await shellRelayRespond(code, id, null, 'agent is running; try again shortly');
            return;
        }

        if (path === 'linux.exec') {
            const cmd = String(args[0] || '').trim();
            if (!cmd) {
                await shellRelayRespond(code, id, null, 'missing command arg');
                return;
            }

            term.write('\x1b[2m[shell-relay] $ ' + cmd + '\x1b[0m\r\n');
            const r = await shellExec(cmd);
            await shellRelayRespond(code, id, {
                output: r && typeof r.output === 'string' ? r.output : '',
                exitCode: r && typeof r.exitCode === 'number' ? r.exitCode : -1,
                crashed: !!(r && r.crashed),
            }, null);
            return;
        }

        if (path === 'linux.vfs.list') {
            const requested = persistResolvePath(String(args[0] || '/'));
            if (!requested || !persistIsValidPath(requested) || !persistPathMounted(requested)) {
                await shellRelayRespond(code, id, null, 'invalid or unmounted path');
                return;
            }

            const state = persistLoad();
            const dirs = Array.isArray(state.dirs) ? state.dirs.slice() : [];
            const files = (state.files && typeof state.files === 'object') ? Object.keys(state.files) : [];
            const prefix = requested === '/' ? '/' : (requested + '/');

            const outDirs = [];
            const outFiles = [];

            for (const d of dirs) {
                if (d === requested || d.startsWith(prefix)) outDirs.push(d);
            }
            for (const f of files) {
                if (f === requested || f.startsWith(prefix)) {
                    const content = String(state.files[f] || '');
                    outFiles.push({ path: f, bytes: content.length });
                }
            }

            outDirs.sort();
            outFiles.sort((a, b) => a.path.localeCompare(b.path));

            await shellRelayRespond(code, id, {
                path: requested,
                mounts: persistMountPaths(),
                dirs: outDirs,
                files: outFiles,
            }, null);
            return;
        }

        if (path === 'linux.vfs.read') {
            const requested = persistResolvePath(String(args[0] || ''));
            if (!requested || !persistIsValidPath(requested) || !persistPathMounted(requested)) {
                await shellRelayRespond(code, id, null, 'invalid or unmounted path');
                return;
            }

            const state = persistLoad();
            if (Object.prototype.hasOwnProperty.call(state.files || {}, requested)) {
                const content = String(state.files[requested] || '');
                await shellRelayRespond(code, id, {
                    path: requested,
                    content,
                    bytes: content.length,
                    source: 'persist',
                }, null);
                return;
            }

            const r = await shellExec('cat ' + shQuote(requested));
            if (r.crashed || r.exitCode !== 0) {
                await shellRelayRespond(code, id, null, 'read failed: exit ' + r.exitCode + (r.crashed ? ' (kernel crashed)' : ''));
                return;
            }
            const content = String(r.output || '');
            state.files[requested] = content;
            persistSave(state);
            await shellRelayRespond(code, id, {
                path: requested,
                content,
                bytes: content.length,
                source: 'guest',
            }, null);
            return;
        }

        if (path === 'linux.vfs.write') {
            const requested = persistResolvePath(String(args[0] || ''));
            if (!requested || !persistIsValidPath(requested) || !persistPathMounted(requested)) {
                await shellRelayRespond(code, id, null, 'invalid or unmounted path');
                return;
            }

            const content = String(args.length > 1 ? args[1] : '');
            const mode = String(args[2] || 'truncate').toLowerCase();
            if (mode !== 'truncate' && mode !== 'append') {
                await shellRelayRespond(code, id, null, 'invalid mode (use truncate or append)');
                return;
            }

            const state = persistLoad();
            if (mode === 'append') state.files[requested] = String(state.files[requested] || '') + content;
            else state.files[requested] = content;
            persistSave(state);

            const op = mode === 'append' ? '>>' : '>';
            const cmd = "printf '%s' " + shQuote(content) + ' ' + op + ' ' + shQuote(requested);
            const r = await shellExec(cmd);

            await shellRelayRespond(code, id, {
                path: requested,
                bytes: content.length,
                mode,
                guest: {
                    exitCode: r && typeof r.exitCode === 'number' ? r.exitCode : -1,
                    crashed: !!(r && r.crashed),
                },
            }, null);
            return;
        }

        if (path === 'linux.vfs.mkdir') {
            const requested = persistResolvePath(String(args[0] || ''));
            if (!requested || !persistIsValidPath(requested) || !persistPathMounted(requested)) {
                await shellRelayRespond(code, id, null, 'invalid or unmounted path');
                return;
            }

            const state = persistLoad();
            if (!state.dirs.includes(requested)) state.dirs.push(requested);
            persistSave(state);

            const r = await shellExec('mkdir -p ' + shQuote(requested));
            await shellRelayRespond(code, id, {
                path: requested,
                guest: {
                    exitCode: r && typeof r.exitCode === 'number' ? r.exitCode : -1,
                    crashed: !!(r && r.crashed),
                },
            }, null);
            return;
        }

        if (path === 'linux.vfs.delete') {
            const requested = persistResolvePath(String(args[0] || ''));
            if (!requested || !persistIsValidPath(requested) || !persistPathMounted(requested)) {
                await shellRelayRespond(code, id, null, 'invalid or unmounted path');
                return;
            }

            const state = persistLoad();
            const prefix = requested + '/';
            let removedFiles = 0;
            let removedDirs = 0;

            for (const key of Object.keys(state.files || {})) {
                if (key === requested || key.startsWith(prefix)) {
                    delete state.files[key];
                    removedFiles += 1;
                }
            }
            state.dirs = (state.dirs || []).filter((d) => {
                const remove = d === requested || d.startsWith(prefix);
                if (remove) removedDirs += 1;
                return !remove;
            });
            persistSave(state);

            const r = await shellExec('rm -rf ' + shQuote(requested));
            await shellRelayRespond(code, id, {
                path: requested,
                removedFiles,
                removedDirs,
                guest: {
                    exitCode: r && typeof r.exitCode === 'number' ? r.exitCode : -1,
                    crashed: !!(r && r.crashed),
                },
            }, null);
            return;
        }

        await shellRelayRespond(code, id, null,
            'unsupported path: ' + path + ' (use linux.exec or linux.vfs.list/read/write/mkdir/delete)');
    }

    async function shellRelayLoop(code) {
        shellRelayRunning = true;
        shellRelayStop = false;
        shellRelayCode = code;
        while (!shellRelayStop) {
            try {
                const resp = await fetch(SHELL_RELAY_URL + '/relay/poll?code=' + encodeURIComponent(code), {
                    signal: AbortSignal.timeout(35000),
                });

                if (resp.status === 204) continue;
                if (resp.status === 404 || resp.status === 410) {
                    term.write('\x1b[33m[shell-relay] session closed; reconnect with relay on\x1b[0m\r\n');
                    break;
                }
                if (!resp.ok) {
                    await sleepMs(2000);
                    continue;
                }

                const req = await resp.json();
                await shellRelayHandleRequest(code, req);
            } catch (e) {
                await sleepMs(1500);
            }
        }
        shellRelayRunning = false;
    }

    async function runShellRelayCommand(cmd) {
        const trimmed = String(cmd || '').trim();
        if (!/^relay\b/i.test(trimmed)) return false;

        const parts = trimmed.split(/\s+/);
        const sub = (parts[1] || 'help').toLowerCase();

        if (sub === 'help') {
            term.write('relay commands:\r\n');
            term.write('  relay on [CODE]     connect this shell to relay\r\n');
            term.write('  relay off           disconnect relay\r\n');
            term.write('  relay status        show current state/code\r\n');
            term.write('remote call example (host):\r\n');
            term.write('  curl -s -X POST https://relay.traits.build/relay/call -H "Content-Type: application/json" -d "{\\"code\\":\\"CODE\\",\\"path\\":\\"linux.exec\\",\\"args\\":[\\"echo hi > /tmp/x\\"]}"\r\n');
            term.write('  curl -s -X POST https://relay.traits.build/relay/call -H "Content-Type: application/json" -d "{\\"code\\":\\"CODE\\",\\"path\\":\\"linux.vfs.read\\",\\"args\\":[\\"/tmp/x\\"]}"\r\n');
            term.write('  curl -s -X POST https://relay.traits.build/relay/call -H "Content-Type: application/json" -d "{\\"code\\":\\"CODE\\",\\"path\\":\\"linux.vfs.write\\",\\"args\\":[\\"/tmp/x\\",\\"hello\\n\\",\\"truncate\\"]}"\r\n');
            return true;
        }

        if (sub === 'status') {
            const code = shellRelayCode || shellRelayLoadCode();
            term.write('[shell-relay] status=' + (shellRelayRunning ? 'on' : 'off') + (code ? (' code=' + code) : '') + '\r\n');
            return true;
        }

        if (sub === 'off') {
            shellRelayStop = true;
            shellRelayRunning = false;
            term.write('[shell-relay] disconnected\r\n');
            return true;
        }

        if (sub === 'on') {
            if (shellRelayRunning) {
                term.write('[shell-relay] already connected code=' + shellRelayCode + '\r\n');
                return true;
            }
            const wanted = (parts[2] || shellRelayLoadCode() || '').toUpperCase();
            try {
                const code = await shellRelayRegister(wanted);
                shellRelaySaveCode(code);
                term.write('[shell-relay] connected code=' + code + '\r\n');
                term.write('[shell-relay] host can now call linux.exec via relay/call\r\n');
                shellRelayLoop(code);
            } catch (e) {
                term.write('[shell-relay] connect failed: ' + (e && e.message ? e.message : String(e)) + '\r\n');
            }
            return true;
        }

        term.write('[shell-relay] unknown command: ' + sub + '\r\n');
        return true;
    }

    async function runAutoFixCommand(cmd) {
        const trimmed = String(cmd || '').trim();
        if (!/^autofix\b/i.test(trimmed)) return false;

        const parts = trimmed.split(/\s+/);
        const sub = (parts[1] || 'status').toLowerCase();

        if (sub === 'on') {
            autoFixOnErrorEnabled = true;
            term.write('[auto-fix] on (failed commands will be wrapped with exit sentinel and may hand off to agent)\r\n');
            return true;
        }

        if (sub === 'off') {
            autoFixOnErrorEnabled = false;
            term.write('[auto-fix] off (commands run directly)\r\n');
            return true;
        }

        if (sub === 'status') {
            term.write('[auto-fix] status=' + (autoFixOnErrorEnabled ? 'on' : 'off') + '\r\n');
            return true;
        }

        term.write('[auto-fix] commands:\r\n');
        term.write('  autofix status\r\n');
        term.write('  autofix on\r\n');
        term.write('  autofix off\r\n');
        return true;
    }

    async function runNetModeCommand(cmd) {
        const trimmed = String(cmd || '').trim();
        if (!/^netmode\b/i.test(trimmed)) return false;

        const parts = trimmed.split(/\s+/);
        const sub = (parts[1] || 'status').toLowerCase();

        if (sub === 'status') {
            const mode = (typeof NetProxy !== 'undefined' && NetProxy.getMode) ? NetProxy.getMode() : 'unknown';
            const policy = (typeof NetProxy !== 'undefined' && NetProxy.getPolicy) ? NetProxy.getPolicy() : netPolicy;
            term.write('[netmode] policy=' + policy + ' mode=' + mode + '\r\n');
            term.write('[netmode] usage: netmode relay-only | netmode hybrid | netmode status\r\n');
            return true;
        }

        if (sub !== 'relay-only' && sub !== 'hybrid') {
            term.write('[netmode] unknown option: ' + sub + '\r\n');
            term.write('[netmode] usage: netmode relay-only | netmode hybrid | netmode status\r\n');
            return true;
        }

        netPolicy = sub;
        try { localStorage.setItem(NET_POLICY_KEY, netPolicy); } catch (_) {}

        if (typeof NetProxy !== 'undefined' && NetProxy.setRelayOnly) {
            try { NetProxy.setRelayOnly(netPolicy === 'relay-only'); } catch (_) {}
        }

        if (netPolicy === 'hybrid') {
            term.write('[netmode] policy set to hybrid (browser fallback allowed when tunnel unavailable)\r\n');
            if (typeof NetProxy !== 'undefined' && NetProxy.forceBrowserFallback) {
                try { NetProxy.forceBrowserFallback(); } catch (_) {}
            }
        } else {
            term.write('[netmode] policy set to relay-only (TCP/UDP require tunnel; no browser fallback)\r\n');
            if (typeof NetProxy !== 'undefined' && NetProxy.setTunnelURL && tunnelUrl) {
                try { NetProxy.setTunnelURL(tunnelUrl); } catch (_) {}
            }
        }

        return true;
    }

    async function runInitramfsCurlCommand(cmd) {
        const trimmed = String(cmd || '').trim();
        if (!/^curl\b/i.test(trimmed) || !/initramfs:\/\//i.test(trimmed)) return false;

        const methodMatch = trimmed.match(/(?:^|\s)-X\s+([A-Za-z]+)/);
        const method = (methodMatch ? methodMatch[1] : 'GET').toUpperCase();

        const dataMatch = trimmed.match(/(?:--data-binary|-d)\s+(?:'([\s\S]*?)'|"([\s\S]*?)"|([^\s]+))/);
        const payload = dataMatch ? (dataMatch[1] || dataMatch[2] || dataMatch[3] || '') : '';

        const urlMatch = trimmed.match(/initramfs:\/\/[^\s]+/i);
        if (!urlMatch) {
            term.write('[initramfs] usage: curl initramfs:///path\r\n');
            term.write('[initramfs] write: curl -X PUT initramfs:///path --data-binary "content"\r\n');
            return true;
        }

        const rawUrl = urlMatch[0];
        let path = '';
        const readQuery = rawUrl.match(/^initramfs:\/\/read\?path=(.+)$/i);
        if (readQuery) {
            path = decodeURIComponent(readQuery[1]);
        } else {
            path = rawUrl.replace(/^initramfs:\/\//i, '');
            if (!path.startsWith('/')) path = '/' + path;
            path = decodeURIComponent(path);
        }

        if (!path || path === '/') {
            term.write('[initramfs] missing path\r\n');
            return true;
        }

        if (method === 'GET') {
            const r = await shellExec('cat ' + shQuote(path));
            if (r.output) term.write(r.output + '\r\n');
            if (r.exitCode !== 0) term.write('[initramfs] read failed (exit ' + r.exitCode + ')\r\n');
            return true;
        }

        if (method === 'PUT' || method === 'POST' || method === 'PATCH') {
            const b64 = btoa(unescape(encodeURIComponent(payload)));
            const cmdWrite = "echo '" + b64 + "' > /tmp/.initramfs_curl && base64 -d /tmp/.initramfs_curl > " + shQuote(path) + " && rm /tmp/.initramfs_curl";
            const r = await shellExec(cmdWrite);
            if (r.exitCode === 0) {
                term.write('[initramfs] wrote ' + path + ' (' + payload.length + ' bytes)\r\n');
            } else {
                term.write('[initramfs] write failed (exit ' + r.exitCode + ')\r\n');
                if (r.output) term.write(r.output + '\r\n');
            }
            return true;
        }

        term.write('[initramfs] unsupported method: ' + method + '\r\n');
        return true;
    }

    async function executeWithAutoFixMonitor(cmd) {
        const rawCmd = String(cmd || '').trim();
        if (!rawCmd || agentRunning || autoFixInFlight || !autoFixOnErrorEnabled) return false;
        autoFixInFlight = true;

        const makeTask = (failedCmd, errOut) => {
            const clipped = String(errOut || '').slice(-6000);
            return [
                'A shell command failed. Fix it by running the correct command immediately.',
                'Requirements:',
                '- First action MUST be one concrete correction command.',
                '- Do not explain only; execute the fix.',
                '- Use the exact files/paths shown in error output.',
                '',
                'Failed command:',
                failedCmd,
                '',
                'Captured shell output:',
                clipped || '(no output)'
            ].join('\n');
        };

        return new Promise((resolve) => {
            let finished = false;
            let buffer = '';
            const previousCapture = agentCapture;

            const cleanup = () => {
                if (agentCapture === capture) agentCapture = previousCapture;
                autoFixInFlight = false;
            };

            const done = async (exitCode, output) => {
                if (finished) return;
                finished = true;
                cleanup();

                if (exitCode !== 0 && !agentRunning) {
                    term.write('\x1b[33m[auto-fix] command failed (exit ' + exitCode + '), handing off to agent...\x1b[0m\r\n');
                    try {
                        await runJSAgent(makeTask(rawCmd, output));
                    } catch (e) {
                        term.write('\x1b[31m[auto-fix] agent handoff failed: '
                            + (e && e.message ? e.message : String(e)) + '\x1b[0m\r\n');
                    }
                }
                resolve(true);
            };

            const capture = (data) => {
                const text = typeof data === 'string' ? data : new TextDecoder().decode(data);
                if (previousCapture) previousCapture(data);
                buffer += text;
                const m = buffer.match(AUTOFIX_SENTINEL_RE);
                if (!m) return;

                const exitCode = parseInt(m[1], 10);
                const before = buffer.slice(0, m.index)
                    .replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '')
                    .replace(/\r/g, '');
                const lines = before.split('\n');
                const output = lines.slice(1)
                    .filter(l => !KERNEL_DEBUG_RE.test(String(l || '').trim()))
                    .join('\n')
                    .trim();
                done(Number.isFinite(exitCode) ? exitCode : -1, output);
            };

            agentCapture = capture;
            os.key_input(rawCmd + '; printf "\\n__AFXEC:%s__\\n" "$?"\r');

            setTimeout(() => {
                if (!finished) {
                    cleanup();
                    resolve(true);
                }
            }, 35000);
        });
    }

    let consoleFilterCarry = '';
    const KERNEL_NOISE_RE = /^\[(Main|Runner)[^\]]*\]:/;
    const RCU_STALL_RE = /\b(?:a?grcu|rcu(?:_sched|_seched|_preempt)?)[^\n]*\b(?:stall|stnall|stnalls?)\b/i;
    let lastStallDiagMs = 0;

    function captureFreezeDiagnostics(reason, kernelLine) {
        const now = Date.now();
        // Avoid flooding diagnostics if the same stall line repeats quickly.
        if (now - lastStallDiagMs < 5000) return;
        lastStallDiagMs = now;
        try {
            const proxy = (typeof NetProxy !== 'undefined' && NetProxy.getStats) ? NetProxy.getStats() : null;
            const hostNet = (os && os.getNetworkMetrics) ? os.getNetworkMetrics() : null;
            const runtime = (os && os.getRuntimeMetrics) ? os.getRuntimeMetrics() : null;
            const diag = {
                ts: new Date(now).toISOString(),
                reason: String(reason || 'unknown'),
                kernelLine: String(kernelLine || ''),
                mode: (typeof NetProxy !== 'undefined' && NetProxy.getMode) ? NetProxy.getMode() : 'unknown',
                proxy,
                hostNet,
                runtime,
                agentRunning: !!agentRunning,
            };
            if (!window._linuxFreezeDiagnostics) window._linuxFreezeDiagnostics = [];
            window._linuxFreezeDiagnostics.push(diag);
            if (window._linuxFreezeDiagnostics.length > 20) window._linuxFreezeDiagnostics.shift();
            console.error('[freeze-diag]', diag);
            term.write('\x1B[33m[freeze-diag] ' + reason + ' | mode=' + diag.mode
                + ' cpu=' + (runtime ? runtime.cpuCount : '?')
                + ' tasks=' + (runtime ? runtime.taskCount : '?')
                + ' cb_age=' + (runtime ? Math.round(runtime.lastHostCallbackAgeMs) : '?') + 'ms\x1B[0m\r\n');
            if (kernelLine) {
                term.write('\x1B[33m[freeze-diag] kernel: ' + String(kernelLine).slice(0, 160) + '\x1B[0m\r\n');
            }
        } catch (e) {
            console.warn('[freeze-diag] failed to capture diagnostics', e);
        }
    }

    function writeConsoleFiltered(data) {
        const text = typeof data === 'string' ? data : new TextDecoder().decode(data);
        let combined = consoleFilterCarry + text;
        consoleFilterCarry = '';

        while (true) {
            const nl = combined.indexOf('\n');
            if (nl < 0) break;
            const line = combined.slice(0, nl + 1);
            combined = combined.slice(nl + 1);
            const trimmed = line.replace(/\r?\n$/, '');
            if (AUTOFIX_SENTINEL_RE.test(trimmed)) {
                continue;
            }
            if (RCU_STALL_RE.test(trimmed)) {
                captureFreezeDiagnostics('kernel-rcu-stall', trimmed);
            }
            if (KERNEL_NOISE_RE.test(trimmed)) {
                console.log('[linux/kernel]', trimmed);
            } else {
                term.write(line);
            }
        }

        // Keep possible partial kernel debug lines buffered until newline.
        if (combined.startsWith('[')) {
            consoleFilterCarry = combined;
        } else if (combined) {
            if (AUTOFIX_SENTINEL_RE.test(combined)) {
                return;
            }
            term.write(combined);
        }
    }

    const console_write = (data) => {
        // During agent command execution, suppress raw shell echo
        // (agent displays its own formatted output)
        if (!agentSuppressOutput) {
            writeConsoleFiltered(data);
        }
        // Agent output capture callback
        if (agentCapture) agentCapture(data);
        // Detect first interactive shell prompt (BusyBox prints hash-space or dollar-space)
        if (!shellReady) {
            const text = typeof data === 'string' ? data : new TextDecoder().decode(data);
            consoleBuffer += text;
            // Look for prompt at end of output — shell is ready when we see "prompt "
            const tail = consoleBuffer.slice(-4);
            if (tail.endsWith('$ ') || tail.endsWith('> ') || (tail.includes('#') && tail.endsWith(' '))) {
                shellReady = true;
                shellReadyResolve();
                // Now safe to enable relay tunnel (SMP bring-up is complete)
                if (typeof deferTunnelSetup === 'function') {
                    deferTunnelSetup();
                }
            }
            // Don't let buffer grow unbounded during boot
            if (consoleBuffer.length > 4096) consoleBuffer = consoleBuffer.slice(-512);
        }
    };

    let os;
    let netStatsTimer = null;
    let tunnelStallStart = null;  // Track when tunnel stops receiving packets
    try {
        os = await linux(workerUrl, vmlinux, boot_cmdline, initrd, logLine, console_write);
        // Expose for programmatic testing (e.g. os.key_input("cmd\r"))
        window._linuxOS = os;

        // Periodic network observability: mode, queue depth, drops, callback timings.
        let lastNetLine = '';
        let lastMode = bootNetMode;
        netStatsTimer = setInterval(() => {
            try {
                const proxy = (typeof NetProxy !== 'undefined' && NetProxy.getStats) ? NetProxy.getStats() : null;
                const host = (os && os.getNetworkMetrics) ? os.getNetworkMetrics() : null;
                const runtime = (os && os.getRuntimeMetrics) ? os.getRuntimeMetrics() : null;
                if (!proxy && !host) return;

                const mode = (typeof NetProxy !== 'undefined' && NetProxy.getMode) ? NetProxy.getMode() : 'unknown';
                if (mode !== lastMode) {
                    if (lastMode === 'browser-fallback' && mode === 'tunnel') {
                        console.log('[net] upgraded: browser-fallback -> tunnel');
                    } else {
                        console.log(`[net] mode changed: ${lastMode} -> ${mode}`);
                    }
                    lastMode = mode;
                    tunnelStallStart = null;  // Reset stall timer on mode change
                }
                const q = proxy ? proxy.queueLen : 0;
                const qh = proxy ? proxy.queueHighWater : 0;
                const drop = proxy ? proxy.rxDroppedPackets : 0;
                const rxPkts = proxy ? proxy.rxPackets : 0;
                const txPkts = proxy ? proxy.txPackets : 0;
                const cbRecv = host ? host.recvAvgMs : 0;
                const cbPoll = host ? host.pollAvgMs : 0;
                const line = `[net] mode=${mode} q=${q}/${qh} drop=${drop} rx=${rxPkts} tx=${txPkts} cb_recv=${cbRecv.toFixed(3)}ms cb_poll=${cbPoll.toFixed(3)}ms`;
                if (line !== lastNetLine) {
                    console.log(line);
                    lastNetLine = line;
                }

                // Tunnel stall detection: if tunnel mode but no RX packets while TX is happening,
                // force fallback to browser mode to unblock the kernel's network driver from polling
                // endlessly and starving host callbacks.
                if (mode === 'tunnel' && txPkts > 0 && rxPkts === 0) {
                    if (!tunnelStallStart) {
                        tunnelStallStart = Date.now();
                        console.warn('[net] ⚠️  tunnel stall detected: TX=' + txPkts + ' RX=' + rxPkts);
                    } else {
                        const stallDurationMs = Date.now() - tunnelStallStart;
                        if (stallDurationMs > 3000) {  // 3 second threshold
                            console.error('[net] ❌ tunnel unresponsive for ' + stallDurationMs + 'ms, forcing fallback to browser mode');
                            if (netPolicy === 'relay-only') {
                                term.write('\x1B[33m[traits.build] NET timeout: relay tunnel unresponsive (relay-only policy keeps fallback disabled)\x1B[0m\r\n');
                                tunnelStallStart = null;
                            } else {
                                term.write('\x1B[31m[traits.build] NET timeout: relay tunnel unresponsive, switching to browser emulation\x1B[0m\r\n');
                                if (typeof NetProxy !== 'undefined' && NetProxy.forceBrowserFallback) {
                                    try {
                                        NetProxy.forceBrowserFallback();
                                        tunnelStallStart = null;
                                        console.log('[net] forced fallback initiated');
                                    } catch (e) {
                                        console.error('[net] failureForcing fallback:', e);
                                    }
                                }
                            }
                        }
                    }
                } else if (tunnelStallStart && (rxPkts > 0 || mode !== 'tunnel')) {
                    // Tunnel recovered or mode changed, reset stall tracking
                    tunnelStallStart = null;
                }

                // Watchdog: if host callbacks stop for too long while tasks are alive,
                // capture diagnostics before a full freeze becomes unrecoverable.
                if (runtime && runtime.taskCount > 0 && runtime.lastHostCallbackAgeMs > 15000) {
                    captureFreezeDiagnostics('host-callback-starvation',
                        'last_callback=' + runtime.lastHostCallbackMethod + ' age_ms=' + Math.round(runtime.lastHostCallbackAgeMs));
                }
            } catch (e) {
                // Keep runtime robust even if metrics collection fails.
            }
        }, 5000);

        // Do NOT revoke workerUrl here! linux() returns immediately but
        // CPU 0 boots async and will create secondary CPUs + user tasks
        // later by calling new Worker(workerUrl). Revoke on page cleanup.
        setProgress(100);
    } catch (err) {
        term.write('\r\n\x1B[1;31m[ERROR] ' + err.message + '\x1B[0m\r\n');
        console.error('Linux/WASM boot failed:', err);
        return;
    }

    // ── JS Agent: execute a shell command and capture output ──
    // Sends "cmd; echo __EC:$?__\r" to the shell, captures all console
    // output, and resolves when the __EC:N__ sentinel appears.
    // NOTE: Do NOT use 2>&1 — it triggers restore_redirects→free() crash
    // with CLONE_VM heap corruption in BusyBox hush.
    const EXEC_TIMEOUT = 30000;
    const KERNEL_CRASH_RE = /Kernel panic|BUG!|Wasm crash|null function or function signature mismatch/;
    const KERNEL_DEBUG_RE = /^\[(?:Runner|Main)[^\]]*\]:/;
    async function shellExec(cmd) {
        return new Promise(resolve => {
            let buffer = '';
            let resolved = false;
            let abortCheckInterval = null;
            
            const finishExecution = (output, exitCode, crashed = false) => {
                if (resolved) return;
                resolved = true;
                agentCapture = null;
                agentSuppressOutput = false;
                if (abortCheckInterval) clearInterval(abortCheckInterval);
                console.log('[agent] shellExec finished:', { cmd, exitCode, outputLen: output.length, output: output.slice(0, 200) });
                resolve({ output, exitCode, crashed });
            };
            
            agentCapture = (data) => {
                if (resolved) return;
                // Ensure data is a string (console_write may receive Uint8Array)
                const text = typeof data === 'string' ? data : new TextDecoder().decode(data);
                buffer += text;
                // Early crash detection: abort immediately on kernel panic
                if (KERNEL_CRASH_RE.test(buffer)) {
                    finishExecution('[kernel crashed]', -2, true);
                    console.error('[agent] Kernel crash detected during:', cmd);
                    return;
                }
                const m = buffer.match(/__EC:(\d+)__/);
                if (m) {
                    const exitCode = parseInt(m[1], 10);
                    // Extract output: everything between echoed command and sentinel.
                    // Use m.index (regex match position) NOT indexOf('__EC:') because
                    // the echoed command itself contains '__EC:$?__' which would match first.
                    const beforeSentinel = buffer.slice(0, m.index);
                    const clean = beforeSentinel
                        .replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '')  // strip ANSI CSI
                        .replace(/\r/g, '');
                    const lines = clean.split('\n');
                    // First line is the echoed command, skip it.
                    // Filter kernel debug lines ([Runner...]:, [Main]:) from output.
                    const output = lines.slice(1)
                        .filter(l => !KERNEL_DEBUG_RE.test(l.trim()))
                        .join('\n').trim();
                    finishExecution(output, exitCode);
                }
            };
            agentSuppressOutput = true;
            // No 2>&1! Avoids CLONE_VM restore_redirects crash.
            os.key_input(cmd + '; echo __EC:$?__\r');
            
            // Poll for agentAbort flag every 100ms to interrupt faster
            abortCheckInterval = setInterval(() => {
                if (agentAbort && !resolved) {
                    clearInterval(abortCheckInterval);
                    resolved = true;
                    console.warn('[agent] shellExec ABORT triggered for:', cmd);
                    agentCapture = null;
                    agentSuppressOutput = false;
                    // Send multiple Ctrl+C signals (aggressive interrupt)
                    for (let i = 0; i < 3; i++) {
                        try { os.key_input('\x03'); } catch (e) {}
                    }
                    // Also try Ctrl+Z to suspend
                    try { os.key_input('\x1a'); } catch (e) {}
                    // Send newline to clear any stuck input
                    try { os.key_input('\r'); } catch (e) {}
                    resolve({ output: '[aborted by user]', exitCode: -128 });
                }
            }, 100);
            
            setTimeout(() => {
                if (!resolved) {
                    clearInterval(abortCheckInterval);
                    resolved = true;
                    console.warn('[agent] shellExec TIMEOUT for:', cmd, 'buffer:', buffer.slice(0, 500));
                    agentCapture = null;
                    agentSuppressOutput = false;
                    // Send multiple Ctrl+C signals (more aggressive than before)
                    for (let i = 0; i < 5; i++) {
                        try { os.key_input('\x03'); } catch (e) {}
                    }
                    // Try Ctrl+Z as well
                    try { os.key_input('\x1a'); } catch (e) {}
                    try { os.key_input('\r'); } catch (e) {}
                    resolve({ output: '[timeout after 30s]', exitCode: -1 });
                }
            }, EXEC_TIMEOUT);
        });
    }

    // ── JS Agent: main agent loop ──
    // Runs entirely in JavaScript. Uses browser fetch() for OpenAI API calls
    // (zero forks). Shell commands are restricted to builtins only (echo, for,
    // while read, test, etc.) which do NOT fork. External commands would crash
    // the WASM NOMMU kernel due to CLONE_VM shared-memory corruption.
    let agentAbort = false;

    // Resolve API key: /tmp/key in guest (via persist state or shellExec) takes
    // priority over localStorage secret, so `echo 'sk-...' > /tmp/key` just works.
    async function resolveApiKey() {
        // 1. Check persist store for /tmp/key (instant, no fork)
        const state = persistLoad();
        const persistKey = (state.files && state.files['/tmp/key']) ? state.files['/tmp/key'].trim() : '';
        if (persistKey && persistKey.startsWith('sk-')) return persistKey;

        // 2. Read /tmp/key from guest via shellExec (one fork)
        try {
            const r = await shellExec("cat /tmp/key");
            const guestKey = (r && r.exitCode === 0 && r.output) ? r.output.trim() : '';
            if (guestKey && guestKey.startsWith('sk-')) {
                // Save to persist so next check is instant
                state.files['/tmp/key'] = guestKey;
                persistSave(state);
                return guestKey;
            }
        } catch (e) {}

        // 3. Fall back to localStorage secret
        return (localStorage.getItem('traits.secret.OPENAI_API_KEY') || '').trim() || null;
    }

    async function resolveAgentMaxRounds() {
        const clamp = (n) => {
            if (!Number.isFinite(n)) return 100;
            return Math.max(1, Math.min(100, Math.floor(n)));
        };

        // 1) URL override for quick testing: ?agent_rounds=50
        try {
            const params = new URLSearchParams(location.search);
            const q = params.get('agent_rounds');
            if (q) return clamp(Number(q));
        } catch (e) {}

        // 2) Persistent override: localStorage['linux-wasm.agent-rounds']
        try {
            const s = localStorage.getItem('linux-wasm.agent-rounds') || '';
            if (s) return clamp(Number(s));
        } catch (e) {}

        // 3) Default for web agent loop.
        // Do NOT read /bin/agent.js here: that file lives inside the guest initramfs
        // and can be stale relative to the browser-side JS agent implementation.
        return 100;
    }

    function getSessionMemoryCount() {
        try {
            const sessions = JSON.parse(localStorage.getItem('linux-wasm.agent-sessions') || '[]');
            return Array.isArray(sessions) ? sessions.length : 0;
        } catch (e) {
            return 0;
        }
    }

    // Load session memory: retrieves past session history from localStorage (rolling window)
    function loadSessionMemory() {
        try {
            const sessions = JSON.parse(localStorage.getItem('linux-wasm.agent-sessions') || '[]');
            // Keep last 5 sessions, trim each to last 1000 chars
            const recent = sessions.slice(-5).map(s => (s && s.slice ? s.slice(-1000) : '')).filter(s => s);
            return recent.join('\n---SESSION BOUNDARY---\n');
        } catch (e) {
            return '';
        }
    }

    // Save current session to memory
    function saveSessionMemory(sessionHistory) {
        try {
            const sessions = JSON.parse(localStorage.getItem('linux-wasm.agent-sessions') || '[]');
            sessions.push(sessionHistory);
            // Keep last 10 sessions max
            if (sessions.length > 10) sessions.shift();
            localStorage.setItem('linux-wasm.agent-sessions', JSON.stringify(sessions));
        } catch (e) {}
    }

    async function runJSAgent(task) {
        agentRunning = true;
        agentAbort = false;

        function countSinglePipes(s) {
            let n = 0;
            for (let i = 0; i < s.length; i++) {
                if (s[i] !== '|') continue;
                const prev = i > 0 ? s[i - 1] : '';
                const next = i + 1 < s.length ? s[i + 1] : '';
                // Ignore logical OR (||); count only standalone pipeline separators.
                if (prev === '|' || next === '|') continue;
                n += 1;
            }
            return n;
        }

        function extractTaskFiles(taskText) {
            const raw = String(taskText || '');
            const re = /([a-zA-Z0-9_./-]+\.[a-zA-Z0-9_-]+)/g;
            const out = [];
            const seen = new Set();
            let m;
            while ((m = re.exec(raw)) !== null) {
                const f = m[1];
                if (!f || seen.has(f)) continue;
                seen.add(f);
                out.push(f);
            }
            return out;
        }

        function normalizeCandidatePath(fileName) {
            const f = String(fileName || '').trim();
            if (!f) return '';
            if (f.startsWith('/')) return f;
            return '/bin/' + f;
        }

        function inferSourcePath(taskText) {
            const t = String(taskText || '');
            const files = extractTaskFiles(t);
            if (files.length === 0) return '';
            const m = t.match(/(?:copy|from|update|modify|edit|based on)\s+([a-zA-Z0-9_./-]+\.[a-zA-Z0-9_-]+)/i);
            if (m && m[1]) return normalizeCandidatePath(m[1]);
            return normalizeCandidatePath(files[0]);
        }

        function inferTargetPath(taskText) {
            const t = String(taskText || '');
            const files = extractTaskFiles(t);
            if (files.length === 0) return '';
            const m = t.match(/(?:create|build|write|add)\s+(?:new\s+)?([a-zA-Z0-9_./-]+\.[a-zA-Z0-9_-]+)/i);
            if (m && m[1]) return normalizeCandidatePath(m[1]);
            if (files.length > 1) return normalizeCandidatePath(files[files.length - 1]);
            return normalizeCandidatePath(files[0]);
        }

        function isEditCommand(cmdText) {
            const c = String(cmdText || '').trim();
            return /\b(sed\s+-i|echo\b.*>|cat\b.*>|cp\b|mv\b|tee\b)\b/.test(c);
        }

        function isVerificationCommand(cmdText) {
            const c = String(cmdText || '').trim();
            return /^(cat|grep|head|tail|wc|test\s+-f|ls)\b/.test(c);
        }

        function referencesPath(cmdText, filePath) {
            const c = String(cmdText || '');
            const p = String(filePath || '');
            if (!p) return false;
            if (c.includes(p) || c.includes(p.replace(/^\//, ''))) return true;

            // Allow directory-agnostic verification reads (e.g. inferred /bin/game.qjs
            // while actual file lives at /home/game.qjs). This avoids false DONE blocks
            // when the correct source filename is read from a different valid location.
            const basename = p.split('/').filter(Boolean).pop() || '';
            if (!basename) return false;
            const slashName = '/' + basename;
            return c.includes(slashName) || c.includes(' ' + basename) || c.includes("'" + basename + "'") || c.includes('"' + basename + '"');
        }

        function hasImplementationIntent(taskText) {
            const t = String(taskText || '').toLowerCase();
            return /(update|modify|edit|implement|fix|add|create|build|write|store|save)/.test(t);
        }

        function hasConfigBehaviorIntent(taskText) {
            const t = String(taskText || '').toLowerCase();
            return /(without\s+param|without\s+argument|no\s+param|no\s+argument|interactiv|prompt|ask|stores?\s+.*config|saves?\s+.*config)/.test(t);
        }

        function hasBehaviorEvidence(hist) {
            const h = String(hist || '').toLowerCase();
            return /(usage:|config|prompt|question|readline|std\.in|argv\.length|if\s*\(.*argv|out:\s+.*config|out:\s+.*usage)/.test(h);
        }

        function hasLuaIntent(taskText, sourcePath, targetPath) {
            const t = String(taskText || '').toLowerCase();
            const s = String(sourcePath || '').toLowerCase();
            const d = String(targetPath || '').toLowerCase();
            return /(^|\s)lua(\s|$)|\.lua\b/.test(t) || s.endsWith('.lua') || d.endsWith('.lua');
        }

        function isLuaExecutionCommand(cmdText) {
            const c = String(cmdText || '').trim();
            return /^lua\s+/.test(c);
        }

        function hasLuaErrorOutput(outputText) {
            const out = String(outputText || '').toLowerCase();
            return /(lua error|unfinished string|expected near|syntax error|attempt to|\[string\s+".*"\]:\d+)/.test(out);
        }

        function hasUnbalancedQuotes(text) {
            const s = String(text || '');
            let single = false;
            let double = false;
            for (let i = 0; i < s.length; i++) {
                const ch = s[i];
                const prev = i > 0 ? s[i - 1] : '';
                if (ch === "'" && !double) single = !single;
                else if (ch === '"' && !single && prev !== '\\') double = !double;
            }
            return single || double;
        }

        function hasEscapedSingleQuoteInsideSingleQuotes(text) {
            const s = String(text || '');
            let single = false;
            let double = false;
            for (let i = 0; i < s.length; i++) {
                const ch = s[i];
                const prev = i > 0 ? s[i - 1] : '';
                if (ch === "'" && !double) {
                    // In POSIX shells, \' is not valid inside single quotes and can leave hush in continuation mode.
                    if (single && prev === '\\') return true;
                    single = !single;
                    continue;
                }
                if (ch === '"' && !single && prev !== '\\') double = !double;
            }
            return false;
        }

        function autoRepairSedEscapedSingleQuote(cmdText) {
            const cmd = String(cmdText || '');
            if (!/^\s*sed\b/i.test(cmd)) return '';
            if (!hasEscapedSingleQuoteInsideSingleQuotes(cmd)) return '';
            const m = cmd.match(/^(\s*sed\b[^'"]*)'([^']*\\'[^']*)'([\s\S]*)$/i);
            if (!m) return '';

            const before = m[1] || '';
            const body = m[2] || '';
            const after = m[3] || '';
            const normalized = body
                .replace(/\\'/g, "'")
                .replace(/\\/g, '\\\\')
                .replace(/"/g, '\\"')
                .replace(/\$/g, '\\$')
                .replace(/`/g, '\\`');

            return before + '"' + normalized + '"' + after;
        }

        function validateShellCommand(cmdText) {
            const cmd = String(cmdText || '');
            const trimmed = cmd.trim();
            if (!trimmed) return 'empty command';
            if (/\r|\n/.test(cmd)) return 'multi-line commands are not allowed';
            if (/\\\s*$/.test(trimmed)) return 'command ends with a shell continuation backslash';
            if (hasUnbalancedQuotes(trimmed)) return 'command has unbalanced quotes';
            if (hasEscapedSingleQuoteInsideSingleQuotes(trimmed)) {
                return "command uses \\' inside single-quoted text (invalid in BusyBox hush)";
            }
            if (/<<|`|\$\(/.test(trimmed)) return 'command uses unsupported shell quoting/substitution';
            if (/\bsed\b[^\n]*\b[aci]\\\s*$/i.test(trimmed)) return 'busybox sed append/insert/change with trailing backslash will hang';
            if (/\bsed\b[^\n]*['"][^'"]*[aci]\\['"]?/i.test(trimmed) && /\b[aci]\\\s*$/i.test(trimmed)) {
                return 'busybox sed multi-line append/insert/change is unsafe here';
            }
            return '';
        }

        function repairPlaceholderPaths(cmdText, sourcePath, targetPath) {
            let out = String(cmdText || '');
            if (!out.includes('/path/')) return out;

            if (sourcePath) {
                out = out.replace(/\/path\/source-config\.js/g, sourcePath);
                out = out.replace(/\/path\/source\.js/g, sourcePath);
            }
            if (targetPath) {
                out = out.replace(/\/path\/config\.js/g, targetPath);
                out = out.replace(/\/path\/target\.js/g, targetPath);
            }

            // Last resort: any remaining /path/<file>.js placeholder maps to inferred source.
            if (sourcePath) out = out.replace(/\/path\/[^\s|;&]+\.js/g, sourcePath);
            return out;
        }

        const apiKey = await resolveApiKey();
        if (!apiKey) {
            term.write('\x1b[31mNo API key. Set OPENAI_API_KEY in Settings or write to /tmp/key.\x1b[0m\r\n');
            agentRunning = false;
            os.key_input('\r');
            return;
        }

        const SYS = 'You are a shell agent inside BusyBox Linux/WASM (hush shell).\n' +
            'Process slots are limited. No pipelines, no $(...), no backticks, no while/for loops.\n\n' +
            'YOU MUST TAKE THIS TASK SERIOUSLY. Failing or creating dummy placeholders is unacceptable.\n\n' +
            'FILE SEARCH PROTOCOL:\n' +
            '  When looking for a required source file:\n' +
            '  1. Search in standard script locations: /bin/, /usr/bin/, /usr/local/bin/\n' +
            '  2. Check current working directory (pwd)\n' +
            '  3. Check root directory (/) for root-level scripts\n' +
            '  4. Check /home/ or /root/ for user scripts\n' +
            '  5. NEVER create a dummy/placeholder file if source is not found\n' +
            '  6. If source does not exist, state this clearly and stop\n' +
            '  7. Verify file contents with cat BEFORE claiming success\n\n' +
            'LUA VALIDATION PROTOCOL:\n' +
            '  If task creates or edits a .lua file, you MUST run: lua <file.lua>\n' +
            '  If Lua reports any parse/runtime error, fix the file and rerun lua <file.lua>.\n' +
            '  DONE is allowed only after a clean lua run for edited .lua files.\n\n' +
            'RESPONSE FORMAT (every round):\n' +
            'THINK: <1-2 sentences: what you learned, what you plan to do next, why>\n' +
            'CMD: <exactly one shell command>\n\n' +
            'When finished:\n' +
            'THINK: <what was accomplished>\n' +
            'DONE: <summary of changes made>\n\n' +
            'AVAILABLE COMMANDS:\n' +
            '  cat /path/file                         — read file\n' +
            '  test -f /path && echo exists || echo missing  — check file\n' +
            '  ls /path                               — list directory\n' +
            '  echo "content" > /path/file             — create/overwrite file\n' +
            '  echo "content" >> /path/file            — append to file\n' +
            '  printf "%s\\n" "line1" "line2" > /path/file — rewrite file safely\n' +
            '  sed -i \'s/old/new/\' /path/file         — edit in-place\n' +
            '  sed -i -e \'s/a/b/\' -e \'s/c/d/\' /path  — multi-edit\n' +
            '  head -n N /path/file                    — first N lines\n' +
            '  tail -n N /path/file                    — last N lines\n' +
            '  wc -l /path/file                        — line count\n\n' +
            '  lua /path/file.lua                       — execute Lua file and show errors\n\n' +
            'CONSTRAINTS:\n' +
            '  - ONE command per round (no ; or && chains except test -f pattern)\n' +
            '  - No pipelines (|). No 2>&1. No /proc/* or /sys/*\n' +
            '  - sed patterns must use exact text from the file, never placeholders or ellipsis\n' +
            '  - NEVER use sed append/insert/change forms ending in a backslash (a\\, i\\, c\\); they hang this shell\n' +
            '  - NEVER send commands with raw newlines, unmatched quotes, heredocs, backticks, or $(...)\n' +
            '  - NEVER create placeholder/dummy files as fallback when source not found\n' +
            '  - To create a new file, use echo or tee — one line at a time if needed\n' +
            '  - If output was truncated, use head/tail/sed -n to read specific line ranges\n' +
            '  - VERIFY REAL FILE CONTENTS with cat after successful creation\n';

        async function rewriteUnsafeCommand(originalCmd, reason, taskText, hist, sourcePath, targetPath) {
            const repairSystem = 'You rewrite ONE unsafe BusyBox hush shell command into ONE safe single-line command.\n'
                + 'Return exactly one of:\n'
                + 'CMD: <single-line safe shell command>\n'
                + 'BLOCKED: <short reason>\n\n'
                + 'Rules:\n'
                + '- single line only\n'
                + '- no raw newlines in the command\n'
                + '- no trailing continuation backslash\n'
                + '- no heredocs, no backticks, no $()\n'
                + '- no sed a\\, i\\, or c\\ forms\n'
                + "- never use \\' inside single-quoted shell strings; use double quotes when needed\n"
                + '- prefer printf "%s\\n" ... > file for larger rewrites\n'
                + '- prefer single-line sed -i s/// edits for small exact changes\n'
                + '- preserve the user intent\n';
            const repairUser = 'Task: ' + String(taskText || '') + '\n'
                + 'Unsafe command: ' + String(originalCmd || '') + '\n'
                + 'Reason blocked: ' + String(reason || '') + '\n'
                + 'Inferred source path: ' + String(sourcePath || '') + '\n'
                + 'Inferred target path: ' + String(targetPath || '') + '\n'
                + 'Recent history:\n' + String(hist || '').slice(-2500);
            try {
                const resp = await fetch('https://relay.traits.build/llm/proxy', {
                    method: 'POST',
                    headers: {
                        'Authorization': 'Bearer ' + apiKey,
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({
                        model: 'gpt-5.4',
                        messages: [
                            { role: 'system', content: repairSystem },
                            { role: 'user', content: repairUser }
                        ],
                        max_tokens: 220,
                        temperature: 0
                    })
                });
                const data = await resp.json();
                if (data.error) return '';
                const text = (((data.choices || [])[0] || {}).message || {}).content || '';
                const blocked = text.match(/BLOCKED:\s*(.+)/i);
                if (blocked) return '';
                const match = text.match(/CMD:\s*([^\n]+)/i);
                if (!match || !match[1]) return '';
                let repaired = match[1].trim().replace(/^`+/, '').replace(/`+$/, '');
                repaired = repairPlaceholderPaths(repaired, sourcePath, targetPath);
                if (!repaired || repaired === originalCmd) return '';
                if (validateShellCommand(repaired)) return '';
                return repaired;
            } catch (e) {
                return '';
            }
        }

        let history = '';
        // Load and inject past session memory into rolling window
        const sessionMemory = loadSessionMemory();
        if (sessionMemory) {
            term.write('\x1b[2m[agent] loaded ' + getSessionMemoryCount() + ' past session(s)\x1b[0m\r\n');
            history = '[PAST SESSIONS]\n' + sessionMemory + '\n[END PAST SESSIONS]\n';
        }
        
        let blockedPipelineStreak = 0;
        let blockedTotal = 0;
        let blockedDummyFiles = 0;  // Count dummy file rejection attempts
        let lastCmd = '';
        let sameCmdStreak = 0;
        let agentExistsConfirmed = false;
        let hadSuccessfulEdit = false;
        let hadSuccessfulVerify = false;
        let hadSourceRead = false;
        let hadTargetRead = false;
        let lastEditedPath = '';
        const createIntent = /(create|build\s+new|new\s+\S+\.[a-z0-9]+|add\s+\S+\.[a-z0-9]+|write\s+\S+\.[a-z0-9]+)/i.test(task || '');
        const implementationIntent = hasImplementationIntent(task || '');
        const behaviorIntent = hasConfigBehaviorIntent(task || '');
        const inferredSourcePath = inferSourcePath(task);
        const inferredTargetPath = inferTargetPath(task);
        const luaIntent = hasLuaIntent(task || '', inferredSourcePath, inferredTargetPath);
        const MAX = await resolveAgentMaxRounds();
        let hadLuaRun = false;
        let hadLuaSuccess = false;
        let lastLuaError = '';

        term.write('\x1b[1;32m=== Agent: ' + task + ' ===\x1b[0m\r\n');
        term.write('\x1b[2m[agent] max rounds: ' + MAX + ' | model: gpt5.4\x1b[0m\r\n');
        console.log('[agent] Starting task:', task);

        for (let round = 1; round <= MAX; round++) {
            if (agentAbort) {
                term.write('\x1b[33m[Agent aborted by user]\x1b[0m\r\n');
                break;
            }

            term.write('\x1b[2m-- Round ' + round + '/' + MAX + ' --\x1b[0m\r\n');

            // Inject rolling window of history + past sessions (keep last 3000 chars max)
            const historyWindow = (history + userMsgHistory).slice(-3000);
            let userMsg = round === 1
                ? 'Task: ' + task + (sessionMemory ? '\n\n[Context from ' + JSON.parse(localStorage.getItem('linux-wasm.agent-sessions') || '[]').length + ' past session(s) available in history]' : '')
                : 'Task: ' + task + '\nHistory:\n' + historyWindow + '\nNext command or DONE: summary.';

            if (blockedPipelineStreak >= 2) {
                userMsg += '\nCRITICAL FORMAT: next command must be exactly one of these forms: '
                    + 'test -f /path && echo exists || echo missing ; '
                    + 'cat /path/file ; '
                    + 'sed -i \'s/old/new/\' /path/file ; '
                    + 'echo "text" > /path/file ; '
                    + 'DONE: summary';
            }

            if (agentExistsConfirmed) {
                userMsg += '\nSTATE: Source file existence is already confirmed (exists). Do NOT repeat test -f. '
                    + 'Next step: read source with cat, then execute concrete edits or create target file.';
            }

            if (luaIntent) {
                userMsg += '\nLUA TASK REQUIREMENT: For edited/created .lua files, run lua <file.lua>. '
                    + 'If errors appear, fix and rerun until clean. Do not emit DONE before a clean lua run.';
                if (lastLuaError) {
                    userMsg += '\nLast Lua error to fix: ' + lastLuaError;
                }
            }

            if (/REJECTED.*dummy placeholder/i.test(history)) {
                userMsg += '\n\nCRITICAL: You just tried to create a dummy file. This is WRONG. You must:\n'
                    + '1. Search standard script locations: /bin/, /usr/bin/, /usr/local/bin/, /root/, /home/\n'
                    + '2. If file exists, read with cat and proceed with edits\n'
                    + '3. If file does not exist, state clearly: "Source file not found at [path]"\n'
                    + '4. Use initramfs:// protocol as last resort: curl initramfs:///bin/required-file\n'
                    + '5. NEVER invent placeholder content. Either find the real file or admit failure.\n'
                    + 'Search the correct paths first.';
            }

            console.log('[agent] Round', round, 'userMsg:', userMsg.slice(0, 300));
            term.write('  \x1b[2m[calling LLM...]\x1b[0m\r\n');

            let cmd;
            let think = '';
            try {
                const resp = await fetch('https://relay.traits.build/llm/proxy', {
                    method: 'POST',
                    headers: {
                        'Authorization': 'Bearer ' + apiKey,
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({
                        model: 'gpt-5.4',
                        messages: [
                            { role: 'system', content: SYS },
                            { role: 'user', content: userMsg }
                        ],
                        max_tokens: 400,
                        temperature: 0
                    })
                });
                const data = await resp.json();
                if (data.error) {
                    const msg = typeof data.error === 'string' ? data.error : (data.error.message || JSON.stringify(data.error));
                    term.write('  \x1b[31mAPI error: ' + msg + '\x1b[0m\r\n');
                    if (/incorrect api key|invalid api key|unauthorized|401/i.test(String(msg))) {
                        term.write('  \x1b[33m[stopping: authentication error]\x1b[0m\r\n');
                        break;
                    }
                    continue;
                }
                cmd = data.choices[0].message.content.trim();
                console.log('[agent] Round', round, 'LLM response:', cmd);

                // Extract THINK line(s) and display them
                const thinkMatch = cmd.match(/THINK:\s*(.+)/i);
                if (thinkMatch) {
                    think = thinkMatch[1].trim();
                    term.write('  \x1b[36m💭 ' + think + '\x1b[0m\r\n');
                }
            } catch (err) {
                term.write('  \x1b[31mFetch error: ' + err.message + '\x1b[0m\r\n');
                if (/401|unauthorized|forbidden|incorrect api key|invalid api key/i.test(String(err && err.message ? err.message : err))) {
                    term.write('  \x1b[33m[stopping: authentication error]\x1b[0m\r\n');
                    break;
                }
                continue;
            }

            // Check for DONE anywhere in response (may be after THINK line).
            const doneMatch = cmd.match(/DONE:\s*([\s\S]*)/im);
            if (doneMatch) {
                const summary = doneMatch[1].trim().split('\n')[0] || 'Task complete';
                if (/^missing$/i.test(summary)) {
                    const alreadyFound = /\bOut:\s*exists\b/i.test(history) || /\n\s*\|\s*exists\s*\n/i.test(history);
                    if (createIntent || alreadyFound) {
                        term.write('  \x1b[31m[blocked: DONE: missing is invalid for this task]\x1b[0m\r\n');
                        history += 'Result: BLOCKED — task requires creating/editing a file; do not end with DONE: missing. Continue with concrete file commands.\n';
                        continue;
                    } else {
                        term.write('\r\n\x1b[1;32m=== DONE: ' + summary + ' ===\x1b[0m\r\n');
                        term.write('Completed in ' + round + ' round(s).\r\n');
                        break;
                    }
                } else {
                    if (implementationIntent && !hadSuccessfulEdit) {
                        term.write('  \x1b[31m[blocked: DONE rejected — no successful edit command executed]\x1b[0m\r\n');
                        history += 'Result: BLOCKED — DONE rejected. Execute at least one successful edit command before finishing.\n';
                        continue;
                    }
                    if (implementationIntent && !hadSuccessfulVerify) {
                        term.write('  \x1b[31m[blocked: DONE rejected — no verification command output]\x1b[0m\r\n');
                        history += 'Result: BLOCKED — DONE rejected. Run verification command(s) and show output before finishing.\n';
                        continue;
                    }
                    if (inferredSourcePath && implementationIntent && !hadSourceRead) {
                        term.write('  \x1b[31m[blocked: DONE rejected — source file was never read]\x1b[0m\r\n');
                        history += 'Result: BLOCKED — DONE rejected. Read the source file before claiming completion.\n';
                        continue;
                    }
                    if (inferredTargetPath && createIntent && !hadTargetRead) {
                        term.write('  \x1b[31m[blocked: DONE rejected — target file not read for verification]\x1b[0m\r\n');
                        history += 'Result: BLOCKED — DONE rejected. Read the target file to verify actual content.\n';
                        continue;
                    }
                    if (behaviorIntent && !hasBehaviorEvidence(history)) {
                        term.write('  \x1b[31m[blocked: DONE rejected — behavior requirement not evidenced]\x1b[0m\r\n');
                        history += 'Result: BLOCKED — DONE rejected. Provide output/code evidence for interactive/config behavior requirements.\n';
                        continue;
                    }
                    if (luaIntent && !hadLuaRun) {
                        term.write('  \x1b[31m[blocked: DONE rejected — Lua file not executed]\x1b[0m\r\n');
                        history += 'Result: BLOCKED — DONE rejected. Run lua <file.lua> and inspect output before finishing.\n';
                        continue;
                    }
                    if (luaIntent && !hadLuaSuccess) {
                        term.write('  \x1b[31m[blocked: DONE rejected — Lua run still failing]\x1b[0m\r\n');
                        if (lastLuaError) term.write('  \x1b[33m[last lua error] ' + lastLuaError + '\x1b[0m\r\n');
                        history += 'Result: BLOCKED — DONE rejected. Lua execution is failing; fix script and rerun lua <file.lua> until success.\n';
                        continue;
                    }
                    term.write('\r\n\x1b[1;32m=== DONE: ' + summary + ' ===\x1b[0m\r\n');
                    term.write('Completed in ' + round + ' round(s).\r\n');
                    break;
                }
            }

            // Extract CMD: line from structured response
            {
                const cmdMatch = cmd.match(/CMD:\s*(.+)/im);
                if (cmdMatch) {
                    cmd = cmdMatch[1].trim();
                } else {
                    // Fallback: strip markdown fences, take first non-THINK line
                    cmd = cmd.replace(/^```(?:sh|bash)?\n?/, '').replace(/\n?```$/, '');
                    const lines = cmd.split('\n').filter(l => !/^THINK:/i.test(l.trim()));
                    cmd = (lines[0] || '').trim();
                }
            }
            // Strip any leftover markdown fences from CMD value
            cmd = cmd.replace(/^`+/, '').replace(/`+$/, '');

            const repairedCmd = repairPlaceholderPaths(cmd, inferredSourcePath, inferredTargetPath);
            if (repairedCmd !== cmd) {
                term.write('  \x1b[33m[auto-repair] ' + cmd + ' -> ' + repairedCmd + '\x1b[0m\r\n');
                history += 'Cmd: ' + cmd + '\nResult: AUTO-REPAIR placeholder path -> ' + repairedCmd + '\n';
                cmd = repairedCmd;
            }

            // Block while-read loops — they hang on large files; the prompt instructs LLM to use cat.
            if (/while\s+IFS=|while\s+read/.test(cmd)) {
                term.write('  \x1b[31m[blocked: while-read loop hangs on large files — use cat instead]\x1b[0m\r\n');
                history += 'Cmd: ' + cmd + '\nResult: BLOCKED — while-read loops hang. Use: cat /path/file\n';
                console.log('[agent] Blocked while-read loop:', cmd);
                blockedTotal += 1;
                continue;
            }

            // CRITICAL: Block dummy file fallback pattern — agent giving up and creating placeholder
            // Pattern: create a file with placeholder content after source check failed
            if (/(echo.*['"](#|\/\/).*['"]\s*>\s*|^\s*echo\s+"[^"]*placeholder|^\s*echo\s+"[^"]*Sample|^\s*echo\s+"\/\/\s)/.test(cmd) && /missing|not found|does not exist|cannot access/i.test(history)) {
                blockedDummyFiles += 1;
                term.write('  \x1b[31m[REJECTED: Do not create dummy placeholder files]\x1b[0m\r\n');
                term.write('  \x1b[33m[Critical: Source file was not found, so you created a fake one.]\x1b[0m\r\n');
                term.write('  \x1b[33m[This is NOT acceptable. Required files must exist in their actual locations.]\x1b[0m\r\n');
                history += 'Cmd: ' + cmd + '\nResult: REJECTED — Cannot create dummy placeholder files when source is missing. '
                    + 'Search standard paths (/bin, /usr/bin, /home/, /root/) or use initramfs:// to fetch. See instructions.\n';
                console.log('[agent] Blocked dummy file fallback:', cmd, 'after history:', history.slice(-200));
                blockedTotal += 1;
                if (blockedDummyFiles >= 2) {
                    term.write('  \x1b[1;31m[STOPPING: Multiple dummy file attempts. Task cannot continue this way.]\x1b[0m\r\n');
                    term.write('  \x1b[33m[Source files must exist. Search /bin, /usr/bin, or other standard paths.]\x1b[0m\r\n');
                    break;
                }
                continue;
            }

            // Block complex multi-pipe commands; they often exhaust process slots or become malformed.
            const pipeCount = countSinglePipes(cmd);
            if (pipeCount > 1) {
                term.write('  \x1b[31m[blocked: complex pipeline causes Resource busy — use simpler single-step commands]\x1b[0m\r\n');
                history += 'Cmd: ' + cmd + '\nResult: BLOCKED — complex pipeline. Use simple rounds: test -f, cat file, create/modify target in separate commands, verify.\n';

                if (createIntent) {
                    history += 'Suggested steps (one per round):\n'
                        + '1) Check source file exists: test -f <source> && echo exists || echo missing\n'
                        + '2) Read source: cat <source>\n'
                        + '3) Create/modify target: echo "..." > <target> or sed -i edit\n'
                        + '4) Verify: cat <target>\n'
                        + '5) DONE: summary\n';
                }

                console.log('[agent] Blocked complex pipeline command:', cmd);
                blockedPipelineStreak += 1;
                blockedTotal += 1;
                if (blockedPipelineStreak >= 3 || blockedTotal >= 5) {
                    term.write('  \x1b[33m[stopping: repeated blocked commands; model did not adapt]\x1b[0m\r\n');
                    term.write('  \x1b[33m[tip: try a narrower prompt with explicit source path and exact desired change]\x1b[0m\r\n');
                    break;
                }
                continue;
            }

            // Prevent repeated no-op checks that stall progress.
            const sourceExistsCheck = inferredSourcePath
                ? new RegExp('^test\\s+-f\\s+' + inferredSourcePath.replace(/[.*+?^${}()|[\\]\\]/g, '\\\\$&') + '\\s+&&\\s+echo\\s+exists\\s+\\|\\|\\s+echo\\s+missing$', 'i')
                : null;
            if (sourceExistsCheck && sourceExistsCheck.test(cmd) && agentExistsConfirmed) {
                term.write('  \x1b[31m[blocked: source file already confirmed exists — move to read/edit step]\x1b[0m\r\n');
                history += 'Cmd: ' + cmd + '\nResult: BLOCKED — repeated existence check. Next step: read source with cat or apply concrete edit.\n';
                blockedTotal += 1;
                continue;
            }

            if (cmd === lastCmd) sameCmdStreak += 1;
            else sameCmdStreak = 0;

            if (sameCmdStreak >= 3) {
                term.write('  \x1b[31m[blocked: repeated identical command — advance to next step]\x1b[0m\r\n');
                history += 'Cmd: ' + cmd + '\nResult: BLOCKED — you already ran this command (output is in history above). Move to the next step.\n';
                blockedTotal += 1;
                continue;
            }
            lastCmd = cmd;

            blockedPipelineStreak = 0;

            if (!cmd) {
                term.write('  \x1b[2m[empty response]\x1b[0m\r\n');
                continue;
            }

            const autoRepairedSedQuote = autoRepairSedEscapedSingleQuote(cmd);
            if (autoRepairedSedQuote && autoRepairedSedQuote !== cmd) {
                term.write('  \x1b[33m[auto-repair sed quote] ' + cmd + ' -> ' + autoRepairedSedQuote + '\x1b[0m\r\n');
                history += 'Cmd: ' + cmd + '\nResult: AUTO-REPAIR sed quote escape -> ' + autoRepairedSedQuote + '\n';
                cmd = autoRepairedSedQuote;
            }

            let shellValidationError = validateShellCommand(cmd);
            if (shellValidationError) {
                const repairedUnsafe = await rewriteUnsafeCommand(cmd, shellValidationError, task, history, inferredSourcePath, inferredTargetPath);
                if (repairedUnsafe) {
                    term.write('  \x1b[33m[auto-repair unsafe shell] ' + cmd + ' -> ' + repairedUnsafe + '\x1b[0m\r\n');
                    history += 'Cmd: ' + cmd + '\nResult: AUTO-REPAIR unsafe shell command -> ' + repairedUnsafe + '\n';
                    cmd = repairedUnsafe;
                    shellValidationError = validateShellCommand(cmd);
                }
            }
            if (shellValidationError) {
                term.write('  \x1b[31m[blocked: unsafe shell command — ' + shellValidationError + ']\x1b[0m\r\n');
                term.write('  \x1b[33m[tip: prefer single-line sed substitutions or printf "%s\\n" ... > /path/file for rewrites]\x1b[0m\r\n');
                history += 'Cmd: ' + cmd + '\nResult: BLOCKED — unsafe shell command (' + shellValidationError + '). '
                    + 'Use a single-line command with balanced quotes and no continuation backslash.\n';
                blockedTotal += 1;
                continue;
            }

            // Guard malformed sed patches using ellipsis placeholders that corrupt files.
            if (/^sed\b/i.test(cmd) && /(\.\.\.|…)/.test(cmd)) {
                term.write('  \x1b[31m[blocked: sed command uses ellipsis placeholder — requires exact text]\x1b[0m\r\n');
                history += 'Cmd: ' + cmd + '\nResult: BLOCKED — sed replacement contains ellipsis placeholder (... or …). Use exact literal text from file.\n';
                blockedTotal += 1;
                continue;
            }

            term.write('  \x1b[33m$ ' + cmd + '\x1b[0m\r\n');

            // Execute in guest shell (this is the ONLY fork per round)
            console.log('[agent] Executing:', cmd);
            const { output, exitCode, crashed } = await shellExec(cmd);
            agentSuppressOutput = false;
            console.log('[agent] Exec result:', { exitCode, outputLen: output.length, crashed, output: output.slice(0, 300) });

            if (exitCode === 0) {
                if (isEditCommand(cmd)) {
                    hadSuccessfulEdit = true;
                    if (inferredTargetPath && referencesPath(cmd, inferredTargetPath)) lastEditedPath = inferredTargetPath;
                    else if (inferredSourcePath && referencesPath(cmd, inferredSourcePath)) lastEditedPath = inferredSourcePath;
                }
                if (isVerificationCommand(cmd)) hadSuccessfulVerify = true;
                if (inferredSourcePath && isVerificationCommand(cmd) && referencesPath(cmd, inferredSourcePath)) {
                    hadSourceRead = true;
                }
                if (inferredTargetPath && isVerificationCommand(cmd) && referencesPath(cmd, inferredTargetPath)) {
                    hadTargetRead = true;
                }
                if (!inferredTargetPath && lastEditedPath && isVerificationCommand(cmd) && referencesPath(cmd, lastEditedPath)) {
                    hadTargetRead = true;
                }
            }

            if (isLuaExecutionCommand(cmd)) {
                hadLuaRun = true;
                const luaFailed = exitCode !== 0 || hasLuaErrorOutput(output);
                if (luaFailed) {
                    const compact = String(output || '').replace(/\s+/g, ' ').trim();
                    lastLuaError = compact.slice(0, 300) || ('exit ' + exitCode);
                    hadLuaSuccess = false;
                } else {
                    hadLuaSuccess = true;
                    lastLuaError = '';
                }
            }

            // If kernel crashed, abort immediately — no recovery possible
            if (crashed) {
                term.write('  \x1b[1;31m[Kernel crashed — reboot required]\x1b[0m\r\n');
                term.write('  \x1b[2mPress ⟳ Reboot to restart the kernel.\x1b[0m\r\n');
                break;
            }

            // Timeout means shell is likely wedged; stop to avoid repeated hangs.
            if (exitCode === -1) {
                term.write('  \x1b[33m[command timed out, stopping agent]\x1b[0m\r\n');
                break;
            }

            // Display output
            if (output) {
                const lines = output.split('\n');
                const show = Math.min(lines.length, 40);
                for (let i = 0; i < show; i++) {
                    term.write('  \x1b[37m| ' + lines[i] + '\x1b[0m\r\n');
                }
                if (lines.length > 40) {
                    term.write('  \x1b[2m| ...(' + lines.length + ' lines total)\x1b[0m\r\n');
                }

                if (/resource busy|vfork: Resource busy/i.test(output)) {
                    term.write('  \x1b[33m[hint: split into single commands; avoid pipes and command chains]\x1b[0m\r\n');
                    history += 'Result: Resource busy from process-slot exhaustion. Retry with one command per round and no pipes.\n';
                }

                if (inferredSourcePath && /(^|\n)exists(\n|$)/i.test(output)) {
                    const sourceExistsCheck = new RegExp('^test\\s+-f\\s+' + inferredSourcePath.replace(/[.*+?^${}()|[\\]\\]/g, '\\$&') + '\\s+&&\\s+echo\\s+exists\\s+\\|\\|\\s+echo\\s+missing$', 'i');
                    if (sourceExistsCheck.test(cmd)) agentExistsConfirmed = true;
                }
            }
            term.write('  \x1b[2m[exit: ' + exitCode + ']\x1b[0m\r\n\r\n');

            // Build history for next round.
            // Keep full command output in model history to avoid losing file content.
            const fullOut = output;
            const thinkEntry = think ? 'Think: ' + think + '\n' : '';
            history += thinkEntry + 'Cmd: ' + cmd + '\nExit: ' + exitCode + '\nOut: ' + fullOut + '\n';

            // Rolling window: keep only the last 10 history entries so the
            // LLM always sees recent results instead of getting lost in a
            // massive context. Split on 'Think: ' or 'Cmd: ' prefix to count entries.
            const histEntries = history.split(/(?=^(?:Think|Cmd): )/m);
            if (histEntries.length > 10) {
                history = '(earlier rounds omitted)\n' + histEntries.slice(-10).join('');
            }
        }

        // Keep agentRunning=true during persist pull so keyboard input stays blocked
        // and shellExec sentinel detection isn't corrupted by user keystrokes.
        agentSuppressOutput = false;
        agentCapture = null;
        try {
            if (persistAutosyncEnabled()) {
                await persistPullFromGuest(true);
            }
        } catch (e) {
            console.error('[agent] persistPullFromGuest error:', e);
        }
        
        // Save this session to memory before finishing
        saveSessionMemory(history);
        
        agentRunning = false;
        agentSuppressOutput = false;
        agentCapture = null;
        // Get a fresh visible prompt
        await new Promise(r => setTimeout(r, 100));
        os.key_input('\r');
    }

    // ── Keyboard input ──
    // Single handler: capture-phase document listener handles ALL input.
    // We do NOT rely on xterm's term.onData because its hidden textarea
    // loses focus in the SPA context and silently stops receiving events.
    const KEY_SEQ = {
        'Enter':'\r', 'Backspace':'\x7f', 'Tab':'\t', 'Escape':'\x1b',
        'Delete':'\x1b[3~',
        'ArrowUp':'\x1b[A', 'ArrowDown':'\x1b[B',
        'ArrowRight':'\x1b[C', 'ArrowLeft':'\x1b[D',
        'Home':'\x1b[H', 'End':'\x1b[F',
        'PageUp':'\x1b[5~', 'PageDown':'\x1b[6~',
        'F1':'\x1bOP', 'F2':'\x1bOQ', 'F3':'\x1bOR', 'F4':'\x1bOS',
        'F5':'\x1b[15~', 'F6':'\x1b[17~', 'F7':'\x1b[18~', 'F8':'\x1b[19~',
        'F9':'\x1b[20~', 'F10':'\x1b[21~', 'F11':'\x1b[23~', 'F12':'\x1b[24~',
    };

    const currentPromptInput = () => {
        try {
            const buf = term.buffer.active;
            const line = buf.getLine(buf.baseY + buf.cursorY);
            if (!line) return null;
            const text = line.translateToString(true);
            const m = text.match(/^.*?[#$]\s*(.*)$/);
            return m ? m[1] : null;
        } catch(e) {
            return null;
        }
    };

    const replacePromptInput = (nextText) => {
        const current = currentPromptInput();
        if (current === null) return false;
        try {
            if (current.length > 0) os.key_input('\x15'); // Ctrl+U clear current input
            if (nextText) os.key_input(nextText);
            return true;
        } catch(e) {
            return false;
        }
    };

    const sendPastedText = (text) => {
        if (!text) return;
        historyNavIndex = null;
        // Normalize CRLF from clipboard into LF for shell input.
        const normalized = String(text).replace(/\r\n/g, '\n');
        // If paste contains newline, it may trigger Enter — clear currentInput after
        if (normalized.includes('\n') || normalized.includes('\r')) {
            currentInput = '';
        } else {
            currentInput += normalized;
        }
        try { os.key_input(normalized); } catch(err) { console.error('[linux] paste error:', err); }
    };

    const copySelectionToClipboard = async () => {
        const text = term && term.hasSelection && term.hasSelection() ? term.getSelection() : '';
        if (!text) return false;
        try {
            if (navigator.clipboard && navigator.clipboard.writeText) {
                await navigator.clipboard.writeText(text);
                return true;
            }
        } catch(e) {}
        return false;
    };

    const handleKey = (e) => {
        // ── Block input during agent execution (allow Ctrl+C to abort) ──
        if (agentRunning) {
            e.preventDefault();
            e.stopPropagation();
            if (e.ctrlKey && e.key === 'c') {
                agentAbort = true;
                agentSuppressOutput = false;
                term.write('^C\r\n\x1b[33m[Aborting agent...]\x1b[0m\r\n');
            }
            return;
        }

        // macOS shortcuts (Cmd+C / Cmd+V) for host clipboard integration.
        if (e.metaKey && !e.ctrlKey && !e.altKey) {
            const key = e.key.toLowerCase();
            if (key === 'c') {
                if (term.hasSelection && term.hasSelection()) {
                    e.preventDefault();
                    e.stopPropagation();
                    void copySelectionToClipboard();
                    return;
                }
                // No selection: allow normal Ctrl-C behavior below via OS key mapping.
                // We intentionally do not swallow the event here.
            } else if (key === 'v') {
                e.preventDefault();
                e.stopPropagation();
                if (navigator.clipboard && navigator.clipboard.readText) {
                    navigator.clipboard.readText().then(sendPastedText).catch(() => {});
                }
                return;
            }
        }
        if (e.metaKey) return;

        // JS-level persistent history: works immediately after boot/reload.
        if (!e.ctrlKey && !e.altKey && savedHistory.length > 0 && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
            if (e.key === 'ArrowUp') {
                if (historyNavIndex === null) historyNavIndex = savedHistory.length - 1;
                else if (historyNavIndex > 0) historyNavIndex -= 1;
                if (replacePromptInput(savedHistory[historyNavIndex] || '')) {
                    currentInput = savedHistory[historyNavIndex] || '';
                    e.preventDefault();
                    e.stopPropagation();
                    return;
                }
            } else if (historyNavIndex !== null) {
                if (historyNavIndex < savedHistory.length - 1) {
                    historyNavIndex += 1;
                    if (replacePromptInput(savedHistory[historyNavIndex] || '')) {
                        currentInput = savedHistory[historyNavIndex] || '';
                        e.preventDefault();
                        e.stopPropagation();
                        return;
                    }
                } else {
                    historyNavIndex = null;
                    if (replacePromptInput('')) {
                        currentInput = '';
                        e.preventDefault();
                        e.stopPropagation();
                        return;
                    }
                }
            }
        }

        let seq = null;
        if (e.ctrlKey && !e.altKey && e.key.length === 1) {
            const c = e.key.toUpperCase();
            if (c >= 'A' && c <= 'Z')    seq = String.fromCharCode(c.charCodeAt(0) - 64);
            else if (e.key === ' ')       seq = '\x00';
            else if (e.key === '[')       seq = '\x1b';
            else if (e.key === '\\')      seq = '\x1c';
            else if (e.key === ']')       seq = '\x1d';
        } else if (!e.ctrlKey && !e.altKey) {
            if (KEY_SEQ[e.key] !== undefined) seq = KEY_SEQ[e.key];
            else if (e.key.length === 1)      seq = e.key;
        }
        if (seq !== null) {
            // ── Track typed input for agent interception ──
            // Net stats lines corrupt xterm cursor position, so we can't rely on
            // reading the buffer. Instead, maintain currentInput from keystrokes.
            if (seq === '\r') {
                const cmd = currentInput.trim();
                let intercepted = false;

                // ── History tracking ──
                if (cmd && cmd !== savedHistory[savedHistory.length - 1]) {
                    savedHistory.push(cmd);
                    if (savedHistory.length > HIST_MAX) savedHistory = savedHistory.slice(-HIST_MAX);
                    try { localStorage.setItem(HIST_KEY, JSON.stringify(savedHistory)); } catch(e2) {}
                }

                // ── Intercept "agent" command → run browser-side JS agent ──
                // Guest-side os.exec() crashes on WASM NOMMU (CLONE_VM heap corruption).
                // Browser agent uses fetch() for LLM calls (zero forks) and shellExec()
                // for guest commands (1 fork per round, with crash detection).
                const agentTask = parseAgentTask(cmd);
                if (agentTask !== null) {
                    intercepted = true;
                    os.key_input('\x15'); // Clear current input
                    const task = agentTask;
                    if (!task) {
                        term.write('\r\nUsage: agent <task>\r\n');
                        term.write('       qjs /bin/agent.js <task>\r\n');
                        term.write('       agent.sh <task>\r\n');
                        term.write('       qjs /bin/traits-call.js <trait.path> [args-json]\r\n');
                        os.key_input('\r');
                    } else if (agentRunning) {
                        term.write('\r\n\x1b[33mAgent already running.\x1b[0m\r\n');
                        os.key_input('\r');
                    } else {
                        term.write('\r\n');
                        runJSAgent(task);
                    }
                }

                // ── Intercept "persist" commands (localStorage-backed pseudo mount) ──
                if (!intercepted && cmd.startsWith('persist')) {
                    intercepted = true;
                    os.key_input('\x15'); // Clear current input
                    term.write('\r\n');
                    runPersistCommand(cmd).then(() => {
                        os.key_input('\r');
                    }).catch((err) => {
                        term.write('\x1b[31m[persist] error: ' + (err && err.message ? err.message : String(err)) + '\x1b[0m\r\n');
                        os.key_input('\r');
                    });
                }

                // ── Intercept relay commands (serverless remote shell control) ──
                if (!intercepted && /^relay\b/i.test(cmd)) {
                    intercepted = true;
                    os.key_input('\x15'); // Clear current input
                    term.write('\r\n');
                    runShellRelayCommand(cmd).then(() => {
                        os.key_input('\r');
                    }).catch((err) => {
                        term.write('\x1b[31m[shell-relay] error: ' + (err && err.message ? err.message : String(err)) + '\x1b[0m\r\n');
                        os.key_input('\r');
                    });
                }

                // ── Intercept autofix commands (toggle failed-command monitor) ──
                if (!intercepted && /^autofix\b/i.test(cmd)) {
                    intercepted = true;
                    os.key_input('\x15'); // Clear current input
                    term.write('\r\n');
                    runAutoFixCommand(cmd).then(() => {
                        os.key_input('\r');
                    }).catch((err) => {
                        term.write('\x1b[31m[auto-fix] error: ' + (err && err.message ? err.message : String(err)) + '\x1b[0m\r\n');
                        os.key_input('\r');
                    });
                }

                // ── Intercept netmode commands (relay-only vs hybrid transport policy) ──
                if (!intercepted && /^netmode\b/i.test(cmd)) {
                    intercepted = true;
                    os.key_input('\x15'); // Clear current input
                    term.write('\r\n');
                    runNetModeCommand(cmd).then(() => {
                        os.key_input('\r');
                    }).catch((err) => {
                        term.write('\x1b[31m[netmode] error: ' + (err && err.message ? err.message : String(err)) + '\x1b[0m\r\n');
                        os.key_input('\r');
                    });
                }

                // ── Intercept qjs-style wasm trait call commands ──
                if (!intercepted) {
                    const traitCallBody = parseTraitCall(cmd);
                    if (traitCallBody !== null) {
                        intercepted = true;
                        os.key_input('\x15'); // Clear current input
                        term.write('\r\n');
                        runTraitCallCommand(cmd).then(() => {
                            os.key_input('\r');
                        }).catch((err) => {
                            term.write('\x1b[31m[traits-call] error: ' + (err && err.message ? err.message : String(err)) + '\x1b[0m\r\n');
                            os.key_input('\r');
                        });
                    }
                }

                // ── Intercept curl initramfs://... (local-only, no helper server) ──
                if (!intercepted && /^curl\b/i.test(cmd) && /initramfs:\/\//i.test(cmd)) {
                    intercepted = true;
                    os.key_input('\x15'); // Clear current input
                    term.write('\r\n');
                    runInitramfsCurlCommand(cmd).then(() => {
                        os.key_input('\r');
                    }).catch((err) => {
                        term.write('\x1b[31m[initramfs] error: ' + (err && err.message ? err.message : String(err)) + '\x1b[0m\r\n');
                        os.key_input('\r');
                    });
                }

                // Track mkdir-created directories so they persist across refresh.
                if (!intercepted && cmd.startsWith('mkdir')) {
                    persistTrackMkdir(cmd);
                }

                // Track file writes via redirect (echo/printf > /path).
                if (!intercepted) {
                    persistTrackRedirect(cmd);
                }

                // Experimental mode: if a normal shell command fails (non-zero exit),
                // auto-handoff failed command + output to runJSAgent for correction.
                if (!intercepted && cmd && autoFixOnErrorEnabled && !agentRunning && !autoFixInFlight) {
                    intercepted = true;
                    os.key_input('\x15'); // Clear current input before monitored execution
                    executeWithAutoFixMonitor(cmd).then(() => {
                        if (!agentRunning) os.key_input('\r');
                    }).catch((err) => {
                        term.write('\x1b[31m[auto-fix] monitor error: '
                            + (err && err.message ? err.message : String(err)) + '\x1b[0m\r\n');
                        if (!agentRunning) os.key_input('\r');
                    });
                }

                currentInput = '';
                historyNavIndex = null;
                if (intercepted) {
                    e.preventDefault();
                    e.stopPropagation();
                    return;
                }
            } else if (seq === '\x7f') {
                // Backspace — remove last char from tracked input
                currentInput = currentInput.slice(0, -1);
                historyNavIndex = null;
            } else if (seq === '\x15') {
                // Ctrl+U — clear line
                currentInput = '';
            } else if (seq.length === 1 && seq.charCodeAt(0) >= 32) {
                // Printable character — append to tracked input
                currentInput += seq;
                historyNavIndex = null;
            } else {
                // Any non-Enter key input exits history-navigation mode.
                historyNavIndex = null;
            }
            e.preventDefault();
            e.stopPropagation();
            try { os.key_input(seq); } catch(err) { console.error('[linux] key_input error:', err); }
        }
    };

    const handlePaste = (e) => {
        const text = e.clipboardData ? e.clipboardData.getData('text/plain') : '';
        if (!text) return;
        e.preventDefault();
        e.stopPropagation();
        sendPastedText(text);
    };

    const handleCopy = (e) => {
        if (!(term.hasSelection && term.hasSelection())) return;
        const text = term.getSelection();
        if (!text) return;
        if (e.clipboardData) {
            e.preventDefault();
            e.stopPropagation();
            e.clipboardData.setData('text/plain', text);
        }
    };

    // Capture phase fires before any bubble-phase handler in the SPA
    document.addEventListener('keydown', handleKey, true);
    document.addEventListener('paste', handlePaste, true);
    document.addEventListener('copy', handleCopy, true);

    // Clean up when SPA navigates away from this page
    window._pageCleanup = () => {
        shellRelayStop = true;
        shellRelayRunning = false;
        document.removeEventListener('keydown', handleKey, true);
        document.removeEventListener('paste', handlePaste, true);
        document.removeEventListener('copy', handleCopy, true);
        if (netStatsTimer) clearInterval(netStatsTimer);
        URL.revokeObjectURL(workerUrl);
    };

    term.focus();

    // ── API key injection to guest /tmp/key REMOVED ──
    // The old shell-based agent.sh needed the key in /tmp/key. The JS agent
    // reads directly from localStorage, so no guest injection is needed.
    // The echo + redirect commands caused CLONE_VM heap corruption (memory
    // access out of bounds) which crashed the shell subprocess.

    // History is restored via JS-level ArrowUp/ArrowDown navigation above.

    // Optional boot-time sync of persist files into guest path.
    // Enable with: persist autosync on
    shellReadyPromise.then(() => {
        if (persistAutosyncEnabled()) {
            term.write('\x1b[2m[persist] autosync enabled\x1b[0m\r\n');
            persistSyncToGuest(true).then(() => {
                term.write('\x1b[2m[persist] autosync complete\x1b[0m\r\n');
            }).catch((err) => {
                term.write('\x1b[33m[persist] autosync failed: ' + (err && err.message ? err.message : String(err)) + '\x1b[0m\r\n');
            });
        }
    });

    // Refit on resize
    window.addEventListener('resize', () => {
        try { fitAddon.fit(); } catch(e) {}
    });

    // Reboot button
    const rebootBtn = document.getElementById('btn-reboot');
    if (rebootBtn) {
        rebootBtn.addEventListener('click', () => {
            sessionStorage.removeItem(COI_SW_RELOAD_KEY);
            location.reload();
        });
    }
})();
"#;
