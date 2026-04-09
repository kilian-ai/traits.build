// SPDX-License-Identifier: GPL-2.0-only (runtime js from joelseverin/linux-wasm)
use serde_json::Value;
use maud::{html, DOCTYPE, PreEscaped};

const LINUX_JS: &str = include_str!("linux.js");
const LINUX_WORKER_JS: &str = include_str!("linux-worker.js");

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
// Adapts joelseverin/linux-wasm runtime into the traits.build SPA.
// Fetches vmlinux.wasm + initramfs.cpio.gz from the joelseverin CDN.
// ─────────────────────────────────────────────────────────────────────────────

(async function bootLinuxWasm() {
    const CDN = 'https://joelseverin.github.io/linux-wasm';
    const COI_SW_RELOAD_KEY = 'linux-wasm-coi-v1';

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
            CDN + '/vmlinux.wasm',
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
            CDN + '/initramfs.cpio.gz',
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

    const boot_cmdline = 'maxcpus=3 nohz_full=0,2-63 root=/dev/ram0 rootfstype=ramfs init=/init console=hvc console=ttyS0';

    const logLine = (text) => term.write(('\x1B[2m' + text + '\x1B[0m\n').replaceAll('\n', '\r\n'));
    const console_write = (data) => term.write(data);

    let os;
    try {
        os = await linux(workerUrl, vmlinux, boot_cmdline, initrd, logLine, console_write);
        URL.revokeObjectURL(workerUrl);
        setProgress(100);
    } catch (err) {
        term.write('\r\n\x1B[1;31m[ERROR] ' + err.message + '\x1B[0m\r\n');
        console.error('Linux/WASM boot failed:', err);
        return;
    }

    // ── Keyboard input ──
    // Capture keydown at document level (capture phase) so input works
    // regardless of which element holds browser focus in the SPA.
    // Translate keydown events → ANSI/VT100 sequences → os.key_input().
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

    const handleKey = (e) => {
        // Don't steal Meta shortcuts (Cmd on Mac)
        if (e.metaKey) return;
        let seq = null;
        if (e.ctrlKey && !e.altKey && e.key.length === 1) {
            const c = e.key.toUpperCase();
            if (c >= 'A' && c <= 'Z')       seq = String.fromCharCode(c.charCodeAt(0) - 64);
            else if (e.key === ' ')          seq = '\x00';
            else if (e.key === '[')          seq = '\x1b';
            else if (e.key === '\\')         seq = '\x1c';
            else if (e.key === ']')          seq = '\x1d';
        } else if (!e.ctrlKey && !e.altKey) {
            if (KEY_SEQ[e.key] !== undefined) seq = KEY_SEQ[e.key];
            else if (e.key.length === 1)     seq = e.key;
        }
        if (seq !== null) {
            e.preventDefault();
            e.stopPropagation();
            os.key_input(seq);
        }
    };

    // Capture phase so we intercept before SPA bubble-phase handlers
    document.addEventListener('keydown', handleKey, true);

    // Clean up when SPA navigates away from this page
    window._pageCleanup = () => document.removeEventListener('keydown', handleKey, true);

    term.focus();

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
