/**
 * traits.js — Unified client SDK for traits.build
 *
 * Routes calls through WASM kernel (instant, local) when available,
 * falls back to REST API for server-only traits.
 *
 * Usage:
 *   import { Traits } from '/static/www/sdk/traits.js';
 *   const traits = new Traits();         // auto-detects server from current origin
 *   await traits.init();                 // loads WASM kernel
 *   const hash = await traits.call('sys.checksum', ['hash', 'hello']);
 *   const list = await traits.list();
 *   const info = await traits.info('sys.checksum');
 */

// ── WASM kernel bindings (lazy-loaded) ──
let wasm = null;
let wasmReady = false;
let wasmCallableSet = new Set();

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

export class Traits {
    /**
     * @param {Object} opts
     * @param {string} [opts.server]    - Base URL (default: current origin)
     * @param {boolean} [opts.wasm]     - Enable WASM dispatch (default: true in browser)
     * @param {string} [opts.wasmUrl]   - WASM binary URL (default: /wasm/traits_wasm_bg.wasm)
     * @param {string} [opts.jsUrl]     - WASM JS glue URL (default: /wasm/traits_wasm.js)
     */
    constructor(opts = {}) {
        this.server = (opts.server || (typeof location !== 'undefined' ? location.origin : '')).replace(/\/$/, '');
        this.useWasm = opts.wasm !== undefined ? opts.wasm : (typeof window !== 'undefined');
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
        if (this.useWasm && !wasmReady) {
            const wasmBase = this.server || '';
            this._wasmInfo = await loadWasm(
                wasmBase + this.wasmUrl,
                wasmBase + this.jsUrl
            );
        }
        return {
            wasm: wasmReady,
            traits: this._wasmInfo?.traits_registered || 0,
            callable: this._wasmInfo?.wasm_callable || 0,
            version: this._wasmInfo?.version || null,
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
        const useLocal = forceMode === 'wasm' ||
            (forceMode !== 'rest' && wasmReady && wasmCallableSet.has(path));

        if (useLocal) {
            return this._callWasm(path, args);
        }
        return this._callRest(path, args, opts);
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
     * @returns {'wasm'|'rest'|'unknown'}
     */
    dispatchMode(path) {
        if (wasmReady && wasmCallableSet.has(path)) return 'wasm';
        if (this.server) return 'rest';
        return 'unknown';
    }

    /**
     * List all traits. Uses WASM registry if available, REST otherwise.
     * @returns {Promise<Array>}
     */
    async list() {
        if (wasmReady) {
            return JSON.parse(wasm.list_traits());
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
     * @returns {{wasm: boolean, traits: number, callable: number, version: string|null}}
     */
    get status() {
        return {
            wasm: wasmReady,
            traits: this._wasmInfo?.traits_registered || 0,
            callable: this._wasmInfo?.wasm_callable || 0,
            version: this._wasmInfo?.version || null,
        };
    }

    // ── Internal ──

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
export function getTraits(opts) {
    if (!_default) _default = new Traits(opts);
    return _default;
}

// Convenience re-exports for quick use
export default Traits;
