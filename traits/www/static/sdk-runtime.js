(function() {
/**
 * traits.js — Unified client SDK for traits.build
 *
 * Dispatch cascade: WASM kernel (instant, local) → helper (localhost) → REST API.
 * Helper = local Rust binary running on localhost for privileged traits.
 *
 * Usage:
 *   import { Traits } from '/static/www/sdk/traits.js';
 *   const traits = new Traits();         // auto-detects server from current origin
 *   await traits.init();                 // loads WASM kernel + discovers helper
 *   const hash = await traits.call('sys.checksum', ['hash', 'hello']);
 *   const list = await traits.list();
 *   const info = await traits.info('sys.checksum');
 */

// ── WASM kernel bindings (lazy-loaded) ──
let wasm = null;
let wasmReady = false;
let wasmCallableSet = new Set();

// ── Local helper state ──
let helperUrl = null;
let helperReady = false;
let helperInfo = null;
const HELPER_PORTS = [8090, 8091, 9090];
const HELPER_TIMEOUT = 1500;

// ── WebLLM engine state (lazy-loaded) ──
let _webllmLib = null;
let _webllmEngine = null;
let _webllmModel = null;
let _webllmLoading = null;

const WEBLLM_DEFAULT_MODEL = 'SmolLM2-360M-Instruct-q4f16_1-MLC';

function _webllmProgress(text) {
    if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('webllm-progress', { detail: text }));
    }
}

async function _ensureWebLLM(model) {
    const modelId = model || WEBLLM_DEFAULT_MODEL;

    // Already loaded with same model
    if (_webllmEngine && _webllmModel === modelId) return _webllmEngine;

    // Detect concurrent load for same model — wait for it
    if (_webllmLoading) {
        try { await _webllmLoading; } catch(e) {}
        if (_webllmModel === modelId && _webllmEngine) return _webllmEngine;
    }

    // Check WebGPU support
    if (!navigator.gpu) throw new Error('WebGPU not supported in this browser (requires Chrome 113+ or Edge 113+)');
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error('WebGPU adapter not available');

    _webllmLoading = (async () => {
        try {
            // Lazy-load WebLLM library
            if (!_webllmLib) {
                _webllmProgress('Loading WebLLM library…');
                _webllmLib = await import('https://esm.run/@mlc-ai/web-llm');
            }

            // Clean up existing engine
            if (_webllmEngine) {
                try { await _webllmEngine.unload(); } catch(e) {}
                _webllmEngine = null; _webllmModel = null;
            }

            _webllmProgress(`Loading model ${modelId}… (first run downloads ~200 MB)`);
            _webllmEngine = await _webllmLib.CreateMLCEngine(modelId, {
                initProgressCallback: (report) => {
                    _webllmProgress(report.text || `${Math.round((report.progress || 0) * 100)}%`);
                }
            });
            _webllmModel = modelId;
            _webllmProgress('Model ready.');
            return _webllmEngine;
        } catch(e) {
            _webllmEngine = null; _webllmModel = null;
            _webllmProgress('');
            throw e;
        }
    })();

    try { return await _webllmLoading; } finally { _webllmLoading = null; }
}

async function probeHelper(url, timeout = HELPER_TIMEOUT) {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), timeout);
    try {
        const res = await fetch(`${url}/health`, { signal: ctrl.signal });
        clearTimeout(timer);
        if (res.ok) return await res.json();
    } catch(e) { clearTimeout(timer); }
    return null;
}

async function discoverHelper() {
    // Try stored URL first
    try {
        const stored = localStorage.getItem('traits.helper.url');
        if (stored) {
            const info = await probeHelper(stored, 1000);
            if (info) { helperUrl = stored; helperInfo = info; helperReady = true; return; }
        }
    } catch(e) {}
    // Auto-discover on common ports
    for (const port of HELPER_PORTS) {
        const url = `http://localhost:${port}`;
        const info = await probeHelper(url);
        if (info) {
            helperUrl = url; helperInfo = info; helperReady = true;
            try { localStorage.setItem('traits.helper.url', url); } catch(e) {}
            return;
        }
    }
}

async function callHelper(path, args) {
    if (!helperReady) return null;
    const rest = path.replace(/\./g, '/');
    try {
        const res = await fetch(`${helperUrl}/traits/${rest}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ args }),
        });
        const data = await res.json();
        return {
            ok: res.ok,
            result: res.ok ? data.result : undefined,
            error: res.ok ? undefined : (data.error || `HTTP ${res.status}`),
            dispatch: 'helper',
        };
    } catch(e) { return null; }
}

function syncHelperToWasm() {
    if (wasm && wasm.set_helper_connected) {
        wasm.set_helper_connected(helperReady);
    }
}

async function loadWasm(wasmUrl, jsUrl) {
    try {
        const mod = await import(jsUrl);
        await mod.default(wasmUrl);
        const result = JSON.parse(mod.init());
        // Build callable set
        const callable = JSON.parse(mod.callable_traits());
        callable.forEach(p => wasmCallableSet.add(p));
        wasm = mod;
        wasmReady = true;
        return result;
    } catch (e) {
        console.warn('[traits.js] WASM unavailable, using REST only:', e.message || e);
        wasmReady = false;
        return null;
    }
}

// ── Traits Client ──

class Traits {
    /**
     * @param {Object} opts
     * @param {string} [opts.server]    - Base URL (default: current origin)
     * @param {boolean} [opts.wasm]     - Enable WASM dispatch (default: true in browser)
     * @param {string} [opts.wasmUrl]   - WASM binary URL (default: /wasm/traits_wasm_bg.wasm)
     * @param {string} [opts.jsUrl]     - WASM JS glue URL (default: /wasm/traits_wasm.js)
     * @param {boolean} [opts.helper]   - Enable helper discovery (default: true)
     * @param {string} [opts.helperUrl] - Override helper URL (skips discovery)
     */
    constructor(opts = {}) {
        this.server = (opts.server || (typeof location !== 'undefined' ? location.origin : '')).replace(/\/$/, '');
        this.useWasm = opts.wasm !== undefined ? opts.wasm : (typeof window !== 'undefined');
        this.useHelper = opts.helper !== false;
        this._helperUrlOverride = opts.helperUrl || null;
        this.wasmUrl = opts.wasmUrl || '/wasm/traits_wasm_bg.wasm';
        this.jsUrl = opts.jsUrl || '/wasm/traits_wasm.js';
        this._initPromise = null;
        this._wasmInfo = null;
    }

    /**
     * Initialize the client. Loads WASM kernel if enabled.
     * Safe to call multiple times (idempotent).
     * @returns {Promise<{wasm: boolean, traits: number, callable: number}>}
     */
    async init() {
        if (this._initPromise) return this._initPromise;
        this._initPromise = this._doInit();
        return this._initPromise;
    }

    async _doInit() {
        // Run WASM init and helper discovery in parallel
        const helperPromise = this.useHelper
            ? (this._helperUrlOverride
                ? this.connectHelper(this._helperUrlOverride)
                : discoverHelper())
            : Promise.resolve();

        if (this.useWasm && !wasmReady) {
            const wasmBase = this.server || '';
            this._wasmInfo = await loadWasm(
                wasmBase + this.wasmUrl,
                wasmBase + this.jsUrl
            );
        }

        await helperPromise;
        syncHelperToWasm();

        return {
            wasm: wasmReady,
            traits: this._wasmInfo?.traits_registered || 0,
            callable: this._wasmInfo?.wasm_callable || 0,
            version: this._wasmInfo?.version || null,
            helper: helperReady,
            helperUrl: helperUrl,
        };
    }

    /**
     * Call a trait by dot-notation path.
     * WASM-callable traits dispatch locally; others go to the server.
     *
     * @param {string} path - Trait path (e.g. 'sys.checksum')
     * @param {Array} [args=[]] - Positional arguments
     * @param {Object} [opts] - Options
     * @param {boolean} [opts.force] - 'wasm' or 'rest' to force dispatch mode
     * @param {boolean} [opts.stream] - Enable SSE streaming (REST only)
     * @returns {Promise<any>} - Parsed result
     */
    async call(path, args = [], opts = {}) {
        // Ensure initialized
        if (!this._initPromise) await this.init();

        const forceMode = opts.force;
        let wasmResult = null;

        // 1. WASM (instant, local)
        if (forceMode === 'wasm' || (forceMode !== 'rest' && forceMode !== 'helper' && wasmReady && wasmCallableSet.has(path))) {
            wasmResult = this._callWasm(path, args);
            if (wasmResult.ok) {
                // Intercept WebLLM dispatch sentinel — route to JS-side WebLLM engine
                if (wasmResult.result && wasmResult.result.dispatch === 'webllm') {
                    return this._callWebLLM(wasmResult.result.prompt, wasmResult.result.model);
                }
                return wasmResult;
            }
            if (forceMode === 'wasm') return wasmResult; // Forced WASM — don't cascade
            // WASM failed — cascade to helper/REST
        }

        // 2. Local helper (privileged traits on localhost)
        if (forceMode === 'helper' || (forceMode !== 'rest' && helperReady)) {
            const t0 = performance.now();
            const result = await callHelper(path, args);
            if (result) {
                result.ms = Math.round((performance.now() - t0) * 10) / 10;
                return result;
            }
        }

        // 3. Server REST (if server URL is configured)
        if (this.server) {
            return this._callRest(path, args, opts);
        }

        // 4. No dispatch path available
        if (wasmResult) return wasmResult;
        return { ok: false, error: `No dispatch path for '${path}'`, dispatch: 'none' };
    }

    /**
     * Check if a trait can be dispatched locally via WASM.
     * @param {string} path
     * @returns {boolean}
     */
    isCallable(path) {
        return wasmReady && wasmCallableSet.has(path);
    }

    /**
     * Check where a call will be dispatched.
     * @param {string} path
     * @returns {'wasm'|'helper'|'rest'|'none'}
     */
    dispatchMode(path) {
        if (wasmReady && wasmCallableSet.has(path)) return 'wasm';
        if (helperReady) return 'helper';
        if (this.server) return 'rest';
        return 'none';
    }

    /**
     * Connect to a specific helper URL. Overrides auto-discovery.
     * @param {string} url - e.g. 'http://localhost:8090'
     * @returns {Promise<{ok: boolean, status?: string, version?: string}>}
     */
    /**
     * Attach an externally-loaded WASM module (e.g. from base64 initSync).
     * Use when the host page has its own WASM boot sequence.
     * @param {Object} mod - The WASM module (e.g. window.TraitsWasm)
     */
    attachWasm(mod) {
        wasm = mod;
        wasmReady = true;
        wasmCallableSet.clear();
        const callable = JSON.parse(mod.callable_traits());
        callable.forEach(p => wasmCallableSet.add(p));
        syncHelperToWasm();
    }

    /**
     * Re-probe helper connection. Call periodically to detect connect/disconnect.
     * @returns {Promise<boolean>} Whether helper is currently connected
     */
    async refreshHelper() {
        if (helperReady) {
            const info = await probeHelper(helperUrl, 1000);
            if (!info) {
                helperReady = false; helperUrl = null; helperInfo = null;
                syncHelperToWasm();
            }
        } else {
            await discoverHelper();
            if (helperReady) syncHelperToWasm();
        }
        return helperReady;
    }

    async connectHelper(url) {
        const info = await probeHelper(url.replace(/\/$/, ''));
        if (info) {
            helperUrl = url.replace(/\/$/, '');
            helperInfo = info;
            helperReady = true;
            syncHelperToWasm();
            try { localStorage.setItem('traits.helper.url', helperUrl); } catch(e) {}
            return { ok: true, ...info };
        }
        return { ok: false, error: 'Helper not reachable at ' + url };
    }

    /**
     * Disconnect from helper and clear stored URL.
     */
    disconnectHelper() {
        helperReady = false;
        helperUrl = null;
        helperInfo = null;
        syncHelperToWasm();
        try { localStorage.removeItem('traits.helper.url'); } catch(e) {}
    }

    /**
     * List all traits. Uses WASM registry → helper → REST.
     * @returns {Promise<Array>}
     */
    async list() {
        if (wasmReady) return JSON.parse(wasm.list_traits());
        if (helperReady) {
            try { const r = await fetch(`${helperUrl}/traits`); if (r.ok) return r.json(); } catch(e) {}
        }
        const res = await fetch(`${this.server}/traits`);
        return res.json();
    }

    /**
     * Get detailed info for a specific trait.
     * @param {string} path
     * @returns {Promise<Object|null>}
     */
    async info(path) {
        if (wasmReady) {
            const raw = wasm.get_trait_info(path);
            return raw ? JSON.parse(raw) : null;
        }
        const rest = path.replace(/\./g, '/');
        if (helperReady) {
            try { const r = await fetch(`${helperUrl}/traits/${rest}`); if (r.ok) return r.json(); } catch(e) {}
        }
        const res = await fetch(`${this.server}/traits/${rest}`);
        if (!res.ok) return null;
        return res.json();
    }

    /**
     * Search traits by query string.
     * @param {string} query
     * @returns {Promise<Array>}
     */
    async search(query) {
        if (wasmReady) {
            return JSON.parse(wasm.search_traits(query));
        }
        // REST fallback — list + client-side filter
        const all = await this.list();
        const q = query.toLowerCase();
        return all.filter(t =>
            t.path?.toLowerCase().includes(q) ||
            t.description?.toLowerCase().includes(q)
        );
    }

    /**
     * Get list of WASM-callable trait paths.
     * @returns {string[]}
     */
    get callableTraits() {
        return [...wasmCallableSet];
    }

    /**
     * Get kernel status.
     * @returns {{wasm: boolean, traits: number, callable: number, version: string|null, helper: boolean, helperUrl: string|null}}
     */
    get status() {
        return {
            wasm: wasmReady,
            traits: this._wasmInfo?.traits_registered || 0,
            callable: this._wasmInfo?.wasm_callable || 0,
            version: this._wasmInfo?.version || null,
            helper: helperReady,
            helperUrl: helperUrl,
        };
    }

    /** @returns {boolean} */
    get helperConnected() { return helperReady; }
    /** @returns {Object|null} */
    get helperStatus() { return helperReady ? { url: helperUrl, ...helperInfo } : null; }

    // ── Page Rendering ──

    /**
     * Call a trait and render its HTML result into a DOM element.
     * @param {string} path - Trait path (e.g. 'www.traits.build')
     * @param {Array} [args=[]] - Arguments
     * @param {string|HTMLElement} [target='body'] - CSS selector or element
     * @returns {Promise<{ok: boolean, dispatch: string}>}
     */
    async render(path, args = [], target = 'body') {
        const el = typeof target === 'string' ? document.querySelector(target) : target;
        if (!el) return { ok: false, error: `Target not found: ${target}` };

        const res = await this.call(path, args);
        if (res.ok) {
            const html = typeof res.result === 'string'
                ? res.result
                : JSON.stringify(res.result, null, 2);
            el.innerHTML = html;
            this._runScripts(el);
        }
        return res;
    }

    /**
     * Navigate to a URL path (SPA-style). Fetches page HTML from the server
     * and injects it into the target element. Updates browser history.
     * @param {string} urlPath - URL path (e.g. '/wasm', '/admin')
     * @param {string|HTMLElement} [target='body'] - CSS selector or element
     * @param {Object} [opts]
     * @param {boolean} [opts.pushState=true] - Update browser URL
     * @returns {Promise<{ok: boolean, path: string}>}
     */
    async navigate(urlPath, target = 'body', opts = {}) {
        const el = typeof target === 'string' ? document.querySelector(target) : target;
        if (!el) return { ok: false, error: `Target not found: ${target}` };

        try {
            const res = await fetch(`${this.server}${urlPath}`);
            if (!res.ok) return { ok: false, error: `HTTP ${res.status}` };
            const html = await res.text();
            el.innerHTML = html;
            this._runScripts(el);
            if (opts.pushState !== false && typeof history !== 'undefined') {
                history.pushState({ path: urlPath }, '', urlPath);
            }
            return { ok: true, path: urlPath };
        } catch (e) {
            return { ok: false, error: e.message || String(e) };
        }
    }

    /**
     * Enable SPA-style link interception. Internal link clicks use
     * navigate() instead of full page loads.
     * @param {string|HTMLElement} [scope='body'] - Scope for link interception
     * @param {string|HTMLElement} [target='body'] - Render target
     */
    intercept(scope = 'body', target = 'body') {
        const el = typeof scope === 'string' ? document.querySelector(scope) : scope;
        if (!el) return;

        el.addEventListener('click', (e) => {
            const a = e.target.closest('a[href]');
            if (!a) return;
            const href = a.getAttribute('href');
            // Skip external links, anchors, and special protocols
            if (!href || href.startsWith('http') || href.startsWith('#') ||
                href.startsWith('mailto:') || href.startsWith('javascript:') ||
                a.hasAttribute('download') || a.target === '_blank') return;
            e.preventDefault();
            this.navigate(href, target);
        });

        // Handle browser back/forward
        window.addEventListener('popstate', (e) => {
            if (e.state?.path) {
                this.navigate(e.state.path, target, { pushState: false });
            }
        });
    }

    // ── Internal ──

    /**
     * Execute <script> tags that were injected via innerHTML.
     * innerHTML doesn't run scripts, so we re-create them.
     */
    _runScripts(container) {
        for (const old of container.querySelectorAll('script')) {
            const s = document.createElement('script');
            for (const attr of old.attributes) s.setAttribute(attr.name, attr.value);
            s.textContent = old.textContent;
            old.replaceWith(s);
        }
    }

    _callWasm(path, args) {
        const t0 = performance.now();
        try {
            const raw = wasm.call(path, JSON.stringify(args));
            const dt = performance.now() - t0;
            const result = JSON.parse(raw);
            return { ok: true, result, dispatch: 'wasm', ms: Math.round(dt * 10) / 10 };
        } catch (e) {
            return { ok: false, error: e.message || String(e), dispatch: 'wasm' };
        }
    }

    async _callWebLLM(prompt, model) {
        const t0 = performance.now();
        try {
            const engine = await _ensureWebLLM(model);
            const reply = await engine.chat.completions.create({
                messages: [{ role: 'user', content: prompt }],
                temperature: 0.7,
                max_tokens: 1024,
            });
            const dt = performance.now() - t0;
            const content = reply.choices?.[0]?.message?.content || '';
            return {
                ok: true,
                result: content,
                dispatch: 'webllm',
                model: _webllmModel,
                ms: Math.round(dt * 10) / 10,
            };
        } catch (e) {
            return { ok: false, error: e.message || String(e), dispatch: 'webllm' };
        }
    }

    async _callRest(path, args, opts = {}) {
        const rest = path.replace(/\./g, '/');
        const url = `${this.server}/traits/${rest}` + (opts.stream ? '?stream=1' : '');
        const t0 = performance.now();

        try {
            const res = await fetch(url, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ args }),
            });

            const dt = performance.now() - t0;

            if (opts.stream && res.headers.get('content-type')?.includes('text/event-stream')) {
                return { ok: true, stream: this._readSSE(res.body), dispatch: 'rest', ms: Math.round(dt * 10) / 10 };
            }

            const data = await res.json();
            return {
                ok: res.ok,
                result: res.ok ? data.result : undefined,
                error: res.ok ? undefined : (data.error || `HTTP ${res.status}`),
                dispatch: 'rest',
                ms: Math.round(dt * 10) / 10,
            };
        } catch (e) {
            return { ok: false, error: e.message || String(e), dispatch: 'rest' };
        }
    }

    async *_readSSE(body) {
        const reader = body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';

        try {
            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                buffer += decoder.decode(value, { stream: true });

                const lines = buffer.split('\n');
                buffer = lines.pop() || '';

                for (const line of lines) {
                    if (line.startsWith('data: ')) {
                        const data = line.slice(6);
                        if (data === '[DONE]') return;
                        try { yield JSON.parse(data); } catch { yield data; }
                    }
                }
            }
        } finally {
            reader.releaseLock();
        }
    }
}

// ── Default singleton ──
let _default = null;

/**
 * Get or create the default Traits client instance.
 * @param {Object} [opts] - Options passed to constructor (first call only)
 * @returns {Traits}
 */
function getTraits(opts) {
    if (!_default) _default = new Traits(opts);
    return _default;
}

// Convenience re-exports for quick use
if (typeof window !== "undefined") { window.Traits = Traits; window.getTraits = getTraits; }
})();
