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

    const PERSIST_KEY = 'linux-wasm.persist.files';
    const PERSIST_PVFS_KEY = 'traits.pvfs';
    const PERSIST_PVFS_PREFIX = 'linux-wasm.persist';
    const PERSIST_DIRS_META = PERSIST_PVFS_PREFIX + '/.dirs.json';
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
        return PERSIST_PVFS_PREFIX + p;
    }

    function persistFromPvfsPath(path) {
        const text = String(path || '');
        const prefix = PERSIST_PVFS_PREFIX + '/';
        if (!text.startsWith(prefix)) return '';
        return persistNormalizePath(text.slice(PERSIST_PVFS_PREFIX.length));
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
                if (k === PERSIST_DIRS_META) continue;
                const guestPath = persistFromPvfsPath(k);
                if (!persistIsValidPath(guestPath) || !persistPathMounted(guestPath)) continue;
                files[guestPath] = String(pvfs[k] || '');
            }
            let dirs = [];
            try {
                const parsedDirs = JSON.parse(String(pvfs[PERSIST_DIRS_META] || '[]'));
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
            if (k === PERSIST_DIRS_META || k.startsWith(PERSIST_PVFS_PREFIX + '/')) delete pvfs[k];
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
        if (persistPullRunning || agentRunning) return;
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

    let consoleFilterCarry = '';
    const KERNEL_NOISE_RE = /^\[(Main|Runner)[^\]]*\]:/;

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
                        console.log('[net] upgraded: browser-fallback -> tunnel');
                    } else {
                        console.log(`[net] mode changed: ${lastMode} -> ${mode}`);
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
                    console.log(line);
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
            agentCapture = (data) => {
                if (resolved) return;
                // Ensure data is a string (console_write may receive Uint8Array)
                const text = typeof data === 'string' ? data : new TextDecoder().decode(data);
                buffer += text;
                // Early crash detection: abort immediately on kernel panic
                if (KERNEL_CRASH_RE.test(buffer)) {
                    resolved = true;
                    agentCapture = null;
                    agentSuppressOutput = false;
                    console.error('[agent] Kernel crash detected during:', cmd);
                    resolve({ output: '[kernel crashed]', exitCode: -2, crashed: true });
                    return;
                }
                const m = buffer.match(/__EC:(\d+)__/);
                if (m) {
                    resolved = true;
                    agentCapture = null;
                    agentSuppressOutput = false;
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
                    console.log('[agent] shellExec resolved:', { cmd, exitCode, outputLen: output.length, output: output.slice(0, 200) });
                    resolve({ output, exitCode });
                }
            };
            agentSuppressOutput = true;
            // No 2>&1! Avoids CLONE_VM restore_redirects crash.
            os.key_input(cmd + '; echo __EC:$?__\r');
            setTimeout(() => {
                if (!resolved) {
                    resolved = true;
                    console.warn('[agent] shellExec TIMEOUT for:', cmd, 'buffer:', buffer.slice(0, 500));
                    agentCapture = null;
                    agentSuppressOutput = false;
                    try { os.key_input('\x03'); } catch (e) {}
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

    async function runJSAgent(task) {
        agentRunning = true;
        agentAbort = false;
        const apiKey = await resolveApiKey();
        if (!apiKey) {
            term.write('\x1b[31mNo API key. Set OPENAI_API_KEY in Settings or write to /tmp/key.\x1b[0m\r\n');
            agentRunning = false;
            os.key_input('\r');
            return;
        }

        const SYS = 'You are a shell agent inside BusyBox Linux/WASM (musl, hush shell, NOMMU). ' +
            'CRITICAL: fork() CRASHES this kernel. You MUST use ONLY shell builtins. ' +
            'External commands (ls, cat, grep, find, etc.) fork and will crash. ' +
            'Rules: 1) Reply with EXACTLY one shell command (single line), no markdown, no explanation. ' +
            '2) When the task is done, reply DONE: summary. ' +
            '3) ALLOWED builtins: echo, printf, cd, pwd, read, test, [, for, while, if, case, set, unset, export, true, false. ' +
            '4) List directory: echo /path/* (glob expansion is a builtin). ' +
            '5) Read file safely: test -f /path && { IFS= read -r l < /path && echo "$l" || echo ""; } || echo missing ' +
            '6) Check file: test -f /path && echo exists || echo missing ' +
            '7) Do NOT read /proc paths unless explicitly requested. ' +
            '8) Write file: Use echo "content" with output redirect: echo "text" > /path/file ' +
            '9) Create dir: cannot mkdir (forks). Use available dirs only. ' +
            '10) If a requested file is missing, reply DONE: missing and stop. Do NOT probe alternative files. ' +
            '11) Avoid /proc/self/*, /proc/mounts, and status-like proc files; they may hang in this kernel. ' +
            '12) NEVER use: ls, cat, grep, find, head, tail, awk, sed, wc, sort, mkdir, rm, cp, mv, date, uname, curl, wget, du. ' +
            '13) NEVER use $(...) or backticks — command substitution forks a subshell.';

        let history = '';
        const MAX = 10;

        term.write('\x1b[1;32m=== Agent: ' + task + ' ===\x1b[0m\r\n');
        console.log('[agent] Starting task:', task);

        for (let round = 1; round <= MAX; round++) {
            if (agentAbort) {
                term.write('\x1b[33m[Agent aborted by user]\x1b[0m\r\n');
                break;
            }

            term.write('\x1b[2m-- Round ' + round + '/' + MAX + ' --\x1b[0m\r\n');

            const userMsg = round === 1
                ? 'Task: ' + task
                : 'Task: ' + task + '\nHistory:\n' + history + '\nNext command or DONE: summary.';

            console.log('[agent] Round', round, 'userMsg:', userMsg.slice(0, 300));
            term.write('  \x1b[2m[calling LLM...]\x1b[0m\r\n');

            let cmd;
            try {
                const resp = await fetch('https://relay.traits.build/llm/proxy', {
                    method: 'POST',
                    headers: {
                        'Authorization': 'Bearer ' + apiKey,
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({
                        model: 'gpt-4o-mini',
                        messages: [
                            { role: 'system', content: SYS },
                            { role: 'user', content: userMsg }
                        ],
                        max_tokens: 200,
                        temperature: 0
                    })
                });
                const data = await resp.json();
                if (data.error) {
                    const msg = typeof data.error === 'string' ? data.error : (data.error.message || JSON.stringify(data.error));
                    term.write('  \x1b[31mAPI error: ' + msg + '\x1b[0m\r\n');
                    continue;
                }
                cmd = data.choices[0].message.content.trim();
                console.log('[agent] Round', round, 'LLM response:', cmd);
            } catch (err) {
                term.write('  \x1b[31mFetch error: ' + err.message + '\x1b[0m\r\n');
                continue;
            }

            // Check for DONE
            if (/^done/i.test(cmd)) {
                term.write('\r\n\x1b[1;32m=== ' + cmd + ' ===\x1b[0m\r\n');
                term.write('Completed in ' + round + ' round(s).\r\n');
                break;
            }

            // Strip markdown fences
            cmd = cmd.replace(/^```(?:sh|bash)?\n?/, '').replace(/\n?```$/, '');
            // Take first line only
            cmd = cmd.split('\n')[0].trim();

            // Rewrite known hanging pattern produced by LLM into a safer one-liner.
            const whileReadPath = cmd.match(/while\s+IFS=\s*read\s+-r\s+\w+[\s\S]*<\s*(\/\S+)\s*$/);
            if (whileReadPath) {
                const p = whileReadPath[1];
                if (p.startsWith('/proc/')) {
                    cmd = "echo unsupported_proc_path";
                } else {
                    cmd = "test -f '" + shellSingleQuote(p) + "' && { IFS= read -r l < '" + shellSingleQuote(p) + "' && echo \"$l\" || echo \"\"; } || echo missing";
                }
                console.log('[agent] Rewrote risky while-read command to:', cmd);
            }

            if (!cmd) {
                term.write('  \x1b[2m[empty response]\x1b[0m\r\n');
                continue;
            }

            term.write('  \x1b[33m$ ' + cmd + '\x1b[0m\r\n');

            // Execute in guest shell (this is the ONLY fork per round)
            console.log('[agent] Executing:', cmd);
            const { output, exitCode, crashed } = await shellExec(cmd);
            agentSuppressOutput = false;
            console.log('[agent] Exec result:', { exitCode, outputLen: output.length, crashed, output: output.slice(0, 300) });

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
                const show = Math.min(lines.length, 20);
                for (let i = 0; i < show; i++) {
                    term.write('  \x1b[37m| ' + lines[i] + '\x1b[0m\r\n');
                }
                if (lines.length > 20) {
                    term.write('  \x1b[2m| ...(' + lines.length + ' lines total)\x1b[0m\r\n');
                }
            }
            term.write('  \x1b[2m[exit: ' + exitCode + ']\x1b[0m\r\n\r\n');

            // Build history for next round (keep short)
            const truncOut = output.length > 500 ? output.slice(0, 500) + '...' : output;
            history += 'Cmd: ' + cmd + '\nExit: ' + exitCode + '\nOut: ' + truncOut + '\n';
        }

        agentRunning = false;
        agentSuppressOutput = false;
        agentCapture = null;
        // Pull modified files from guest FS into persist store
        if (persistAutosyncEnabled()) {
            await persistPullFromGuest(true);
        }
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

                // Track mkdir-created directories so they persist across refresh.
                if (!intercepted && cmd.startsWith('mkdir')) {
                    persistTrackMkdir(cmd);
                }

                // Track file writes via redirect (echo/printf > /path).
                if (!intercepted) {
                    persistTrackRedirect(cmd);
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
