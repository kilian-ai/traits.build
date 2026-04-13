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
    const ASSET_REV = '4b95d8a';
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

    const resolveTunnelUrl = () => {
        try {
            const params = new URLSearchParams(location.search);
            const fromQuery = params.get('linux_tunnel');
            if (fromQuery) return fromQuery;
        } catch (e) {}

        try {
            const fromStorage = localStorage.getItem('linux-wasm.tunnel-url') || '';
            if (fromStorage) return fromStorage;
        } catch (e) {}

        // Default: use the global relay tunnel endpoint.
        return 'wss://relay.traits.build/linux/tunnel';
    };

    const tunnelUrl = resolveTunnelUrl();
    if (typeof NetProxy !== 'undefined' && NetProxy.setTunnelURL && tunnelUrl) {
        NetProxy.setTunnelURL(tunnelUrl);
    }
    
    let bootNetMode = 'unknown';

    // Wait for tunnel readiness (5s timeout) and report status with error details if any
    if (typeof NetProxy !== 'undefined' && NetProxy.waitForTunnelReady) {
        console.log('[linux.rs] waiting for tunnel readiness (5 second timeout)...');
        const wasConnected = await NetProxy.waitForTunnelReady(5000);
        console.log('[linux.rs] tunnel ready result:', wasConnected);
        const mode = (typeof NetProxy.getMode) ? NetProxy.getMode() : 'unknown';
        bootNetMode = mode;
        const suffix = tunnelUrl ? ` (${tunnelUrl})` : '';
        term.write(`\x1B[2m[traits.build] NET mode: ${mode}${suffix}\x1B[0m\r\n`);
        if (mode === 'browser-fallback') {
            const errMsg = (typeof NetProxy.getTunnelError) ? NetProxy.getTunnelError() : null;
            console.log('[linux.rs] tunnel error message:', errMsg);
            const errDetail = errMsg ? ` — ${errMsg}` : '';
            term.write(`\x1B[33m[traits.build] NET degraded: browser emulation fallback (tunnel unavailable${errDetail})\x1B[0m\r\n`);
        } else if (mode === 'tunnel') {
            term.write(`\x1B[32m[traits.build] NET ready: tunnel connected to relay server\x1B[0m\r\n`);
        }
    }

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

    // WASM kernel model: each user task (non-kthread) needs its own dedicated CPU.
    // The kernel's user_task_set_affinity() in arch/wasm/kernel/process.c pins each
    // forked user process to a unique CPU. CPU 1 is reserved as IRQ_CPU.
    // With maxcpus=N, we get (N-1) usable user CPUs (minus IRQ_CPU).
    // Too few CPUs → clone() returns -EBUSY ("Resource busy") when shell forks.
    // CPUs are recycled when tasks exit (release_thread clears user_cpus bitmask).
    // maxcpus=5 gives 4 user CPUs (0,2,3,4): enough for init + shell + commands.
    // Higher values (e.g. 10) create 70+ Web Workers which can stall browser boot.
    const boot_cmdline = `maxcpus=5 root=/dev/ram0 rootfstype=ramfs rdinit=${initProgram} console=hvc console=ttyS0`;
    if (initProgram !== '/init') {
        term.write(`\x1B[2m[traits.build] INIT mode: minimal (${initProgram})\x1B[0m\r\n`);
    }

    const logLine = (text) => term.write(('\x1B[2m' + text + '\x1B[0m\n').replaceAll('\n', '\r\n'));
    // Track whether the first shell prompt has appeared (for secret injection)
    let shellReady = false;
    let shellReadyResolve = null;
    const shellReadyPromise = new Promise(r => { shellReadyResolve = r; });
    let consoleBuffer = '';

    const console_write = (data) => {
        term.write(data);
        // Detect first interactive shell prompt (BusyBox prints hash-space or dollar-space)
        if (!shellReady) {
            consoleBuffer += data;
            // Look for prompt at end of output — shell is ready when we see "prompt "
            const tail = consoleBuffer.slice(-4);
            if (tail.endsWith('$ ') || tail.endsWith('> ') || (tail.includes('#') && tail.endsWith(' '))) {
                shellReady = true;
                shellReadyResolve();
            }
            // Don't let buffer grow unbounded during boot
            if (consoleBuffer.length > 4096) consoleBuffer = consoleBuffer.slice(-512);
        }
    };

    let os;
    let netStatsTimer = null;
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
                if (!proxy && !host) return;

                const mode = (typeof NetProxy !== 'undefined' && NetProxy.getMode) ? NetProxy.getMode() : 'unknown';
                if (mode !== lastMode) {
                    if (lastMode === 'browser-fallback' && mode === 'tunnel') {
                        term.write('\x1B[32m[traits.build] NET upgraded: tunnel connected after boot\x1B[0m\r\n');
                    } else {
                        term.write(`\x1B[2m[traits.build] NET mode changed: ${lastMode} -> ${mode}\x1B[0m\r\n`);
                    }
                    lastMode = mode;
                }
                const q = proxy ? proxy.queueLen : 0;
                const qh = proxy ? proxy.queueHighWater : 0;
                const drop = proxy ? proxy.rxDroppedPackets : 0;
                const cbRecv = host ? host.recvAvgMs : 0;
                const cbPoll = host ? host.pollAvgMs : 0;
                const line = `[net] mode=${mode} q=${q}/${qh} drop=${drop} cb_recv=${cbRecv.toFixed(3)}ms cb_poll=${cbPoll.toFixed(3)}ms`;
                if (line !== lastNetLine) {
                    term.write(`\x1B[2m${line}\x1B[0m\r\n`);
                    lastNetLine = line;
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
                    e.preventDefault();
                    e.stopPropagation();
                    return;
                }
            } else if (historyNavIndex !== null) {
                if (historyNavIndex < savedHistory.length - 1) {
                    historyNavIndex += 1;
                    if (replacePromptInput(savedHistory[historyNavIndex] || '')) {
                        e.preventDefault();
                        e.stopPropagation();
                        return;
                    }
                } else {
                    historyNavIndex = null;
                    if (replacePromptInput('')) {
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
            // ── Track shell history on Enter ──
            if (seq === '\r') {
                try {
                    const buf = term.buffer.active;
                    const line = buf.getLine(buf.baseY + buf.cursorY);
                    if (line) {
                        const text = line.translateToString(true).trim();
                        const m = text.match(/^.*?[#$]\s+(.+)$/);
                        if (m && m[1].trim()) {
                            const cmd = m[1].trim();
                            if (cmd !== savedHistory[savedHistory.length - 1]) {
                                savedHistory.push(cmd);
                                if (savedHistory.length > HIST_MAX) savedHistory = savedHistory.slice(-HIST_MAX);
                                try { localStorage.setItem(HIST_KEY, JSON.stringify(savedHistory)); } catch(e2) {}
                            }
                        }
                    }
                } catch(e2) {}
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
        document.removeEventListener('keydown', handleKey, true);
        document.removeEventListener('paste', handlePaste, true);
        document.removeEventListener('copy', handleCopy, true);
        if (netStatsTimer) clearInterval(netStatsTimer);
        URL.revokeObjectURL(workerUrl);
    };

    term.focus();

    // ── Inject OpenAI API key from Settings secrets into guest /tmp/key ──
    // The Settings page stores secrets as localStorage['traits.secret.KEYNAME'].
    // If the user has stored OPENAI_API_KEY, write it to the guest filesystem
    // so agent.sh can use it without manual key entry.
    (async () => {
        const apiKey = localStorage.getItem('traits.secret.OPENAI_API_KEY');
        if (!apiKey) return;
        // Wait for the shell prompt to appear before injecting
        await shellReadyPromise;
        // Small delay to let the shell fully initialize
        await new Promise(r => setTimeout(r, 500));
        // Base64-encode the key to avoid shell escaping issues, then decode in guest.
        // Disable echo so the command (and key) don't appear in terminal output.
        const b64 = btoa(apiKey);
        os.key_input("stty -echo; echo " + b64 + " | base64 -d > /tmp/key; stty echo; clear\r");
    })();

    // History is restored via JS-level ArrowUp/ArrowDown navigation above.

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
