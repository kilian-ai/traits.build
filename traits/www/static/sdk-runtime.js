(function() {
/**
 * traits.js — Unified client SDK for traits.build
 *
 * Dispatch cascade: WASM kernel (instant, local) → helper (localhost) → REST API.
 * Helper = local Rust binary running on localhost for privileged traits.
 * Runtime bindings: interface paths (e.g. llm/prompt) resolve to bound implementations
 * before dispatch. Supports deferred binding (bindWhenReady) for lazy-loaded impls.
 *
 * Usage:
 *   import { Traits } from '/static/www/sdk/traits.js';
 *   const traits = new Traits();         // auto-detects server from current origin
 *   await traits.init();                 // loads WASM kernel + discovers helper
 *   const hash = await traits.call('sys.checksum', ['hash', 'hello']);
 *   traits.bind('llm/prompt', 'llm.prompt.openai');       // set default
 *   traits.bindWhenReady('llm/prompt', 'llm.prompt.webllm', readyPromise);
 */

// ── WASM kernel bindings (lazy-loaded) ──
let wasm = null;
let wasmReady = false;
let wasmCallableSet = new Set();

const BACKGROUND_IFACE = 'kernel/background';
const BACKGROUND_WORKER = 'sdk.background.worker';
const BACKGROUND_DIRECT = 'sdk.background.direct';
const BACKGROUND_TOKIO = 'sdk.background.tokio';

function resolveWorkerScriptUrl(explicitUrl) {
    if (explicitUrl) return explicitUrl;
    if (typeof document !== 'undefined') {
        const inline = document.querySelector('script[data-runtime-src="inline:traits-worker"]');
        if (inline && inline.textContent) {
            const blob = new Blob([inline.textContent], { type: 'text/javascript' });
            return URL.createObjectURL(blob);
        }
    }
    if (typeof location !== 'undefined' && location.protocol === 'file:') {
        return `./traits-worker.js?v=${Date.now()}`;
    }
    return '/static/www/static/traits-worker.js';
}

// ── Local helper state ──
let helperUrl = null;
let helperReady = false;
let helperInfo = null;
const HELPER_PORTS = [8090, 8091, 9090];
const HELPER_TIMEOUT = 1500;

// ── Relay state (remote helper via pairing code) ──
const RELAY_DEFAULT_SERVER = 'https://relay.traits.build';
const RELAY_ENABLED_KEY = 'traits.relay.enabled';

function _relayServer() {
    try {
        let server = localStorage.getItem('traits.relay.server') || RELAY_DEFAULT_SERVER;
        // Migrate stale domains from before the CF Workers migration
        if (server.includes('fly.dev') || server.includes('kiliannc.workers.dev')) {
            server = RELAY_DEFAULT_SERVER;
            localStorage.setItem('traits.relay.server', server);
            localStorage.removeItem('traits.relay.token'); // token is server-scoped
        }
        return server;
    } catch(e) { return RELAY_DEFAULT_SERVER; }
}
function _rememberedRelayCode() {
    try { return localStorage.getItem('traits.relay.code'); } catch(e) { return null; }
}
function _relayEnabled() {
    try { return localStorage.getItem(RELAY_ENABLED_KEY) !== '0'; } catch(e) { return true; }
}
function _relayCode() {
    const code = _rememberedRelayCode();
    return code && _relayEnabled() ? code : null;
}
function _relayToken() {
    try { return localStorage.getItem('traits.relay.token'); } catch(e) { return null; }
}
// Decode the code embedded in a token without verifying signature (client-side read-only).
function _relayTokenCode() {
    try {
        const token = _relayToken();
        if (!token) return null;
        const payload = JSON.parse(atob(token.slice(0, token.lastIndexOf('.'))));
        return payload.code || null;
    } catch(_) { return null; }
}
function _relayTokenExpired() {
    try {
        const token = _relayToken();
        if (!token) return true;
        const payload = JSON.parse(atob(token.slice(0, token.lastIndexOf('.'))));
        return !payload.exp || Date.now() / 1000 > payload.exp;
    } catch(_) { return true; }
}

async function callRelay(path, args) {
    const token = !_relayTokenExpired() ? _relayToken() : null;
    const code  = _relayCode();
    if (!token && !code) return null;
    const server = _relayServer();
    try {
        const body = token ? { token, path, args } : { code, path, args };
        const res = await fetch(`${server}/relay/call`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body),
        });
        if (!res.ok && res.status === 401) {
            // Token rejected — clear it and fall back to code next time
            try { localStorage.removeItem('traits.relay.token'); } catch(_) {}
            return null;
        }
        if (!res.ok && res.status === 404) return null;
        const data = await res.json();
        if (data.error) return { ok: false, error: data.error, dispatch: 'relay' };
        return { ok: true, result: data.result, dispatch: 'relay' };
    } catch(e) { return null; }
}

// ── WebLLM engine state (lazy-loaded) ──
let _webllmLib = null;
let _webllmEngine = null;
let _webllmModel = null;
let _webllmLoading = null;
let _lastWebLLMStep = '';
let _webllmProgressTime = 0;

const WEBLLM_DEFAULT_MODEL = 'Llama-3.2-3B-Instruct-q4f16_1-MLC';

// ── Voice (browser microphone) state ──
let _voiceStream = null;
let _voicePc = null;             // RTCPeerConnection for WebRTC voice
let _voiceDc = null;             // DataChannel for sending/receiving events
let _voiceAudioEl = null;        // <audio> element for model playback
let _voiceApiKey = null;
let _voiceSdk = null;            // Reference to Traits instance for tool dispatch

// Traits excluded from voice function-calling tools (mirrors native TOOL_EXCLUDE)
const VOICE_TOOL_EXCLUDE = new Set([
    'sys.voice', 'sys.mcp', 'sys.serve', 'sys.cli', 'sys.cli.native', 'sys.cli.wasm',
    'sys.dylib_loader', 'sys.reload', 'sys.release', 'sys.secrets',
    'kernel.main', 'kernel.dispatcher', 'kernel.globals', 'kernel.registry',
    'kernel.config', 'kernel.plugin_api', 'kernel.cli',
    'www.admin', 'www.admin.deploy', 'www.admin.fast_deploy',
    'www.admin.scale', 'www.admin.destroy', 'www.admin.save_config',
]);

function _dispatchVoiceEvent(type, data) {
    if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('voice-event', { detail: { type, ...data } }));
    }
}

/** Convert a trait param type string to JSON Schema */
function _traitTypeToSchema(typeStr) {
    if (!typeStr) return { type: 'string' };
    const t = typeStr.toLowerCase().replace(/\s/g, '');
    if (t === 'int' || t === 'integer') return { type: 'integer' };
    if (t === 'float' || t === 'number') return { type: 'number' };
    if (t === 'bool' || t === 'boolean') return { type: 'boolean' };
    if (t === 'string') return { type: 'string' };
    if (t.startsWith('list<') || t.startsWith('array<')) {
        const inner = t.slice(t.indexOf('<') + 1, t.lastIndexOf('>'));
        return { type: 'array', items: _traitTypeToSchema(inner) };
    }
    if (t.startsWith('map<')) return { type: 'object' };
    return { type: 'string' };
}

/** Build OpenAI Realtime API tool definitions from the trait registry.
 *  In WASM-only mode (no helper/server), only WASM-callable traits are included
 *  since non-WASM traits cannot be dispatched in the browser. */
async function _buildVoiceTools(sdk) {
    let traits = [];
    try { traits = await sdk.list(); } catch(e) { return []; }
    const wasmOnly = wasmReady && !helperReady;
    const tools = [];
    for (const t of traits) {
        if (!t.path) continue;
        if (VOICE_TOOL_EXCLUDE.has(t.path)) continue;
        if (t.path.startsWith('www.')) continue;
        const kind = (t.source || t.kind || '').toLowerCase();
        if (kind === 'library' || kind === 'interface') continue;
        if (wasmOnly && !t.wasm_callable) continue;

        const toolName = t.path.replace(/\./g, '_');
        const properties = {};
        const required = [];
        if (Array.isArray(t.params)) {
            for (const p of t.params) {
                const prop = _traitTypeToSchema(p.type || p.param_type);
                if (p.description) prop.description = p.description;
                properties[p.name] = prop;
                if (p.required !== false && !p.optional) required.push(p.name);
            }
        }
        const parameters = { type: 'object', properties };
        if (required.length) parameters.required = required;
        tools.push({ type: 'function', name: toolName, description: t.description || '', parameters });
    }
    // Always include the synthetic quit tool so the model can end the session
    tools.push({
        type: 'function',
        name: 'sys_voice_quit',
        description: 'End the voice conversation. Call this when the user says goodbye, wants to stop, or asks to quit.',
        parameters: { type: 'object', properties: {} }
    });

    return tools;
}

async function _ensureVoiceApiKey(traits) {
    if (_voiceApiKey) return _voiceApiKey;
    // Try to get from WASM secrets
    if (wasm && wasm.set_secret) {
        try {
            // Check if key exists in WASM
            const result = JSON.parse(wasm.call('sys.secrets', JSON.stringify(['get', 'openai_api_key'])));
            if (result.ok && result.result) {
                _voiceApiKey = String(result.result).trim();
                return _voiceApiKey;
            }
        } catch(e) {}
    }
    // Try from Settings page secrets (localStorage['traits.secret.OPENAI_API_KEY'])
    try {
        const settingsKey = (localStorage.getItem('traits.secret.OPENAI_API_KEY') || '').trim();
        if (settingsKey) {
            _voiceApiKey = settingsKey;
            // Also inject into WASM kernel so sys.secrets can resolve it
            if (wasm && wasm.set_secret) {
                try { wasm.set_secret('openai_api_key', settingsKey); } catch(e) {}
            }
            return _voiceApiKey;
        }
    } catch(e) {}
    // Try from legacy localStorage key (for development)
    try {
        const stored = (localStorage.getItem('traits.voice.api_key') || '').trim();
        if (stored) {
            _voiceApiKey = stored;
            return _voiceApiKey;
        }
    } catch(e) {}
    return null;
}

function _webllmProgress(text) {
    if (text) {
        _lastWebLLMStep = text;
        _webllmProgressTime = Date.now();
    }
    if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('webllm-progress', { detail: text }));
    }
}

async function _ensureWebLLM(model) {
    const modelId = model || WEBLLM_DEFAULT_MODEL;

    // Already loaded with same model
    if (_webllmEngine && _webllmModel === modelId) {
        _webllmProgress('Engine ready.');
        return _webllmEngine;
    }

    // If a previous load is in progress, wait briefly for it.
    // If it's stale (e.g. preload CDN import hanging), abandon and start fresh.
    if (_webllmLoading) {
        _webllmProgress('Waiting for previous load…');
        console.log('[WebLLM] Waiting for existing _webllmLoading…');
        const race = await Promise.race([
            _webllmLoading.then(() => 'done').catch(() => 'failed'),
            new Promise(r => setTimeout(() => r('stale'), 15_000)),
        ]);
        if (race === 'stale') {
            console.warn('[WebLLM] Previous load stale after 15s — starting fresh');
            _webllmProgress('Previous load timed out, restarting…');
            _webllmLoading = null;
            _webllmLib = null; // CDN import may have been the problem
        } else if (_webllmModel === modelId && _webllmEngine) {
            return _webllmEngine;
        }
        // else: previous load finished but different model or failed — continue
    }

    // Check WebGPU support
    _webllmProgress('Checking WebGPU…');
    console.log('[WebLLM] Checking WebGPU…');
    if (!navigator.gpu) throw new Error('WebGPU not supported in this browser (requires Chrome 113+ or Edge 113+)');
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error('WebGPU adapter not available — no compatible GPU found');
    console.log('[WebLLM] WebGPU adapter OK');

    _webllmLoading = (async () => {
        try {
            // Lazy-load WebLLM library with a 30s timeout
            if (!_webllmLib) {
                _webllmProgress('Downloading WebLLM library from CDN…');
                console.log('[WebLLM] Importing from https://esm.run/@mlc-ai/web-llm …');
                const imported = await Promise.race([
                    import('https://esm.run/@mlc-ai/web-llm'),
                    new Promise((_, reject) =>
                        setTimeout(() => reject(new Error(
                            'WebLLM CDN import timed out after 30s — check network/ad blocker'
                        )), 30_000)
                    ),
                ]);
                _webllmLib = imported;
                console.log('[WebLLM] Library imported OK');
                _webllmProgress('WebLLM library loaded.');
            }

            // Clean up existing engine
            if (_webllmEngine) {
                try { await _webllmEngine.unload(); } catch(e) {}
                _webllmEngine = null; _webllmModel = null;
            }

            _webllmProgress(`Loading model ${modelId}… (first run downloads ~1.7 GB)`);
            console.log('[WebLLM] Creating engine for', modelId);

            // Override context_window_size: the prebuilt config caps all models at 4096.
            // Clone the catalog, find our model, and raise the cap to 32K.
            const catalog = _webllmLib.prebuiltAppConfig;
            const modifiedList = catalog.model_list.map(rec =>
                rec.model_id === modelId
                    ? { ...rec, overrides: { ...rec.overrides, context_window_size: 32768 } }
                    : rec
            );

            // Watchdog: if no progress callback fires for 60s, CreateMLCEngine is stuck
            let progressFired = false;
            let clearWatchdog = () => {};
            const watchdog = new Promise((_, reject) => {
                const check = setInterval(() => {
                    const elapsed = Date.now() - _webllmProgressTime;
                    if (progressFired && elapsed < 60_000) return; // still making progress
                    if (!progressFired && elapsed > 60_000) {
                        clearInterval(check);
                        reject(new Error(
                            'WebLLM engine stalled — no progress for 60s. ' +
                            'This may indicate a browser/GPU compatibility issue. ' +
                            'Try Chrome 113+ with WebGPU enabled.'
                        ));
                    }
                }, 5_000);
                clearWatchdog = () => clearInterval(check);
            });

            const enginePromise = _webllmLib.CreateMLCEngine(modelId, {
                initProgressCallback: (report) => {
                    progressFired = true;
                    const text = report.text || `${Math.round((report.progress || 0) * 100)}%`;
                    console.log('[WebLLM] progress:', text);
                    _webllmProgress(text);
                },
                appConfig: { ...catalog, model_list: modifiedList },
            });

            try {
                _webllmEngine = await Promise.race([enginePromise, watchdog]);
            } finally {
                clearWatchdog();
            }
            _webllmModel = modelId;
            console.log('[WebLLM] Engine ready');
            _webllmProgress('Model ready.');
            return _webllmEngine;
        } catch(e) {
            console.error('[WebLLM] Engine load failed:', e);
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

function _syncRelayCodeFromHelper(info) {
    // If helper reports an active relay code, sync it to localStorage.
    // This auto-reconnects the relay after a Mac server restart without
    // requiring the user to manually re-enter the pairing code.
    try {
        const code = info?.relay?.code;
        const url  = info?.relay?.url;
        if (!code) return;
        const storedCode = localStorage.getItem('traits.relay.code');
        if (storedCode !== code) {
            localStorage.setItem('traits.relay.code', code);
            if (url) localStorage.setItem('traits.relay.server', url);
            // Clear any stored token if it was issued for a different code
            const tokenCode = _relayTokenCode();
            if (tokenCode && tokenCode !== code) {
                localStorage.removeItem('traits.relay.token');
            }
        }
    } catch(e) {}
}

async function discoverHelper() {
    // Try stored URL first
    try {
        const stored = localStorage.getItem('traits.helper.url');
        if (stored) {
            const info = await probeHelper(stored, 1000);
            if (info) {
                helperUrl = stored; helperInfo = info; helperReady = true;
                _syncRelayCodeFromHelper(info);
                return;
            }
        }
    } catch(e) {}
    // Auto-discover on common ports
    for (const port of HELPER_PORTS) {
        const url = `http://localhost:${port}`;
        const info = await probeHelper(url);
        if (info) {
            helperUrl = url; helperInfo = info; helperReady = true;
            try { localStorage.setItem('traits.helper.url', url); } catch(e) {}
            _syncRelayCodeFromHelper(info);
            return;
        }
    }
}

async function callHelper(path, args, opts = {}) {
    if (!helperReady) return null;
    const rest = path.replace(/\./g, '/');
    try {
        const url = `${helperUrl}/traits/${rest}` + (opts.stream ? '?stream=1' : '');
        const res = await fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ args }),
        });
        if (opts.stream && res.headers.get('content-type')?.includes('text/event-stream')) {
            return { ok: true, stream: readHelperSSE(res.body), dispatch: 'helper' };
        }
        const data = await res.json();
        return {
            ok: res.ok,
            result: res.ok ? data.result : undefined,
            error: res.ok ? undefined : (data.error || `HTTP ${res.status}`),
            dispatch: 'helper',
        };
    } catch(e) { return null; }
}

async function* readHelperSSE(body) {
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
        // Runtime binding table: interface path → implementation trait path
        this._bindings = new Map();
        // Pending deferred bindings: interface → { impl, cancel }
        this._pendingBindings = new Map();

        // WASM worker pool (for background SPA multitasking)
        this.workerPoolSize = Math.max(1, Number(opts.workerPoolSize || 2));
        this.workerUrl = opts.workerUrl || '';
        this._workerScriptUrl = null;
        this._workers = [];
        this._workerQueue = [];
        this._trackedTasks = new Map();
        this._nextWorkerMsgId = 1;
        this._nextTaskId = 1;

        // Background execution abstraction: iface binding -> adapter implementation.
        this._backgroundAdapters = new Map();
        this._installBuiltinBackgroundAdapters();
        if (!this._bindings.has(BACKGROUND_IFACE)) {
            this._bindings.set(BACKGROUND_IFACE, BACKGROUND_WORKER);
        }
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

    _installBuiltinBackgroundAdapters() {
        this.registerBackgroundAdapter(BACKGROUND_WORKER, {
            run: async ({ id, task }) => {
                if (task.cmd === 'call') {
                    const path = task.path || '';
                    if (!wasmCallableSet.has(path)) {
                        return { ok: false, id, error: `Trait '${path}' is not WASM-callable`, dispatch: 'worker' };
                    }
                }
                await this.initWorkerPool();
                return this._enqueueWorkerTask(id, task);
            },
        });

        this.registerBackgroundAdapter(BACKGROUND_DIRECT, {
            run: async ({ id, task }) => {
                if (task.cmd === 'call') {
                    const res = await this.call(task.path || '', task.args || []);
                    return { ...res, id, dispatch: res.dispatch || 'direct' };
                }

                if (!wasm) {
                    return { ok: false, id, error: `Direct background command '${task.cmd}' requires an attached WASM module`, dispatch: 'direct' };
                }

                switch (task.cmd) {
                    case 'cli_input':
                        return { ok: true, id, result: wasm.cli_input ? wasm.cli_input(task.data || '') : '', dispatch: 'direct' };
                    case 'cli_welcome':
                        return { ok: true, id, result: wasm.cli_welcome ? wasm.cli_welcome() : '', dispatch: 'direct' };
                    case 'cli_get_history':
                        return { ok: true, id, result: wasm.cli_get_history ? wasm.cli_get_history() : '[]', dispatch: 'direct' };
                    case 'cli_set_history':
                        if (wasm.cli_set_history) wasm.cli_set_history(task.history_json || '[]');
                        return { ok: true, id, result: true, dispatch: 'direct' };
                    case 'cli_format_rest_result':
                        return {
                            ok: true,
                            id,
                            result: wasm.cli_format_rest_result
                                ? wasm.cli_format_rest_result(task.path || '', task.args_json || '[]', task.result_json || 'null')
                                : '',
                            dispatch: 'direct',
                        };
                    case 'vfs_dump':
                        return { ok: true, id, result: wasm.vfs_dump ? wasm.vfs_dump() : '{}', dispatch: 'direct' };
                    case 'vfs_load':
                        if (wasm.vfs_load) wasm.vfs_load(task.json || '{}');
                        return { ok: true, id, result: true, dispatch: 'direct' };
                    case 'vfs_read':
                        return { ok: true, id, result: wasm.vfs_read ? wasm.vfs_read(task.path || '') : '', dispatch: 'direct' };
                    case 'vfs_write':
                        if (wasm.vfs_write) wasm.vfs_write(task.path || '', task.content || '');
                        return { ok: true, id, result: true, dispatch: 'direct' };
                    default:
                        return { ok: false, id, error: `Unsupported direct background command: '${task.cmd}'`, dispatch: 'direct' };
                }
            },
        });

        // Native helper-proxied backend. Meant for future tokio task traits.
        this.registerBackgroundAdapter(BACKGROUND_TOKIO, {
            run: async ({ id, task }) => {
                if (task.cmd !== 'call') {
                    return {
                        ok: false,
                        id,
                        error: `tokio backend currently supports only trait calls (got '${task.cmd}')`,
                        dispatch: 'tokio',
                    };
                }
                const res = await this.call(task.path || '', task.args || [], { force: 'helper' });
                if (res && res.ok) return { ...res, id, dispatch: 'tokio' };
                return {
                    ok: false,
                    id,
                    error: res?.error || 'tokio backend requires a reachable helper implementation',
                    dispatch: 'tokio',
                };
            },
        });
    }

    registerBackgroundAdapter(name, adapter) {
        if (!name || typeof name !== 'string') {
            throw new Error('Background adapter name must be a non-empty string');
        }
        if (!adapter || typeof adapter.run !== 'function') {
            throw new Error(`Background adapter '${name}' must provide a run() function`);
        }
        this._backgroundAdapters.set(name, adapter);
        return this;
    }

    getBackgroundBinding() {
        return this._bindings.get(BACKGROUND_IFACE) || BACKGROUND_WORKER;
    }

    setBackgroundBinding(impl) {
        this.bind(BACKGROUND_IFACE, impl);
        return this;
    }

    backgroundStatus() {
        return {
            binding: this.getBackgroundBinding(),
            adapters: [...this._backgroundAdapters.keys()],
            ...this.workerStatus(),
        };
    }

    /**
     * Initialize a WASM worker pool for background calls.
     * @param {number} [size] - Number of workers (default from constructor)
     * @returns {Promise<{ok: boolean, workers: number}>}
     */
    async initWorkerPool(size) {
        const target = Math.max(1, Number(size || this.workerPoolSize));
        if (this._workers.length >= target) {
            return { ok: true, workers: this._workers.length };
        }
        this._workerScriptUrl = this._workerScriptUrl || resolveWorkerScriptUrl(this.workerUrl);
        while (this._workers.length < target) {
            const state = await this._spawnWorker(this._workers.length);
            this._workers.push(state);
        }
        this._syncHelperToWorkers();
        return { ok: true, workers: this._workers.length };
    }

    /**
     * Stop all worker pool workers.
     */
    shutdownWorkerPool() {
        for (const w of this._workers) {
            try { w.worker.terminate(); } catch(e) {}
            if (wasm && wasm.unregister_task) {
                try { wasm.unregister_task(`worker-${w.index}`); } catch(e) {}
            }
            this._trackedTasks.delete(`worker-${w.index}`);
        }
        this._workers = [];
        this._workerQueue = [];
        if (this._workerScriptUrl && this._workerScriptUrl.startsWith('blob:')) {
            try { URL.revokeObjectURL(this._workerScriptUrl); } catch(e) {}
        }
        this._workerScriptUrl = null;
    }

    /**
     * Run a WASM-callable trait in the worker pool.
     * @param {string} path
     * @param {Array} [args=[]]
     * @returns {{id: string, promise: Promise<any>}}
     */
    spawn(path, args = [], opts = {}) {
        return this.executeBackground({ cmd: 'call', path, args }, opts);
    }

    /**
     * Run an arbitrary background task through the configured background adapter.
     * @param {Object} task
     * @param {string} task.cmd - Worker command (e.g. 'call', 'cli_input')
     * @param {Object} [opts]
     * @param {string} [opts.impl] - Override adapter implementation
     * @returns {{id: string, promise: Promise<any>}}
     */
    executeBackground(task, opts = {}) {
        const id = `task-${this._nextTaskId++}`;
        const meta = this._backgroundTaskMeta(task);
        const taskName = meta.name;
        const taskType = meta.taskType;
        const detail = meta.detail;
        const promise = (async () => {
            if (!this._initPromise) await this.init();
            // Register with WASM kernel for sys.ps visibility
            if (wasm && wasm.register_task) {
                try { wasm.register_task(id, taskName, taskType, Date.now(), detail); } catch(e) {}
            }
            this._trackTask(id, taskName, taskType, detail);
            const impl = opts.impl || this.getBackgroundBinding();
            const adapter = this._backgroundAdapters.get(impl);
            if (!adapter) {
                this._untrackTask(id);
                if (wasm && wasm.unregister_task) try { wasm.unregister_task(id); } catch(e) {}
                return { ok: false, id, error: `Unknown background adapter: '${impl}'`, dispatch: 'background' };
            }
            try {
                return await adapter.run({ id, task, opts, sdk: this });
            } finally {
                this._untrackTask(id);
                if (wasm && wasm.unregister_task) try { wasm.unregister_task(id); } catch(e) {}
            }
        })();
        return { id, promise };
    }

    /**
     * Convenience helper for worker-like commands used by background runtimes.
     * @param {string} cmd
     * @param {Object} [payload]
     * @param {Object} [opts]
     * @returns {Promise<any>}
     */
    async backgroundCall(cmd, payload = {}, opts = {}) {
        const job = this.executeBackground({ cmd, ...payload }, opts);
        return job.promise;
    }

    /**
     * Convenience wrapper around spawn() that awaits the result.
     * @param {string} path
     * @param {Array} [args=[]]
     * @returns {Promise<any>}
     */
    async callInWorker(path, args = []) {
        const job = this.executeBackground({ cmd: 'call', path, args }, { impl: BACKGROUND_WORKER });
        return job.promise;
    }

    /**
     * List worker pool status.
     * @returns {{workers: number, queued: number, running: number}}
     */
    workerStatus() {
        const running = this._workers.filter(w => w.busy).length;
        return { workers: this._workers.length, queued: this._workerQueue.length, running };
    }

    async _spawnWorker(index) {
        const worker = new Worker(this._workerScriptUrl);
        const pending = new Map();
        const state = { index, worker, pending, busy: false, syncedTaskIds: [] };

        worker.onmessage = (ev) => {
            const msg = ev.data || {};
            const req = pending.get(msg.id);
            if (!req) return;
            pending.delete(msg.id);
            if (msg.ok) req.resolve(msg.result);
            else req.reject(new Error(msg.error || 'Worker call failed'));
        };

        worker.onerror = (ev) => {
            for (const [, req] of pending) {
                req.reject(new Error(ev.message || 'Worker crashed'));
            }
            pending.clear();
            state.busy = false;
        };

        await this._rpcWorker(state, 'ping', {});
        await this._rpcWorker(state, 'init', {});
        await this._syncTasksToWorker(state);
        // Register worker as a service for sys.ps
        if (wasm && wasm.register_task) {
            try { wasm.register_task(`worker-${index}`, `Web Worker #${index}`, 'worker', Date.now(), BACKGROUND_WORKER); } catch(e) {}
        }
        this._trackTask(`worker-${index}`, `Web Worker #${index}`, 'worker', BACKGROUND_WORKER);
        return state;
    }

    _rpcWorker(state, cmd, payload) {
        return new Promise((resolve, reject) => {
            const id = this._nextWorkerMsgId++;
            state.pending.set(id, { resolve, reject });
            state.worker.postMessage({ id, cmd, payload });
        });
    }

    _drainWorkerQueue() {
        for (const state of this._workers) {
            if (state.busy) continue;
            const next = this._workerQueue.shift();
            if (!next) return;
            state.busy = true;
            this._rpcWorker(state, next.cmd, next.payload)
                .then((result) => {
                    next.resolve({
                        ok: true,
                        id: next.id,
                        result,
                        dispatch: 'worker',
                        worker: state.index,
                        ms: Math.round((performance.now() - next.t0) * 10) / 10,
                    });
                })
                .catch((e) => {
                    next.resolve({
                        ok: false,
                        id: next.id,
                        error: e.message || String(e),
                        dispatch: 'worker',
                        worker: state.index,
                        ms: Math.round((performance.now() - next.t0) * 10) / 10,
                    });
                })
                .finally(() => {
                    state.busy = false;
                    this._drainWorkerQueue();
                });
        }
    }

    _enqueueWorkerTask(id, task) {
        return new Promise((resolve) => {
            const t0 = performance.now();
            this._workerQueue.push({
                id,
                cmd: task.cmd,
                payload: { ...task },
                t0,
                resolve,
            });
            this._drainWorkerQueue();
        });
    }

    _syncHelperToWorkers() {
        for (const state of this._workers) {
            this._rpcWorker(state, 'set_helper_connected', { connected: helperReady }).catch(() => {});
        }
    }

    _trackTask(id, name, taskType, detail) {
        this._trackedTasks.set(String(id), {
            id: String(id),
            name: String(name || id),
            task_type: String(taskType || 'task'),
            started_ms: Date.now(),
            detail: String(detail || ''),
        });
        this._syncTasksToWorkers();
    }

    _untrackTask(id) {
        this._trackedTasks.delete(String(id));
        this._syncTasksToWorkers();
    }

    _snapshotMainThreadTasks() {
        if (!wasm || !wasm.call) return [];
        try {
            const raw = wasm.call('sys.ps', '[]');
            const parsed = JSON.parse(raw);
            const processes = Array.isArray(parsed?.processes) ? parsed.processes : [];
            return processes
                .filter((p) => p && p.id != null)
                .map((p) => ({
                    id: String(p.id),
                    name: String(p.name || p.id),
                    task_type: String(p.task_type || p.type || 'task'),
                    started_ms: Number(p.started_ms || Date.now()),
                    detail: String(p.detail || ''),
                }));
        } catch (e) {
            return [];
        }
    }

    _backgroundTaskMeta(task) {
        const cmd = String(task?.cmd || '');
        const path = String(task?.path || '');

        if (cmd === 'call' && path) {
            return { name: path, taskType: 'task', detail: `call(${path})` };
        }

        if (cmd === 'cli_input') {
            return { name: 'Terminal Input', taskType: 'task', detail: 'interactive CLI input' };
        }

        if (cmd === 'cli_format_rest_result') {
            return { name: 'Terminal Format', taskType: 'task', detail: 'REST result formatting' };
        }

        return {
            name: cmd || path || 'background task',
            taskType: 'task',
            detail: path ? `${cmd}(${path})` : cmd,
        };
    }

    _syncTasksToWorker(state) {
        const merged = new Map();
        for (const t of this._snapshotMainThreadTasks()) merged.set(t.id, t);
        for (const t of this._trackedTasks.values()) merged.set(t.id, t);
        const tasks = Array.from(merged.values());
        const existing_ids = state.syncedTaskIds || [];
        state.syncedTaskIds = tasks.map((t) => t.id);
        return this._rpcWorker(state, 'sync_tasks', { tasks, existing_ids });
    }

    _syncTasksToWorkers() {
        for (const state of this._workers) {
            this._syncTasksToWorker(state).catch(() => {});
        }
    }

    /**
     * Call a trait by dot-notation path.
     * WASM-callable traits dispatch locally; others go to the server.
     *
     * @param {string} path - Trait path (e.g. 'sys.checksum')
     * @param {Array} [args=[]] - Positional arguments
     * @param {Object} [opts] - Options
     * @param {string} [opts.force] - Force dispatch target: 'wasm', 'helper', 'native', 'relay', or 'rest'
     * @param {boolean} [opts.stream] - Enable SSE streaming (REST only)
     * @returns {Promise<any>} - Parsed result
     */
    async call(path, args = [], opts = {}) {
        // Ensure initialized
        if (!this._initPromise) await this.init();

        // 0. Binding resolution: redirect interface paths to bound implementations
        const bound = this._bindings.get(path);
        if (bound && bound !== path) {
            return this.call(bound, args, opts);
        }

        const forceMode = opts.force === 'native' ? 'helper' : opts.force;
        let wasmResult = null;

        // 1. WASM (instant, local)
        if (forceMode === 'wasm' || (!forceMode && wasmReady && wasmCallableSet.has(path))) {
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
        if (forceMode === 'helper' || (!forceMode && helperReady)) {
            const t0 = performance.now();
            const result = await callHelper(path, args, opts);
            if (result) {
                result.ms = Math.round((performance.now() - t0) * 10) / 10;
                return result;
            }
        }

        // 3. Relay (remote helper via pairing code)
        if (forceMode === 'relay' || (!forceMode && !helperReady && _relayCode())) {
            const t0 = performance.now();
            const result = await callRelay(path, args);
            if (result) {
                result.ms = Math.round((performance.now() - t0) * 10) / 10;
                return result;
            }
        }

        // 4. Server REST (if server URL is configured)
        if (this.server) {
            return this._callRest(path, args, opts);
        }

        // 5. No dispatch path available
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
        this._syncHelperToWorkers();
        // Register WASM kernel as a service for sys.ps
        if (mod.register_task) {
            try { mod.register_task('wasm-kernel', 'WASM Kernel', 'service', Date.now(), `${callable.length} callable traits`); } catch(e) {}
        }
        this._trackTask('wasm-kernel', 'WASM Kernel', 'service', `${callable.length} callable traits`);
    }

    /**
     * Start loading the default WebLLM model in the background.
     * If the model is already loaded, this is a no-op.
     * When a WebLLM call arrives later, it reuses the in-flight or completed engine.
     * @param {string} [model] - Model ID (default: WEBLLM_DEFAULT_MODEL)
     */
    preloadWebLLM(model) {
        if (typeof navigator === 'undefined' || !navigator.gpu) return; // no WebGPU
        _ensureWebLLM(model).catch(() => {}); // fire-and-forget, errors swallowed
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
                this._syncHelperToWorkers();
            }
        } else {
            await discoverHelper();
            if (helperReady) {
                syncHelperToWasm();
                this._syncHelperToWorkers();
            }
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
            this._syncHelperToWorkers();
            try { localStorage.setItem('traits.helper.url', helperUrl); } catch(e) {}
            _syncRelayCodeFromHelper(info);
            return { ok: true, ...info };
        }
        return { ok: false, error: 'Helper not reachable at ' + url };
    }

    // ── Voice (Browser Microphone) ──

    /**
     * Start voice conversation using browser microphone.
     * Uses OpenAI Realtime API for speech-to-speech conversation.
     * Connects directly from the browser — no relay or backend required.
     * Includes audio playback and function calling (tool use).
     * @param {Object} opts
     * @param {string} [opts.apiKey] - OpenAI API key (or set via setVoiceApiKey)
     * @param {string} [opts.voice='cedar'] - Voice: alloy, ash, ballad, coral, echo, sage, shimmer, verse, marin, cedar
     * @param {string} [opts.model='gpt-4o-mini-realtime-preview'] - Realtime model
     * @param {string} [opts.instructions] - Custom system instructions
     * @param {boolean} [opts.tools=true] - Enable function calling with trait tools
     * @param {Function} [opts.onTranscript] - Callback for user transcript
     * @param {Function} [opts.onResponse] - Callback for model response
     * @param {Function} [opts.onToolCall] - Callback for tool calls: (name, args) => void
     * @param {Function} [opts.onToolResult] - Callback for tool results: (name, result) => void
     * @param {Function} [opts.onAudio] - Callback for raw audio chunks (PCM16 Uint8Array)
     * @param {Function} [opts.onError] - Callback for errors
     * @returns {Promise<{ok: boolean, tools?: number, error?: string}>}
     */
    async startVoice(opts = {}) {
        // Stop any existing voice session
        await this.stopVoice();

        const rawKey = opts.apiKey || _voiceApiKey || await _ensureVoiceApiKey(this);
        const apiKey = rawKey ? rawKey.trim() : null;
        if (!apiKey) {
            return { ok: false, error: 'OpenAI API key required. Set OPENAI_API_KEY in Settings > Secrets' };
        }

        const voice = opts.voice || 'cedar';
        const model = opts.model || 'gpt-4o-mini-realtime-preview';
        const enableTools = opts.tools !== false;
        _voiceSdk = this;

        try {
            // ── Ephemeral token: browser WebRTC needs a short-lived token ──
            let ephemeralKey = null;

            // Try dispatch cascade (helper → relay → server) to mint one server-side
            try {
                const tokenResult = await this.call('sys.voice', ['token', model, voice]);
                const result = tokenResult.ok ? (tokenResult.result || tokenResult) : null;
                if (result && result.token) {
                    ephemeralKey = result.token;
                    console.log('[Voice] Got ephemeral token via helper/server');
                } else {
                    console.log('[Voice] Dispatch token result:', JSON.stringify(tokenResult).slice(0, 200));
                }
            } catch(e) {
                console.log('[Voice] Ephemeral token via dispatch failed:', e.message || e);
            }

            // Fallback: mint token directly from browser (OpenAI sets CORS: *)
            if (!ephemeralKey) {
                try {
                    const resp = await fetch('https://api.openai.com/v1/realtime/client_secrets', {
                        method: 'POST',
                        headers: {
                            'Authorization': 'Bearer ' + apiKey,
                            'Content-Type': 'application/json',
                        },
                        body: JSON.stringify({
                            session: { type: 'realtime', model,
                                       modalities: ['text', 'audio'],
                                       voice: voice,
                                       input_audio_transcription: { model: 'whisper-1' } }
                        })
                    });
                    const data = await resp.json();
                    if (resp.ok) {
                        const token = (data.client_secret && data.client_secret.value) || data.value;
                        if (token) {
                            ephemeralKey = token;
                            console.log('[Voice] Got ephemeral token via direct fetch');
                        } else {
                            console.warn('[Voice] Token response missing value:', JSON.stringify(data).slice(0, 200));
                        }
                    } else {
                        console.warn('[Voice] Token request failed:', resp.status, JSON.stringify(data).slice(0, 300));
                    }
                } catch(e) {
                    console.log('[Voice] Direct ephemeral token fetch failed:', e.message || e);
                }
            }

            if (!ephemeralKey) {
                return { ok: false, error: 'Could not obtain ephemeral token. Check that your OPENAI_API_KEY is valid and has Realtime API access.' };
            }

            // ── Request microphone access ──
            _voiceStream = await navigator.mediaDevices.getUserMedia({
                audio: {
                    echoCancellation: true,
                    noiseSuppression: true,
                    autoGainControl: true,
                }
            });

            // ── Build tool definitions ──
            let tools = [];
            if (enableTools) {
                tools = await _buildVoiceTools(this);
            }

            _dispatchVoiceEvent('started', { voice, model, tools: tools.length });

            // ── WebRTC peer connection ──
            _voicePc = new RTCPeerConnection();

            // Play remote audio from the model
            _voiceAudioEl = document.createElement('audio');
            _voiceAudioEl.autoplay = true;
            _voicePc.ontrack = (e) => { _voiceAudioEl.srcObject = e.streams[0]; };

            // Add local microphone track
            _voiceStream.getTracks().forEach(track => _voicePc.addTrack(track, _voiceStream));

            // Data channel for sending/receiving Realtime API events
            _voiceDc = _voicePc.createDataChannel('oai-events');

            // Handle data channel open — send session config
            _voiceDc.addEventListener('open', () => {
                const defaultInstructions = `You are a concise, helpful voice assistant powered by the traits.build platform.

Keep responses short and conversational — aim for 1–3 sentences unless the user asks for detail.
Use natural spoken language. Avoid bullet points, markdown, code blocks, or structured formatting — the user is listening, not reading.
When the user asks a technical question, give the answer directly. Don't over-explain unless asked to elaborate.
If you don't know something, say so briefly. Don't hedge excessively.

Be warm but not effusive. No filler greetings like "Great question!" or "Absolutely!".
Mirror the user's energy — if they're terse, be terse. If they're chatty, match it.
Use contractions naturally (I'm, you'll, that's, etc.).
Avoid repeating the user's question back to them.

Never output code blocks, URLs, file paths, or anything hard to speak aloud. If the user needs code, say "I can help with that — you'll want to check the docs or switch to text mode."
For numbers, spell them out when short (e.g. "three" not "3"), use digits for long ones.
Don't say "as an AI" or "as a language model." Just answer.

Don't monologue. Pause after answering to let the user respond.
If interrupted, stop immediately and listen to the new input.

You're running in the browser on the traits.build platform. The user is a developer.
You have access to function-calling tools that execute locally in the browser via WebAssembly.
When the user asks you to do something and a matching tool exists, call it directly — don't say you can't.`;

                // WebRTC session.update: voice/model/modalities are locked at token creation.
                // Only send mutable session fields here.
                const sessionConfig = {
                    instructions: opts.instructions || defaultInstructions,
                    input_audio_transcription: { model: 'whisper-1' },
                    turn_detection: {
                        type: 'server_vad',
                        threshold: 0.8,
                        prefix_padding_ms: 300,
                        silence_duration_ms: 800
                    }
                };
                if (tools.length > 0) sessionConfig.tools = tools;
                _voiceDc.send(JSON.stringify({ type: 'session.update', session: sessionConfig }));
            });

            // Handle incoming events (same JSON format as WebSocket messages)
            _voiceDc.addEventListener('message', async (event) => {
                try {
                    const msg = JSON.parse(event.data);
                    const type = msg.type;

                    // ── User transcript ──
                    if (type === 'conversation.item.input_audio_transcription.completed') {
                        if (opts.onTranscript && msg.transcript) {
                            opts.onTranscript(msg.transcript.trim());
                        }
                    }

                    // ── Model response transcript ──
                    else if (type === 'response.audio_transcript.done') {
                        if (opts.onResponse && msg.transcript) {
                            opts.onResponse(msg.transcript.trim());
                        }
                    }

                    // ── Function call — model wants to invoke a trait tool ──
                    else if (type === 'response.function_call_arguments.done') {
                        const callId = msg.call_id || '';
                        const funcName = msg.name || '';
                        const argsStr = msg.arguments || '{}';
                        if (opts.onToolCall) opts.onToolCall(funcName, argsStr);
                        _dispatchVoiceEvent('tool_call', { name: funcName, arguments: argsStr });

                        // Handle sys_voice_quit — stop the session
                        if (funcName === 'sys_voice_quit') {
                            if (_voiceDc && _voiceDc.readyState === 'open') {
                                _voiceDc.send(JSON.stringify({
                                    type: 'conversation.item.create',
                                    item: { type: 'function_call_output', call_id: callId, output: '{"ok":true,"action":"quit"}' }
                                }));
                            }
                            this.stopVoice();
                            return;
                        }

                        // Dispatch tool call via SDK cascade (WASM → helper → REST)
                        const traitPath = funcName.replace(/_/g, '.');
                        let callArgs = [];
                        try {
                            const parsed = JSON.parse(argsStr);
                            const traitInfo = await this.info(traitPath);
                            if (traitInfo && Array.isArray(traitInfo.params)) {
                                callArgs = traitInfo.params.map(p => parsed[p.name] !== undefined ? parsed[p.name] : null);
                            } else {
                                callArgs = Object.values(parsed);
                            }
                        } catch(e) { callArgs = []; }

                        this.call(traitPath, callArgs).then(result => {
                            const output = JSON.stringify(result.ok ? (result.result !== undefined ? result.result : result) : { error: result.error });
                            const truncated = output.length > 2000 ? output.slice(0, 2000) + '…(truncated)' : output;
                            if (_voiceDc && _voiceDc.readyState === 'open') {
                                _voiceDc.send(JSON.stringify({
                                    type: 'conversation.item.create',
                                    item: { type: 'function_call_output', call_id: callId, output: truncated }
                                }));
                                _voiceDc.send(JSON.stringify({ type: 'response.create' }));
                            }
                            if (opts.onToolResult) opts.onToolResult(funcName, truncated);
                            _dispatchVoiceEvent('tool_result', { name: funcName, result: truncated });
                        }).catch(e => {
                            if (_voiceDc && _voiceDc.readyState === 'open') {
                                _voiceDc.send(JSON.stringify({
                                    type: 'conversation.item.create',
                                    item: { type: 'function_call_output', call_id: callId, output: JSON.stringify({ error: e.message }) }
                                }));
                                _voiceDc.send(JSON.stringify({ type: 'response.create' }));
                            }
                        });
                    }

                    // ── Error ──
                    else if (type === 'error') {
                        const errMsg = msg.error?.message || 'Unknown error';
                        _dispatchVoiceEvent('error', { message: errMsg });
                        if (opts.onError) opts.onError(errMsg);
                    }
                } catch(e) {
                    console.warn('[Voice] message parse error:', e);
                }
            });

            // Handle connection state changes
            _voicePc.onconnectionstatechange = () => {
                if (_voicePc && (_voicePc.connectionState === 'disconnected' || _voicePc.connectionState === 'failed')) {
                    _dispatchVoiceEvent('disconnected', {});
                }
            };

            // ── SDP negotiation via OpenAI Realtime API ──
            const offer = await _voicePc.createOffer();
            await _voicePc.setLocalDescription(offer);

            console.log('[Voice] Connecting via WebRTC to', model, '— ephemeral token (' + ephemeralKey.length + ' chars)');
            const sdpResponse = await fetch('https://api.openai.com/v1/realtime/calls', {
                method: 'POST',
                body: offer.sdp,
                headers: {
                    'Authorization': 'Bearer ' + ephemeralKey,
                    'Content-Type': 'application/sdp',
                },
            });

            if (!sdpResponse.ok) {
                const errText = await sdpResponse.text();
                throw new Error(`WebRTC SDP exchange failed (${sdpResponse.status}): ${errText.slice(0, 200)}`);
            }

            const answerSdp = await sdpResponse.text();
            await _voicePc.setRemoteDescription({ type: 'answer', sdp: answerSdp });

            return { ok: true, tools: tools.length };

        } catch(e) {
            await this.stopVoice();
            return { ok: false, error: e.message || String(e) };
        }
    }

    /**
     * Stop the current voice session.
     * @returns {Promise<void>}
     */
    async stopVoice() {
        if (_voiceDc) {
            try { _voiceDc.close(); } catch(e) {}
            _voiceDc = null;
        }
        if (_voicePc) {
            _voicePc.close();
            _voicePc = null;
        }
        if (_voiceStream) {
            _voiceStream.getTracks().forEach(track => track.stop());
            _voiceStream = null;
        }
        if (_voiceAudioEl) {
            _voiceAudioEl.srcObject = null;
            _voiceAudioEl = null;
        }
        _voiceSdk = null;
        _dispatchVoiceEvent('stopped', {});
    }

    /**
     * Check if voice session is active.
     * @returns {boolean}
     */
    isVoiceActive() {
        return _voiceDc !== null && _voiceDc.readyState === 'open';
    }

    /**
     * Set the OpenAI API key for voice.
     * @param {string} apiKey
     */
    setVoiceApiKey(apiKey) {
        _voiceApiKey = apiKey;
        try {
            localStorage.setItem('traits.voice.api_key', apiKey);
        } catch(e) {}
    }

    // ── Runtime Bindings ──

    /**
     * Set an immediate binding: interface → implementation.
     * All calls to the interface path will be redirected to the implementation.
     * @param {string} iface - Interface path (e.g. 'llm/prompt')
     * @param {string} impl - Implementation trait path (e.g. 'llm.prompt.openai')
     * @returns {this}
     */
    bind(iface, impl) {
        const prev = this._bindings.get(iface) || null;
        this._bindings.set(iface, impl);
        if (typeof window !== 'undefined') {
            window.dispatchEvent(new CustomEvent('traits-binding', {
                detail: { interface: iface, impl, previous: prev }
            }));
        }
        return this;
    }

    /**
     * Remove a binding. Calls to the interface will no longer be redirected.
     * Also cancels any pending deferred binding for this interface.
     * @param {string} iface - Interface path
     * @returns {this}
     */
    unbind(iface) {
        const prev = this._bindings.get(iface) || null;
        this._bindings.delete(iface);
        const pending = this._pendingBindings.get(iface);
        if (pending) { pending.cancel(); this._pendingBindings.delete(iface); }
        if (typeof window !== 'undefined') {
            window.dispatchEvent(new CustomEvent('traits-binding', {
                detail: { interface: iface, impl: null, previous: prev }
            }));
        }
        return this;
    }

    /**
     * Get the current binding for an interface.
     * @param {string} iface - Interface path
     * @returns {string|null} - Bound implementation path, or null
     */
    getBinding(iface) {
        return this._bindings.get(iface) || null;
    }

    /**
     * List all active bindings.
     * @returns {Object} - { 'llm/prompt': 'llm.prompt.openai', ... }
     */
    listBindings() {
        return Object.fromEntries(this._bindings);
    }

    /**
     * List pending (deferred) bindings that haven't resolved yet.
     * @returns {Object} - { 'llm/prompt': 'llm.prompt.webllm', ... }
     */
    listPendingBindings() {
        const result = {};
        for (const [iface, entry] of this._pendingBindings) {
            result[iface] = entry.impl;
        }
        return result;
    }

    /**
     * Deferred binding: bind an interface to an implementation when a Promise resolves.
     * While the promise is pending, the existing binding (if any) stays active.
     * When the promise resolves, the binding switches automatically.
     * If the promise rejects, a 'traits-binding-error' event fires.
     *
     * @param {string} iface - Interface path (e.g. 'llm/prompt')
     * @param {string} impl - Implementation to bind when ready (e.g. 'llm.prompt.webllm')
     * @param {Promise} readyPromise - Resolves when the implementation is ready
     * @returns {this}
     */
    bindWhenReady(iface, impl, readyPromise) {
        // Cancel any existing pending binding for this interface
        const existing = this._pendingBindings.get(iface);
        if (existing) existing.cancel();

        let cancelled = false;
        const entry = { impl, cancel: () => { cancelled = true; } };
        this._pendingBindings.set(iface, entry);

        readyPromise.then(() => {
            if (cancelled) return;
            this._pendingBindings.delete(iface);
            this.bind(iface, impl);
        }).catch(err => {
            if (cancelled) return;
            this._pendingBindings.delete(iface);
            if (typeof window !== 'undefined') {
                window.dispatchEvent(new CustomEvent('traits-binding-error', {
                    detail: { interface: iface, impl, error: err.message || String(err) }
                }));
            }
        });

        return this;
    }

    /**
     * Disconnect from helper and clear stored URL.
     */
    disconnectHelper() {
        helperReady = false;
        helperUrl = null;
        helperInfo = null;
        syncHelperToWasm();
        this._syncHelperToWorkers();
        try { localStorage.removeItem('traits.helper.url'); } catch(e) {}
    }

    // ── Relay (remote helper via pairing code) ──

    /**
     * Connect to a remote relay. Stores code + server in localStorage.
     * @param {string} code - 4-char pairing code from Mac helper
     * @param {string} [server] - Relay server URL (defaults to relay.traits.build)
     * @returns {Promise<{ok: boolean, active?: boolean, error?: string}>}
     */
    async connectRelay(code, server) {
        const relayServer = server || RELAY_DEFAULT_SERVER;
        try {
            // Verify Mac is actually polling before storing anything
            const statusRes = await fetch(`${relayServer}/relay/status?code=${encodeURIComponent(code)}`);
            const statusData = await statusRes.json();
            if (!statusData.active) return { ok: false, error: 'No helper connected with that code — run traits serve on your Mac first' };
            localStorage.setItem('traits.relay.code', code);
            localStorage.setItem('traits.relay.server', relayServer);
            localStorage.setItem(RELAY_ENABLED_KEY, '1');
            // Request a signed token for password-free future reconnects (best-effort)
            try {
                const tokenRes = await fetch(`${relayServer}/relay/connect`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ code }),
                });
                if (tokenRes.ok) {
                    const tokenData = await tokenRes.json();
                    if (tokenData.token) localStorage.setItem('traits.relay.token', tokenData.token);
                }
            } catch(_) { /* token is optional — code-based flow still works */ }
            // Send _ping so Mac logs the connection
            try {
                await fetch(`${relayServer}/relay/call`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ code, path: '_ping', args: [] }),
                });
            } catch(_) {}
            return { ok: true, active: true, hasToken: !!localStorage.getItem('traits.relay.token') };
        } catch(e) {
            return { ok: false, error: 'Cannot reach relay server: ' + e.message };
        }
    }

    /**
     * Disconnect from relay without forgetting the saved pairing code.
     */
    disconnectRelay() {
        try {
            localStorage.setItem(RELAY_ENABLED_KEY, '0');
            localStorage.removeItem('traits.relay.token');
        } catch(e) {}
    }

    /**
     * Check relay connection status.
     * @returns {Promise<{connected: boolean, code?: string, server?: string, active?: boolean}>}
     */
    async relayStatus() {
        const token = !_relayTokenExpired() ? _relayToken() : null;
        const code  = _relayCode();
        if (!token && !code) return { connected: false };
        const server = _relayServer();
        try {
            const url = token
                ? `${server}/relay/status?token=${encodeURIComponent(token)}`
                : `${server}/relay/status?code=${encodeURIComponent(code)}`;
            const res  = await fetch(url);
            if (res.status === 401) {
                // Token rejected — clear it, fall back to code
                try { localStorage.removeItem('traits.relay.token'); } catch(_) {}
                return { connected: false, code, server, error: 'token_expired' };
            }
            const data = await res.json();
            // Server echoes back the resolved code — keep localStorage in sync
            if (data.code && data.code !== code) localStorage.setItem('traits.relay.code', data.code);
            return { connected: data.active, code: data.code || code, server, hasToken: !!token, ...data };
        } catch(e) {
            return { connected: false, code, server, error: e.message };
        }
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
        const relayCode = _relayCode();
        const rememberedRelayCode = _rememberedRelayCode();
        return {
            wasm: wasmReady,
            traits: this._wasmInfo?.traits_registered || 0,
            callable: this._wasmInfo?.wasm_callable || 0,
            version: this._wasmInfo?.version || null,
            helper: helperReady,
            helperUrl: helperUrl,
            relay: !!relayCode,
            relayCode: relayCode,
            relayRememberedCode: rememberedRelayCode,
            relayServer: rememberedRelayCode ? _relayServer() : null,
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

    async _callWebLLM(prompt, model, onToken) {
        // 5 minutes for first-time model download (~1.7 GB), subsequent calls are fast
        const TIMEOUT_MS = 300_000;
        const t0 = performance.now();
        _lastWebLLMStep = '';
        let aborted = false;
        try {
            const result = await Promise.race([
                (async () => {
                    const engine = await _ensureWebLLM(model);
                    _webllmProgress('Running inference…');
                    console.log('[WebLLM] Starting inference, prompt length:', prompt.length);
                    let content = '';
                    if (typeof onToken === 'function') {
                        // Streaming mode: tokens arrive one at a time
                        const stream = await engine.chat.completions.create({
                            messages: [{ role: 'user', content: prompt }],
                            temperature: 0.7,
                            max_tokens: 1024,
                            stream: true,
                            stream_options: { include_usage: true },
                        });
                        let firstToken = true;
                        for await (const chunk of stream) {
                            if (aborted) break;
                            const delta = chunk.choices?.[0]?.delta?.content || '';
                            if (delta) {
                                if (firstToken) {
                                    _webllmProgress('');   // clear progress line on first token
                                    firstToken = false;
                                }
                                content += delta;
                                onToken(delta);
                            }
                        }
                    } else {
                        // Non-streaming fallback
                        const reply = await engine.chat.completions.create({
                            messages: [{ role: 'user', content: prompt }],
                            temperature: 0.7,
                            max_tokens: 1024,
                        });
                        content = reply.choices?.[0]?.message?.content || '';
                    }
                    if (aborted) return { ok: true, result: content, dispatch: 'webllm', model: _webllmModel, ms: Math.round((performance.now() - t0) * 10) / 10 };
                    const dt = performance.now() - t0;
                    console.log('[WebLLM] Inference done in', Math.round(dt), 'ms, chars:', content.length);
                    _webllmProgress('');
                    return {
                        ok: true,
                        result: content,
                        dispatch: 'webllm',
                        model: _webllmModel,
                        ms: Math.round(dt * 10) / 10,
                    };
                })(),
                new Promise((_, reject) =>
                    setTimeout(() => {
                        aborted = true;
                        reject(new Error(
                            `WebLLM timed out after ${TIMEOUT_MS / 1000}s` +
                            (_lastWebLLMStep ? ` (last step: ${_lastWebLLMStep})` : '') +
                            `. Check browser console (F12) for [WebLLM] logs.`
                        ));
                    }, TIMEOUT_MS)
                ),
            ]);
            return result;
        } catch (e) {
            console.error('[WebLLM] _callWebLLM error:', e);
            _webllmProgress('');
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

            // Guard against non-JSON responses (e.g. HTML 404 from static hosting)
            const ct = res.headers.get('content-type') || '';
            if (!ct.includes('json')) {
                return { ok: false, error: `HTTP ${res.status}`, dispatch: 'rest', ms: Math.round(dt * 10) / 10 };
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

// ────────────────── MCP Server (browser-only, WASM-powered) ──────────────────

/**
 * Browser-only MCP server backed by the WASM kernel.
 * No server or relay needed — all tool dispatch happens locally in the browser.
 *
 * Supports three transports:
 *   1. Direct: call mcpServer.message(jsonRpcString) → jsonRpcResponse
 *   2. BroadcastChannel: cross-tab MCP over a named channel
 *   3. MessagePort: iframe/worker/extension MCP over a MessagePort
 *
 * Usage:
 *   const mcp = new McpServer();                    // direct mode
 *   const resp = mcp.message('{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}');
 *
 *   const mcp = new McpServer({ channel: 'traits-mcp' });  // cross-tab
 *   // Other tabs: new BroadcastChannel('traits-mcp').postMessage({jsonrpc:'2.0',...})
 *
 *   const mcp = new McpServer({ port: messagePort });       // worker/iframe
 */
class McpServer {
    constructor(opts = {}) {
        this._listeners = [];

        if (opts.channel && typeof BroadcastChannel !== 'undefined') {
            this._bc = new BroadcastChannel(opts.channel);
            this._bc.onmessage = (e) => this._handleTransportMessage(e.data, (resp) => this._bc.postMessage(resp));
        }

        if (opts.port) {
            this._port = opts.port;
            this._port.onmessage = (e) => this._handleTransportMessage(e.data, (resp) => this._port.postMessage(resp));
            if (this._port.start) this._port.start();
        }
    }

    /** Process a single JSON-RPC message string. Returns JSON-RPC response string (empty for notifications). */
    message(jsonRpcString) {
        if (!wasm || typeof wasm.mcp_message !== 'function') {
            return JSON.stringify({
                jsonrpc: '2.0', id: null,
                error: { code: -32603, message: 'WASM kernel not loaded — call Traits.init() first' }
            });
        }
        return wasm.mcp_message(jsonRpcString);
    }

    /** Process a parsed JSON-RPC object. Returns parsed response (or null for notifications). */
    handle(jsonRpcObj) {
        const resp = this.message(JSON.stringify(jsonRpcObj));
        if (!resp) return null;
        try { return JSON.parse(resp); } catch { return null; }
    }

    /** Listen for MCP events (for logging/debugging). */
    onMessage(fn) { this._listeners.push(fn); }

    _handleTransportMessage(data, reply) {
        const msg = typeof data === 'string' ? data : JSON.stringify(data);
        for (const fn of this._listeners) try { fn('request', msg); } catch {}
        const resp = this.message(msg);
        if (resp) {
            for (const fn of this._listeners) try { fn('response', resp); } catch {}
            try { reply(JSON.parse(resp)); } catch { reply(resp); }
        }
    }

    /** Stop listening on BroadcastChannel / MessagePort. */
    close() {
        if (this._bc) { this._bc.close(); this._bc = null; }
        if (this._port) { this._port.onmessage = null; this._port = null; }
        this._listeners.length = 0;
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
