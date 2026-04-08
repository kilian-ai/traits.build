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

function parseDispatchTarget(path) {
    if (typeof path !== 'string') return { cleanPath: path, target: null };
    const at = path.lastIndexOf('@');
    if (at < 0) return { cleanPath: path, target: null };
    const suffix = path.slice(at + 1).toLowerCase();
    const target = new Set(['wasm', 'native', 'helper', 'relay', 'rest']).has(suffix)
        ? suffix
        : null;
    if (!target) return { cleanPath: path, target: null };
    return { cleanPath: path.slice(0, at), target };
}

// ── Local voice state (WebGPU STT + LLM + TTS) ──
let _localVoiceActive = false;
let _localVoiceStream = null;
let _localVoiceAudioCtx = null;
let _localVoiceProcessor = null;
let _sttPipeline = null;
let _sttLoading = null;
let _ttsModel = null;
let _ttsLoading = null;
let _transformersLib = null;

// ── Voxtral (local-realtime) voice state ──
let _voxtralVoiceActive = false;
let _voxtralVoiceStream = null;
let _voxtralVoiceAudioCtx = null;
let _voxtralVoiceNode = null;
let _voxtralProcessor = null;
let _voxtralModel = null;
let _voxtralLib4 = null;
let _voxtralModelLoading = null;

// Traits excluded from voice function-calling tools (mirrors native TOOL_EXCLUDE)
const VOICE_TOOL_EXCLUDE = new Set([
    'sys.voice', 'sys.mcp', 'sys.serve', 'sys.cli', 'sys.cli.native', 'sys.cli.wasm',
    'sys.dylib_loader', 'sys.reload', 'sys.release', 'sys.secrets',
    'sys.canvas', 'sys.vfs', 'llm.agent',
    'kernel.main', 'kernel.dispatcher', 'kernel.globals', 'kernel.registry',
    'kernel.config', 'kernel.plugin_api', 'kernel.cli',
    'www.admin', 'www.admin.deploy', 'www.admin.fast_deploy',
    'www.admin.scale', 'www.admin.destroy', 'www.admin.save_config',
]);

const CANVAS_AGENT_SYSTEM =
    '================================================================\n' +
    'MANDATORY GAME RULES — READ FIRST — NON-NEGOTIABLE\n' +
    '================================================================\n' +
    'ANY game you build MUST contain ALL FOUR of these. No exceptions.\n' +
    'Do NOT write the file until every item below is implemented.\n\n' +
    'RULE 1 — SCORE + HIGH SCORE WITH INITIALS (REQUIRED)\n' +
    '  • Live score visible at all times during play.\n' +
    '  • High score persisted in localStorage with 3-char initials.\n' +
    '  • On game over: if score > high score, show an initials-entry prompt (HTML input or key capture). Save new record. Display it on screen at all times.\n' +
    '  • "AAA 0" is a valid default. There is NO excuse to skip this.\n\n' +
    'RULE 2 — POWER-UPS & BONUS FEATURES (REQUIRED — AS MANY AS POSSIBLE)\n' +
    '  • Include at minimum 6 distinct power-ups. More is better.\n' +
    '  • Examples (use ALL of these plus more): extra life, shield, speed boost, slow-motion, multi-ball, laser, magnet, double-score, invincibility, bomb/clear-screen, score multiplier, mystery box, combo streak, ghost ball, fire mode, time freeze.\n' +
    '  • Each power-up must have a visible falling/floating icon with a distinct color and label.\n' +
    '  • Spawn from destroyed objects AND on a timer. Show active power-up status on HUD.\n\n' +
    'RULE 3 — LEVELS 5–10 MINIMUM (REQUIRED)\n' +
    '  • At least 5 levels, ideally 8–10. Each must be meaningfully different.\n' +
    '  • Vary: brick/obstacle layout, enemy patterns, ball speed, background color/theme, special hazards.\n' +
    '  • Show a level-intro screen for 1–2s (e.g. "LEVEL 3 — DANGER ZONE") before gameplay starts.\n' +
    '  • Difficulty ramps with each level (speed, density, enemy count).\n' +
    '  • After the final level: show a VICTORY screen with score and high score.\n\n' +
    'RULE 4 — MUSIC + SOUND FX via WebAudio API (REQUIRED — NO EXTERNAL FILES)\n' +
    '  • ALL audio must be generated programmatically using new AudioContext() + oscillators + gain envelopes.\n' +
    '  • Background music: looping melody/rhythm built from oscillators. Changes or intensifies each level.\n' +
    '  • Sound FX required for: ball/object hit, brick/enemy destroyed, power-up collected, level up, game over, new high score.\n' +
    '  • Mute/unmute button visible on screen at all times.\n' +
    '  • No <audio> tags. No fetch(). No external URLs. WebAudio API only.\n\n' +
    '================================================================\n' +
    'PRE-FLIGHT CHECKLIST — run this before EVERY sys_vfs write call:\n' +
    '  [ ] Score counter visible + updating                (RULE 1)\n' +
    '  [ ] High score with initials in localStorage        (RULE 1)\n' +
    '  [ ] Initials prompt on new high score               (RULE 1)\n' +
    '  [ ] 6+ power-ups with icons + HUD status            (RULE 2)\n' +
    '  [ ] 5+ levels with unique layouts                   (RULE 3)\n' +
    '  [ ] Level-intro transition per level                (RULE 3)\n' +
    '  [ ] WebAudio background music loop                  (RULE 4)\n' +
    '  [ ] WebAudio SFX for all events                     (RULE 4)\n' +
    '  [ ] Mute button on screen                           (RULE 4)\n' +
    'If ANY box is unchecked, implement it BEFORE writing. No partial games.\n' +
    '================================================================\n\n' +
    'You are a canvas code executor. NEVER explain, suggest, or answer in text. ALWAYS call tools immediately.\n\n' +
    'WORKFLOW — execute these steps in order, no skipping, no chatting:\n' +
    '1. sys_vfs(action=read, path=canvas/app.html) — read the current file\n' +
    '2. Apply the requested change to the full HTML (honoring ALL rules above)\n' +
    '3. sys_vfs(action=write, path=canvas/app.html, content=<COMPLETE updated HTML>) — write the whole file, never a diff\n\n' +
    'DIMENSIONS: 390px wide × 844px tall — fills the phone viewport.\n' +
    '- body/root: width:390px; height:844px; overflow:hidden; margin:0\n' +
    '- <canvas>: set attribute width=390 height=844 and CSS width:390px;height:844px\n\n' +
    'RENDERING (HTML injected into div#phone-viewport):\n' +
    '- Get canvas: document.querySelector(\'canvas\') — scripts run inside an iframe, no #phone-viewport prefix needed\n' +
    '- Use let (NEVER const) for any reassigned variable. const in loops crashes silently.\n' +
    '- Cancel existing animation before starting: if(window.__canvasAnimId) cancelAnimationFrame(window.__canvasAnimId);\n' +
    '- Store new id: window.__canvasAnimId = requestAnimationFrame(loop)\n' +
    '- No DOMContentLoaded — script runs immediately on injection\n' +
    '- No external dependencies — inline all CSS and JS\n\n' +
    'STYLE: Dark bg #0a0a0a, bright accents (#00ff88, #ff6b35, #4fc3f7), smooth 60fps.\n' +
    'Canvas scripts can call: traits.call(path,args), traits.echo(text), traits.audio(action,...).'

// ── Shared canvas agent runner — used by BOTH WebRTC and local voice paths ──
// Prefers browser-native direct OpenAI fetch (no helper/server needed).
async function _runCanvasAgent(sdk, request) {
    let _existing = '';
    try {
        const pvfs = JSON.parse(localStorage.getItem('traits.pvfs') || '{}');
        _existing = pvfs['canvas/app.html'] || '';
    } catch(_) {}

    console.log('[Canvas/Agent] ▶ Starting — existing:', _existing.length, 'chars | request:', request);

    // ── Browser-native path: direct OpenAI fetch — works without helper or server ──
    const apiKey = _voiceApiKey || await _ensureVoiceApiKey(sdk).catch(() => null);
    if (apiKey) {
        return await _runCanvasAgentBrowser(request, _existing, apiKey);
    }

    // ── Fallback: dispatch through SDK cascade (needs helper or server) ──
    const prompt = _existing
        ? `User request: ${request}\n\nRead canvas/app.html, apply the change, write the COMPLETE updated file back immediately.`
: `Build the following for the canvas:\n\n${request}\n\nWrite a complete, self-contained HTML+CSS+JS file to canvas/app.html. Requirements:\n- 390px wide × 844px tall, fills the phone viewport\n- Dark theme: background #0a0a0a, bright accent colors\n- Inline all CSS and JS — no external dependencies\n- querySelector('canvas') for canvas access (scripts run inside an iframe)\n- let (not const) for any reassigned variables\n- Cancel any existing animation first: if(window.__canvasAnimId) cancelAnimationFrame(window.__canvasAnimId)\n- Store new animation ID: window.__canvasAnimId = requestAnimationFrame(loop)\n- No DOMContentLoaded listeners`;

    const agentArgs = [prompt, CANVAS_AGENT_SYSTEM, 'sys.vfs,sys.canvas', 'gpt-4.1', 20];
    try {
        const result = await sdk.call('llm.agent', agentArgs);
        const r = result?.result || result;
        let content = '';
        if (Array.isArray(r?.tool_calls)) {
            for (let i = r.tool_calls.length - 1; i >= 0; i--) {
                const tc = r.tool_calls[i];
                if (tc.name === 'sys_vfs' && tc.args?.action === 'write' &&
                    (tc.args?.path === 'canvas/app.html' || String(tc.args?.path || '').endsWith('/app.html'))) {
                    content = String(tc.args?.content || '');
                    console.log('[Canvas/Agent] Content from sys_vfs write, len:', content.length);
                    break;
                }
                if (tc.name === 'sys_canvas' && tc.args?.action === 'set') {
                    content = String(tc.args?.content || '');
                    console.log('[Canvas/Agent] Content from sys_canvas set, len:', content.length);
                    break;
                }
            }
        }
        if (!content) {
            try { const pvfs = JSON.parse(localStorage.getItem('traits.pvfs') || '{}'); content = pvfs['canvas/app.html'] || ''; } catch(_) {}
        }
        if (content) {
            window.dispatchEvent(new CustomEvent('traits-canvas-update', { detail: { content } }));
        }
        return JSON.stringify(r?.ok ? { ok: true, response: r.response || 'Done' } : { error: r?.error || 'agent failed' });
    } catch(e) {
        console.error('[Canvas/Agent] ✗ Error:', e.message || e);
        return JSON.stringify({ error: e.message || String(e) });
    }
}

// ── Browser-native canvas agent loop — direct OpenAI chat completions, no server needed ──
// Handles sys_vfs read/write locally via localStorage['traits.pvfs'].
async function _runCanvasAgentBrowser(request, existing, apiKey) {
    const SYS_VFS_TOOL = {
        type: 'function',
        function: {
            name: 'sys_vfs',
            description: 'Read or write a file in the canvas virtual filesystem',
            parameters: {
                type: 'object',
                properties: {
                    action: { type: 'string', enum: ['read', 'write'], description: 'read or write' },
                    path: { type: 'string', description: 'file path, e.g. canvas/app.html' },
                    content: { type: 'string', description: 'full file content (write only)' }
                },
                required: ['action', 'path']
            }
        }
    };

    const messages = [
        { role: 'system', content: CANVAS_AGENT_SYSTEM },
        { role: 'user', content: existing
            ? `User request: ${request}\n\nRead canvas/app.html, apply the change, write the COMPLETE updated file back immediately.`
: `Build the following for the canvas:\n\n${request}\n\nWrite a complete, self-contained HTML+CSS+JS file to canvas/app.html. Requirements:\n- 390px wide × 844px tall, fills the phone viewport\n- Dark theme: background #0a0a0a, bright accent colors\n- Inline all CSS and JS — no external dependencies\n- querySelector('canvas') for canvas access (scripts run inside an iframe)\n- let (not const) for any reassigned variables\n- Cancel any existing animation first: if(window.__canvasAnimId) cancelAnimationFrame(window.__canvasAnimId)\n- Store new animation ID: window.__canvasAnimId = requestAnimationFrame(loop)\n- No DOMContentLoaded listeners`
        }
    ];

    let lastContent = '';
    try {
        for (let step = 0; step < 6; step++) {
            console.log('[Canvas/Agent/Browser] Step', step + 1);
            const resp = await fetch('https://api.openai.com/v1/chat/completions', {
                method: 'POST',
                headers: { 'Authorization': 'Bearer ' + apiKey, 'Content-Type': 'application/json' },
                body: JSON.stringify({ model: 'gpt-4.1', messages, tools: [SYS_VFS_TOOL], tool_choice: 'auto' })
            });
            if (!resp.ok) {
                const err = await resp.text().catch(() => String(resp.status));
                console.error('[Canvas/Agent/Browser] OpenAI error:', err);
                return JSON.stringify({ error: 'OpenAI error: ' + err });
            }
            const data = await resp.json();
            const choice = data.choices?.[0];
            const msg = choice?.message;
            if (!msg) break;
            messages.push(msg);
            const toolCalls = msg.tool_calls || [];
            if (toolCalls.length === 0) { console.log('[Canvas/Agent/Browser] Done at step', step + 1); break; }
            for (const tc of toolCalls) {
                let args = {};
                try { args = JSON.parse(tc.function.arguments || '{}'); } catch(_) {}
                let toolResult = '{"error":"unknown tool"}';
                if (tc.function.name === 'sys_vfs') {
                    if (args.action === 'read') {
                        let pvfs = {}; try { pvfs = JSON.parse(localStorage.getItem('traits.pvfs') || '{}'); } catch(_) {}
                        const fileContent = pvfs[args.path] || '';
                        toolResult = fileContent || '(empty)';
                        console.log('[Canvas/Agent/Browser] VFS read:', args.path, fileContent.length, 'chars');
                    } else if (args.action === 'write') {
                        let pvfs = {}; try { pvfs = JSON.parse(localStorage.getItem('traits.pvfs') || '{}'); } catch(_) {}
                        pvfs[args.path] = args.content || '';
                        try { localStorage.setItem('traits.pvfs', JSON.stringify(pvfs)); } catch(_) {}
                        lastContent = args.content || '';
                        const isCanvas = args.path === 'canvas/app.html' || String(args.path).endsWith('/app.html');
                        if (isCanvas && lastContent) {
                            console.log('[Canvas/Agent/Browser] Firing traits-canvas-update, len:', lastContent.length);
                            window.dispatchEvent(new CustomEvent('traits-canvas-update', { detail: { content: lastContent } }));
                        }
                        toolResult = '{"ok":true}';
                    }
                }
                messages.push({ role: 'tool', tool_call_id: tc.id, content: toolResult });
            }
            if (choice?.finish_reason === 'stop') break;
        }
        return JSON.stringify(lastContent ? { ok: true, response: 'Canvas updated' } : { ok: false, error: 'No canvas content written' });
    } catch(e) {
        console.error('[Canvas/Agent/Browser] ✗', e);
        return JSON.stringify({ error: e.message || String(e) });
    }
}

function _dispatchVoiceEvent(type, data) {
    if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('voice-event', { detail: { type, ...data } }));
    }
}

// ── Local voice helpers (WebGPU STT + TTS) ──

function _localVoiceProgress(text) {
    if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('local-voice-progress', { detail: text }));
    }
}

async function _ensureTransformers() {
    if (_transformersLib) return _transformersLib;
    _localVoiceProgress('Loading Transformers.js…');
    _transformersLib = await import('https://cdn.jsdelivr.net/npm/@huggingface/transformers');
    return _transformersLib;
}

async function _ensureSTT() {
    if (_sttPipeline) return _sttPipeline;
    if (_sttLoading) return _sttLoading;
    _sttLoading = (async () => {
        try {
            const { pipeline } = await _ensureTransformers();
            _localVoiceProgress('Loading Whisper STT model (first run downloads ~150 MB)…');
            _sttPipeline = await pipeline('automatic-speech-recognition', 'onnx-community/whisper-base', {
                device: 'wasm',
                dtype: { encoder_model: 'fp32', decoder_model_merged: 'q4' },
                progress_callback: (p) => {
                    const status = p.status || '';
                    if (p.progress != null) {
                        _localVoiceProgress(`STT: ${status} ${Math.round(p.progress)}%`);
                    }
                }
            });
            _localVoiceProgress('STT model ready.');
            return _sttPipeline;
        } catch (e) {
            _sttPipeline = null;
            throw e;
        }
    })();
    try { return await _sttLoading; } finally { _sttLoading = null; }
}

async function _ensureTTS() {
    if (_ttsModel) return _ttsModel;
    if (_ttsLoading) return _ttsLoading;
    _ttsLoading = (async () => {
        try {
            _localVoiceProgress('Loading Kokoro TTS model (first run downloads ~92 MB)…');
            const kokoro = await import('https://cdn.jsdelivr.net/npm/kokoro-js@1.2.1');
            _ttsModel = await kokoro.KokoroTTS.from_pretrained('onnx-community/Kokoro-82M-v1.0-ONNX', {
                dtype: 'q8',
                device: 'wasm',
                progress_callback: (p) => {
                    const status = p.status || '';
                    if (p.progress != null) {
                        _localVoiceProgress(`TTS: ${status} ${Math.round(p.progress)}%`);
                    }
                }
            });
            _localVoiceProgress('TTS model ready.');
            return _ttsModel;
        } catch (e) {
            _ttsModel = null;
            throw e;
        }
    })();
    try { return await _ttsLoading; } finally { _ttsLoading = null; }
}

/**
 * Load Voxtral-Mini-4B-Realtime processor + model (cached after first load).
 * Uses @huggingface/transformers 4.x and onnx-community/Voxtral-Mini-4B-Realtime-2602-ONNX.
 * @param {Function} [onProgress] - Optional progress callback(message)
 * @returns {Promise<{processor, model, TextStreamer}>}
 */
async function _ensureVoxtral(onProgress) {
    if (_voxtralProcessor && _voxtralModel) {
        return { processor: _voxtralProcessor, model: _voxtralModel, TextStreamer: _voxtralLib4.TextStreamer };
    }
    if (_voxtralModelLoading) return _voxtralModelLoading;
    _voxtralModelLoading = (async () => {
        try {
            const MODEL_ID = 'onnx-community/Voxtral-Mini-4B-Realtime-2602-ONNX';
            const DTYPE = 'q4';
            const LIB_CDN = 'https://cdn.jsdelivr.net/npm/@huggingface/transformers@4.0.0-next.7';
            if (!_voxtralLib4) {
                const msg = 'Loading Transformers.js 4.x for Voxtral…';
                _localVoiceProgress(msg);
                if (onProgress) onProgress(msg);
                _voxtralLib4 = await import(LIB_CDN);
                // ORT-Web needs to know where its WASM binaries live. The transformers
                // v4 dist only ships the worker script (.mjs), NOT the .wasm binary.
                // The actual binary (ort-wasm-simd-threaded.*.wasm, ~24 MB) lives in the
                // onnxruntime-web package. Point wasmPaths there so ORT finds both the
                // .mjs worker script AND its companion .wasm binary from the same origin.
                // numThreads=1: GitHub Pages lacks COOP/COEP → no SharedArrayBuffer.
                // proxy=false: run WASM on main thread, no Worker proxy needed.
                const ORT_VERSION = '1.25.0-dev.20260307-d626b568e0'; // must match onnxruntime-web dep in transformers@4.0.0-next.7
                const ORT_CDN = `https://cdn.jsdelivr.net/npm/onnxruntime-web@${ORT_VERSION}/dist/`;
                try {
                    const env = _voxtralLib4.env;
                    if (env?.backends?.onnx?.wasm) {
                        env.backends.onnx.wasm.numThreads = 1;
                        env.backends.onnx.wasm.proxy = false;
                        env.backends.onnx.wasm.wasmPaths = ORT_CDN;
                    }
                } catch (_) { /* ignore — env not critical */ }
            }
            const { VoxtralRealtimeProcessor, VoxtralRealtimeForConditionalGeneration, TextStreamer } = _voxtralLib4;
            const mkProg = (label, startPct, span) => (info) => {
                if (!info || typeof info !== 'object') return;
                if (info.status === 'progress' && typeof info.progress === 'number') {
                    const pct = Math.round(startPct + (info.progress / 100) * span);
                    const file = info.file ? ' ' + info.file.split('/').pop() : '';
                    const msg = `${label}: ${pct}%${file}`;
                    _localVoiceProgress(msg);
                    if (onProgress) onProgress(msg);
                }
            };
            _localVoiceProgress('Loading Voxtral processor…');
            if (onProgress) onProgress('Loading Voxtral processor…');
            _voxtralProcessor = await VoxtralRealtimeProcessor.from_pretrained(MODEL_ID, {
                progress_callback: mkProg('Processor', 0, 15),
            });
            // ORT-Web registers the WebGPU backend globally at initialization time.
            // In transformers.js@4.0.0-next.7 the WebGPU backend calls webgpuInit which
            // is undefined on many browsers (Safari, some Chrome configs), causing
            // "no available backend found" even when we request device:'wasm'.
            // Valid devices for this version: 'webgpu' and 'wasm' only.
            const tryDevices = ['wasm'];
            let lastErr;
            for (const device of tryDevices) {
                try {
                    const modelMsg = `Loading Voxtral model (~1.5 GB, first run only, device: ${device})…`;
                    _localVoiceProgress(modelMsg);
                    if (onProgress) onProgress(modelMsg);
                    _voxtralModel = await VoxtralRealtimeForConditionalGeneration.from_pretrained(MODEL_ID, {
                        // embed_tokens q4 uses GatherBlockQuantized op — only supported in WebGPU/JSEP,
                        // not in WASM CPU EP. Use float32 for embed_tokens to get standard Gather op.
                        dtype: { audio_encoder: DTYPE, embed_tokens: 'float32', decoder_model_merged: DTYPE },
                        device,
                        progress_callback: mkProg('Model', 15, 83),
                    });
                    console.log(`[Voxtral] Loaded on device: ${device}`);
                    break; // success
                } catch (e) {
                    lastErr = e;
                    console.warn(`[Voxtral] Device "${device}" failed:`, e.message);
                    _voxtralModel = null;
                }
            }
            if (!_voxtralModel) throw lastErr;
            _localVoiceProgress('Voxtral ready.');
            if (onProgress) onProgress('Voxtral ready.');
            return { processor: _voxtralProcessor, model: _voxtralModel, TextStreamer };
        } catch (e) {
            _voxtralProcessor = null;
            _voxtralModel = null;
            throw e;
        }
    })();
    try { return await _voxtralModelLoading; } finally { _voxtralModelLoading = null; }
}

function _resampleTo16kHz(audioData, sampleRate) {
    if (sampleRate === 16000) return audioData;
    const ratio = sampleRate / 16000;
    const newLen = Math.round(audioData.length / ratio);
    const result = new Float32Array(newLen);
    for (let i = 0; i < newLen; i++) {
        const srcIdx = i * ratio;
        const lo = Math.floor(srcIdx);
        const hi = Math.min(lo + 1, audioData.length - 1);
        const frac = srcIdx - lo;
        result[i] = audioData[lo] * (1 - frac) + audioData[hi] * frac;
    }
    return result;
}

function _mergeFloat32Arrays(arrays) {
    const totalLen = arrays.reduce((s, a) => s + a.length, 0);
    const result = new Float32Array(totalLen);
    let offset = 0;
    for (const arr of arrays) {
        result.set(arr, offset);
        offset += arr.length;
    }
    return result;
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
// Tools allowed on the canvas page — everything else is stripped
const CANVAS_PAGE_TOOLS = new Set(['canvas', 'sys_echo', 'sys_audio', 'sys_voice_quit']);

async function _buildVoiceTools(sdk, page) {
    let traits = [];
    try { traits = await sdk.list(); } catch(e) { return []; }
    const isCanvas = page === 'canvas';
    const tools = [];
    if (!isCanvas) {
      for (const t of traits) {
        if (!t.path) continue;
        if (VOICE_TOOL_EXCLUDE.has(t.path)) continue;
        if (t.path.startsWith('www.')) continue;
        const kind = (t.source || t.kind || '').toLowerCase();
        if (kind === 'library' || kind === 'interface') continue;

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
    }
    // Always include the synthetic quit tool so the model can end the session
    tools.push({
        type: 'function',
        name: 'sys_voice_quit',
        description: 'End the voice conversation. Call this when the user says goodbye, wants to stop, or asks to quit.',
        parameters: { type: 'object', properties: {} }
    });

    // Synthetic canvas tool — simple wrapper around llm.agent
    tools.push({
        type: 'function',
        name: 'canvas',
        description: 'Draw, create, or change anything on the visual canvas. Just describe what you want in plain language. Examples: "draw a bouncing ball", "make it yellow", "add a reset button", "create a Spotify controller".',
        parameters: {
            type: 'object',
            properties: {
                request: {
                    type: 'string',
                    description: 'What to draw, create, or change on the canvas. Use the user\'s exact words.'
                }
            },
            required: ['request']
        }
    });

    // On canvas page, also include echo and audio, strip everything else
    if (isCanvas) {
        for (const t of traits) {
            if (!t.path) continue;
            const toolName = t.path.replace(/\./g, '_');
            if (toolName === 'sys_echo' || toolName === 'sys_audio') {
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
        }
    }

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
            // Handle unsolicited messages from worker (no matching id)
            if (msg._type === 'canvas-sync') {
                if (typeof window !== 'undefined') {
                    window.dispatchEvent(new CustomEvent('traits-canvas-update', { detail: { content: msg.content } }));
                }
                return;
            }
            if (msg._type === 'pvfs-sync') {
                // Worker sent VFS dump — persist to localStorage (Workers can't do this)
                try {
                    console.log('[pvfs-sync] received from worker, len=' + (msg.json || '').length);
                    localStorage.setItem('traits.pvfs', msg.json || '{}');
                    // Also refresh main-thread WASM VFS if available
                    if (wasm && wasm.pvfs_refresh) { try { wasm.pvfs_refresh(); } catch(e) {} }
                } catch(e) { console.warn('[pvfs-sync] localStorage write failed:', e); }
                return;
            }
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
        // Push localStorage secrets into the new worker's WASM secret store
        try {
            const PREFIX = 'traits.secret.';
            for (let i = 0; i < localStorage.length; i++) {
                const k = localStorage.key(i);
                if (k && k.startsWith(PREFIX)) {
                    const key = k.slice(PREFIX.length).toLowerCase();
                    const val = (localStorage.getItem(k) || '').trim();
                    if (val) await this._rpcWorker(state, 'set_secret', { key, value: val });
                }
            }
        } catch(e) {}
        // Seed worker's persistent VFS from localStorage (Workers can't read localStorage)
        try {
            const pvfs = localStorage.getItem('traits.pvfs');
            if (pvfs) {
                console.log('[worker-init] seeding VFS, len=' + pvfs.length);
                await this._rpcWorker(state, 'pvfs_load', { json: pvfs });
            }
        } catch(e) { console.warn('[worker-init] VFS seed failed:', e); }
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

    _syncSecretsToWorkers() {
        try {
            const PREFIX = 'traits.secret.';
            const secrets = [];
            for (let i = 0; i < localStorage.length; i++) {
                const k = localStorage.key(i);
                if (k && k.startsWith(PREFIX)) {
                    const key = k.slice(PREFIX.length).toLowerCase();
                    const val = (localStorage.getItem(k) || '').trim();
                    if (val) secrets.push({ key, val });
                }
            }
            for (const state of this._workers) {
                for (const { key, val } of secrets) {
                    this._rpcWorker(state, 'set_secret', { key, value: val }).catch(() => {});
                }
            }
        } catch(e) {}
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

        // Path-level target hint (e.g. "sys.ps@wasm")
        const parsed = parseDispatchTarget(path);
        const cleanPath = parsed.cleanPath;
        const inlineTarget = parsed.target;

        const forced = (opts.force || inlineTarget || '').toLowerCase();
        const forceMode = forced === 'native' ? 'helper' : (forced || null);

        // 0. Binding resolution: redirect interface paths to bound implementations
        const bound = this._bindings.get(cleanPath);
        if (bound && bound !== cleanPath) {
            return this.call(bound, args, opts);
        }

        let remoteFailure = null;
        let wasmResult = null;

        // 1. Explicitly forced routes
        if (forceMode === 'wasm') {
            return this._callWasm(cleanPath, args);
        }

        if (forceMode === 'helper') {
            const r = await callHelper(cleanPath, args, opts);
            return r || { ok: false, error: 'Helper not connected', dispatch: 'helper' };
        }

        if (forceMode === 'relay') {
            const r = await callRelay(cleanPath, args);
            return r || { ok: false, error: 'Relay not connected', dispatch: 'relay' };
        }

        if (forceMode === 'rest') {
            return this._callRest(cleanPath, args, opts);
        }

        // 2. Default route: WASM-callable traits go local first (fast, offline-safe, works on
        // static hosting). Non-WASM-callable traits (browser.*, spotify.*, etc.) go to native
        // backends first so helper/relay/server can serve them from the browser.

        if (wasmReady && wasmCallableSet.has(cleanPath)) {
            wasmResult = this._callWasm(cleanPath, args);
            if (wasmResult.ok) {
                // Intercept WebLLM dispatch sentinel — route to JS-side WebLLM engine
                if (wasmResult.result && wasmResult.result.dispatch === 'webllm') {
                    return this._callWebLLM(wasmResult.result.prompt, wasmResult.result.model);
                }
                // Intercept OpenAI streaming dispatch sentinel — route to JS-side fetch() SSE
                if (wasmResult.result && wasmResult.result.dispatch === 'openai_stream') {
                    return this._callOpenAIStream(wasmResult.result.prompt, wasmResult.result.model);
                }
                return wasmResult;
            }
            // WASM failed — fall through to native backends
        }

        // 2a. Local helper (native) — primary path for non-WASM traits, fallback for WASM
        if (helperReady) {
            const t0 = performance.now();
            const result = await callHelper(cleanPath, args, opts);
            if (result && result.ok) {
                result.ms = Math.round((performance.now() - t0) * 10) / 10;
                this._syncCanvasFromRemote(cleanPath, args);
                return result;
            }
            if (result) {
                result.ms = Math.round((performance.now() - t0) * 10) / 10;
                remoteFailure = result;
            }
        }

        // 2b. Relay (remote native helper)
        if (_relayCode()) {
            const t0 = performance.now();
            const result = await callRelay(cleanPath, args);
            if (result && result.ok) {
                result.ms = Math.round((performance.now() - t0) * 10) / 10;
                this._syncCanvasFromRemote(cleanPath, args);
                return result;
            }
            if (result) {
                result.ms = Math.round((performance.now() - t0) * 10) / 10;
                remoteFailure = result;
            }
        }

        // 2c. Server REST
        if (this.server) {
            const result = await this._callRest(cleanPath, args, opts);
            if (result.ok) {
                this._syncCanvasFromRemote(cleanPath, args);
                return result;
            }
            remoteFailure = result;
        }

        // 3. Nothing succeeded
        if (wasmResult) return wasmResult;
        if (remoteFailure) return remoteFailure;
        return { ok: false, error: `No dispatch path for '${cleanPath}'`, dispatch: 'none' };
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
        const parsed = parseDispatchTarget(path);
        const forced = parsed.target;
        if (forced) return forced === 'native' ? 'helper' : forced;
        // Mirror call() logic: WASM-callable traits go local first
        if (wasmReady && wasmCallableSet.has(parsed.cleanPath)) return 'wasm';
        if (helperReady) return 'helper';
        if (_relayCode()) return 'relay';
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
        // Populate _wasmInfo so status.traits / status.callable / status.version are correct
        try { this._wasmInfo = JSON.parse(mod.init()); } catch(_) {
            this._wasmInfo = { traits_registered: callable.length, wasm_callable: callable.length, version: null };
        }
        syncHelperToWasm();
        this._syncHelperToWorkers();
        // Inject localStorage secrets into WASM in-memory store so sys.call can resolve them
        if (mod.set_secret) {
            try {
                const PREFIX = 'traits.secret.';
                for (let i = 0; i < localStorage.length; i++) {
                    const k = localStorage.key(i);
                    if (k && k.startsWith(PREFIX)) {
                        const secretName = k.slice(PREFIX.length).toLowerCase();
                        const val = (localStorage.getItem(k) || '').trim();
                        if (val) mod.set_secret(secretName, val);
                    }
                }
            } catch(e) {}
        }
        // Register WASM kernel as a service for sys.ps
        if (mod.register_task) {
            try { mod.register_task('wasm-kernel', 'WASM Kernel', 'service', Date.now(), `${callable.length} callable traits`); } catch(e) {}
        }
        this._trackTask('wasm-kernel', 'WASM Kernel', 'service', `${callable.length} callable traits`);
        // Push all localStorage secrets to any already-running workers
        this._syncSecretsToWorkers();
    }

    /**
     * Inject a secret into the WASM kernel's in-memory store so sys.call can resolve it.
     * Secrets are stored by key name (e.g. "openai_api_key").
     * @param {string} key   - Secret name (lowercase)
     * @param {string} value - Secret value
     */
    setSecret(key, value) {
        if (wasm && wasm.set_secret) {
            try { wasm.set_secret(key, value); } catch(e) {}
        }
        // Also push to all workers — they run in a separate thread and can't read localStorage
        for (const state of this._workers) {
            this._rpcWorker(state, 'set_secret', { key, value }).catch(() => {});
        }
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
     * @param {string} [opts.voice='shimmer'] - Voice: alloy, ash, ballad, coral, echo, sage, shimmer, verse, marin, cedar
     * @param {string} [opts.model='gpt-realtime-mini-2025-12-15'] - Realtime model
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

        const voice = opts.voice || 'shimmer';
        const model = opts.model || 'gpt-realtime-mini-2025-12-15';
        const enableTools = opts.tools !== false;
        _voiceSdk = this;

        try {
            const LS_VOICE_INSTRUCTIONS = 'traits.voice.instructions';
            const currentPage = (typeof location !== 'undefined' && location.hash || '').replace(/^#\/?/, '').split('/')[0] || '';

            // ── If caller passed custom instructions, inject them so sys.voice.instruct build picks them up ──
            if (opts.instructions) {
                try { await this.call('sys.voice.instruct', ['set', opts.instructions]); } catch(_) {}
            } else {
                // Mirror localStorage overrides into WASM so build action reads the right value
                const lsInstr = (() => { try { return (localStorage.getItem(LS_VOICE_INSTRUCTIONS) || '').trim(); } catch(_) { return ''; } })();
                if (lsInstr) { try { await this.call('sys.voice.instruct', ['set', lsInstr]); } catch(_) {} }
            }

            // ── Build full instructions via sys.voice.instruct build (single source of truth) ──
            // Includes: agent context + memory notes + chat history + voice instruct text
            let fullInstructions = '';
            try {
                const instrResult = await this.call('sys.voice.instruct', ['build', opts.agent || '', opts.sessionId || '']);
                fullInstructions = instrResult?.instructions || instrResult?.result?.instructions || '';
            } catch(_) {}
            if (!fullInstructions) {
                fullInstructions = 'You are a concise, helpful voice assistant powered by traits.build. Keep responses short and conversational. You have access to function-calling tools that execute locally via WebAssembly.';
            }
            // Canvas page prefix (visual context cue)
            if (currentPage === 'canvas') {
                const canvasPrefix = 'You are a canvas assistant. The user is on a visual canvas page. ' +
                    'For ANY creative or visual request, call the `canvas` tool with the user\'s words. ' +
                    'Do NOT ask clarifying questions. Do NOT explain how things work. Just call `canvas` and report what was done. ' +
                    'You also have `sys_echo` (show text on screen) and `sys_audio` (play sounds).';
                fullInstructions = canvasPrefix + '\n\n' + fullInstructions;
            }
            console.log('[Voice] Instructions loaded (' + fullInstructions.length + ' chars, source: sys.voice.instruct build)');

            // ── Build tool definitions via sys.voice.tools (shared registry, single source of truth) ──
            let tools = [];
            if (enableTools) {
                try {
                    const toolsResult = await this.call('sys.voice.tools', [currentPage]);
                    tools = toolsResult?.tools || toolsResult?.result?.tools || [];
                } catch(_) {
                    tools = await _buildVoiceTools(this, currentPage); // fallback if trait not yet compiled
                }
            }

            // ── Ephemeral token: browser WebRTC needs a short-lived token ──
            // Direct browser fetch to OpenAI (CORS: *) — fast, no dispatch cascade.
            // Avoids relay timeout (60s) when stale pairing code is in localStorage.
            let ephemeralKey = null;
            try {
                const resp = await fetch('https://api.openai.com/v1/realtime/client_secrets', {
                    method: 'POST',
                    headers: {
                        'Authorization': 'Bearer ' + apiKey,
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({
                        session: { type: 'realtime', model,
                                   audio: { output: { voice: voice } } }
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
                const fallbackInstructions = 'You are a concise, helpful voice assistant powered by traits.build. Keep responses short and conversational. You have access to function-calling tools that execute locally via WebAssembly.';

                // WebRTC session.update: instructions and tools are mutable after
                // session creation. type:'realtime' is required by the API.
                const sessionConfig = {
                    type: 'realtime',
                    instructions: fullInstructions || fallbackInstructions,
                    // Give the user more time to finish their thought before the model responds.
                    // silence_duration_ms: 1200ms (default ~500ms) — waits longer after speech stops.
                    // prefix_padding_ms: 400ms — more audio before speech counts as a turn start.
                    turn_detection: {
                        type: 'server_vad',
                        silence_duration_ms: 1200,
                        prefix_padding_ms: 400,
                        threshold: 0.5,
                    },
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
                        if (msg.transcript) {
                            _dispatchVoiceEvent('response', { text: msg.transcript.trim() });
                        }
                    }

                    // ── Function call — model wants to invoke a trait tool ──
                    else if (type === 'response.function_call_arguments.done') {
                        const callId = msg.call_id || '';
                        const funcName = msg.name || '';
                        const argsStr = msg.arguments || '{}';
                        try { console.log('[Voice] ⚡ Tool call:', funcName, JSON.parse(argsStr)); } catch(_) { console.log('[Voice] ⚡ Tool call:', funcName, argsStr); }
                        if (opts.onToolCall) opts.onToolCall(funcName, argsStr);
                        _dispatchVoiceEvent('tool_call', { name: funcName, arguments: argsStr });

                        // Handle synthetic canvas tool — route to shared _runCanvasAgent
                        if (funcName === 'canvas') {
                            let request = '';
                            try { request = JSON.parse(argsStr).request || argsStr; } catch(e) { request = argsStr; }
                            console.log('[Voice/Canvas] ▶ Canvas tool triggered, launching agent for request:', request);

                            // Immediately resolve the tool call so the model speaks now
                            // rather than waiting silently for the full agent to finish
                            if (_voiceDc && _voiceDc.readyState === 'open') {
                                _voiceDc.send(JSON.stringify({ type: 'conversation.item.create', item: { type: 'function_call_output', call_id: callId, output: '{"status":"building","message":"Working on it now, give me a moment!"}' } }));
                                _voiceDc.send(JSON.stringify({ type: 'response.create' }));
                            }

                            _runCanvasAgent(this, request).then(truncated => {
                                console.log('[Voice/Canvas] ✓ Agent finished, injecting completion message');
                                // Inject a user message so the model knows the canvas is ready and speaks again
                                if (_voiceDc && _voiceDc.readyState === 'open') {
                                    _voiceDc.send(JSON.stringify({ type: 'conversation.item.create', item: { type: 'message', role: 'user', content: [{ type: 'input_text', text: 'Canvas update is ready.' }] } }));
                                    _voiceDc.send(JSON.stringify({ type: 'response.create' }));
                                }
                                if (opts.onToolResult) opts.onToolResult(funcName, truncated);
                                _dispatchVoiceEvent('tool_result', { name: funcName, result: truncated });
                            }).catch(e => {
                                console.error('[Voice/Canvas] ✗ _runCanvasAgent rejected:', e);
                            });
                            return;
                        }

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
                            console.log('[Voice] ✓ Result:', funcName, output.length > 400 ? output.slice(0, 400) + '…' : output);
                            if (_voiceDc && _voiceDc.readyState === 'open') {
                                _voiceDc.send(JSON.stringify({
                                    type: 'conversation.item.create',
                                    item: { type: 'function_call_output', call_id: callId, output: truncated }
                                }));
                                _voiceDc.send(JSON.stringify({ type: 'response.create' }));
                            }
                            if (opts.onToolResult) opts.onToolResult(funcName, truncated);
                            _dispatchVoiceEvent('tool_result', { name: funcName, result: truncated });

                            // After sys.canvas changes: fire live update event for canvas page
                            if (funcName === 'sys_canvas' && result.ok) {
                                const r = result.result || result;
                                if (r.action === 'set' || r.action === 'append' || r.action === 'clear') {
                                    this.call('sys.canvas', ['get']).then(getRes => {
                                        const content = getRes?.result?.content ?? getRes?.content ?? '';
                                        this._lastCanvasContent = content; // keep snapshot fresh for llm_agent check
                                        window.dispatchEvent(new CustomEvent('traits-canvas-update', { detail: { content } }));
                                    }).catch(() => {
                                        window.dispatchEvent(new CustomEvent('traits-canvas-update', {}));
                                    });
                                }
                                // Canvas project actions: fire event for project bridge
                                if (r.canvas_project_action) {
                                    window.dispatchEvent(new CustomEvent('traits-canvas-project', { detail: r }));
                                }
                            }

                            // After llm.agent calls: agent may have modified canvas via sys.canvas set.
                            // Only sync if canvas content actually changed — read-before-call snapshot used.
                            if (funcName === 'llm_agent' && result.ok) {
                                // The sys_canvas after-hook covers direct sys.canvas calls; the 1-sec poller
                                // in canvas.rs covers indirect writes via WASM pvfs. No unconditional read
                                // here to avoid reloading the iframe every time the agent runs a tool.
                                const _prevCanvas = this._lastCanvasContent ?? null;
                                this.call('sys.canvas', ['get']).then(getRes => {
                                    const content = getRes?.result?.content ?? getRes?.content ?? '';
                                    if (content && content !== _prevCanvas) {
                                        this._lastCanvasContent = content;
                                        window.dispatchEvent(new CustomEvent('traits-canvas-update', { detail: { content } }));
                                    }
                                }).catch(() => {});
                            }

                            // After sys.voice.instruct changes: persist to localStorage + live session.update
                            if (funcName === 'sys_voice_instruct' && result.ok) {
                                const r = result.result || result;
                                if (r.action === 'set' || r.action === 'append' || r.action === 'reset') {
                                    this.call('sys.voice.instruct', ['get']).then(getRes => {
                                        const updated = getRes?.result?.instructions || getRes?.instructions || '';
                                        if (updated) {
                                            try { localStorage.setItem(LS_VOICE_INSTRUCTIONS, updated); } catch(_) {}
                                            // Live-update the running voice session
                                            if (_voiceDc && _voiceDc.readyState === 'open') {
                                                _voiceDc.send(JSON.stringify({
                                                    type: 'session.update',
                                                    session: { type: 'realtime', instructions: updated }
                                                }));
                                            }
                                        } else if (r.action === 'reset') {
                                            try { localStorage.removeItem(LS_VOICE_INSTRUCTIONS); } catch(_) {}
                                        }
                                    }).catch(() => {});
                                }
                            }

                            // After sys.spa actions: fire event for SPA bridge to execute
                            if (funcName === 'sys_spa' && result.ok) {
                                const r = result.result || result;
                                if (r.spa_action) {
                                    window.dispatchEvent(new CustomEvent('traits-spa-action', { detail: r }));
                                }
                            }

                            // After sys.voice.mode actions: fire event for voice mode bridge
                            if (funcName === 'sys_voice_mode' && result.ok) {
                                const r = result.result || result;
                                if (r.voice_mode_action) {
                                    window.dispatchEvent(new CustomEvent('traits-voice-mode', { detail: r }));
                                }
                            }

                            // After sys.audio actions: fire event for WebAudio bridge
                            if (funcName === 'sys_audio' && result.ok) {
                                const r = result.result || result;
                                if (r.audio_action) {
                                    window.dispatchEvent(new CustomEvent('traits-audio-action', { detail: r }));
                                }
                            }
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
     * Send a typed text message to the active voice model.
     * @param {string} text - The text to send as a user message.
     * @returns {boolean} true if sent, false if voice is not active.
     */
    sendVoiceText(text) {
        if (!_voiceDc || _voiceDc.readyState !== 'open') return false;
        _voiceDc.send(JSON.stringify({
            type: 'conversation.item.create',
            item: { type: 'message', role: 'user', content: [{ type: 'input_text', text }] }
        }));
        _voiceDc.send(JSON.stringify({ type: 'response.create' }));
        return true;
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

    // ── Local Voice (WebGPU STT + LLM + TTS) ──

    /**
     * Start a fully local voice session using WebGPU-accelerated models:
     *   Mic → Whisper STT → llm/prompt interface → Kokoro TTS → Speaker
     *
     * The LLM step dispatches through this.call('llm.prompt', ...) so it
     * uses whatever implementation is currently bound to the llm/prompt
     * interface (e.g. llm.prompt.webllm for fully-local WebGPU inference).
     *
     * @param {Object} opts
     * @param {string} [opts.voice='af_heart'] - Kokoro TTS voice ID
     * @param {string} [opts.language='en'] - STT language code
     * @param {string} [opts.instructions] - System prompt for the LLM
     * @param {boolean} [opts.tools=true] - Enable function calling with trait tools
     * @param {Function} [opts.onTranscript] - Called with user speech transcription
     * @param {Function} [opts.onResponse] - Called with LLM response text
     * @param {Function} [opts.onToolCall] - Called when model invokes a tool: (name, args) => void
     * @param {Function} [opts.onToolResult] - Called with tool result: (name, result) => void
     * @param {Function} [opts.onProgress] - Called with model loading progress
     * @param {Function} [opts.onError] - Called on error
     * @returns {Promise<{ok: boolean, error?: string, mode?: string, voice?: string, tools?: number}>}
     */
    async startLocalVoice(opts = {}) {
        await this.stopLocalVoice();

        const voice = opts.voice || 'af_heart';
        const language = opts.language || 'en';

        // Check WebGPU
        if (typeof navigator === 'undefined' || !navigator.gpu) {
            return { ok: false, error: 'WebGPU not supported. Local voice requires Chrome 113+ or Edge 113+.' };
        }
        const adapter = await navigator.gpu.requestAdapter();
        if (!adapter) {
            return { ok: false, error: 'No WebGPU adapter found. Check GPU drivers.' };
        }

        _localVoiceActive = true;
        const enableTools = opts.tools !== false;
        _dispatchVoiceEvent('started', { voice, mode: 'local' });

        // Forward progress events to callback
        const progressHandler = (e) => {
            if (opts.onProgress) opts.onProgress(e.detail);
        };
        window.addEventListener('local-voice-progress', progressHandler);

        try {
            // ── Load voice instructions (same sources as cloud voice) ──
            let voiceInstructions = opts.instructions || '';
            if (!voiceInstructions) {
                try { voiceInstructions = (localStorage.getItem('traits.voice.instructions') || '').trim(); } catch(_) {}
            }
            if (!voiceInstructions) {
                try {
                    if (wasm && wasm.vfs_read) {
                        const content = wasm.vfs_read('traits/sys/voice/realtime_instructions.md');
                        if (content) voiceInstructions = content;
                    }
                } catch(_) {}
            }
            if (!voiceInstructions) {
                voiceInstructions = 'You are a concise, helpful voice assistant. Keep responses short and conversational. You have access to function-calling tools that execute locally.';
            }

            // ── Build tool definitions via sys.voice.tools ──
            let tools = [];
            if (enableTools) {
                _localVoiceProgress('Loading tools…');
                const _localPage = (typeof location !== 'undefined' && location.hash || '').replace(/^#\/?/, '').split('/')[0] || '';
                try {
                    const toolsResult = await this.call('sys.voice.tools', [_localPage]);
                    tools = toolsResult?.tools || toolsResult?.result?.tools || [];
                } catch(_) {
                    tools = await _buildVoiceTools(this, _localPage); // fallback
                }
                console.log('[LocalVoice] Loaded', tools.length, 'tools');
            }

            // Load STT and TTS models in parallel (first run downloads weights)
            _localVoiceProgress('Initializing local voice models…');
            if (opts.onProgress) opts.onProgress('Initializing local voice models…');

            const [stt, tts] = await Promise.all([_ensureSTT(), _ensureTTS()]);
            if (!_localVoiceActive) return { ok: false, error: 'Cancelled' };

            // Auto-bind llm/prompt to WebLLM if no binding exists
            if (!this._bindings.has('llm/prompt')) {
                this.bind('llm/prompt', 'llm.prompt.webllm');
                console.log('[LocalVoice] Auto-bound llm/prompt → llm.prompt.webllm');
            }

            // Preload WebLLM engine so first LLM call is fast
            const llmBinding = this._bindings.get('llm/prompt') || '';
            if (llmBinding === 'llm.prompt.webllm') {
                _localVoiceProgress('Pre-loading LLM model…');
                if (opts.onProgress) opts.onProgress('Pre-loading LLM model…');
                try { await _ensureWebLLM(); } catch(e) {
                    console.warn('[LocalVoice] WebLLM preload failed:', e.message);
                }
            }

            if (!_localVoiceActive) return { ok: false, error: 'Cancelled' };

            // Get microphone
            _localVoiceStream = await navigator.mediaDevices.getUserMedia({
                audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true }
            });

            _localVoiceAudioCtx = new AudioContext();
            const source = _localVoiceAudioCtx.createMediaStreamSource(_localVoiceStream);

            // ScriptProcessorNode for continuous audio capture + silence detection
            _localVoiceProcessor = _localVoiceAudioCtx.createScriptProcessor(4096, 1, 1);
            source.connect(_localVoiceProcessor);
            _localVoiceProcessor.connect(_localVoiceAudioCtx.destination);

            const history = [];    // conversation history: [{role, content}]
            let audioBuffer = [];  // accumulated Float32 audio chunks
            let speechStart = 0;   // timestamp when speech began
            let silenceStart = 0;  // timestamp when silence began
            let processing = false; // true during STT→LLM→TTS pipeline
            const SILENCE_THRESHOLD = 0.015;
            const SILENCE_TIMEOUT = 1200; // ms of silence to trigger end-of-speech
            const MIN_SPEECH = 500;       // ms minimum speech duration
            const sdk = this;

            const processTurn = async (pcm) => {
                processing = true;
                try {
                    // 1. STT — Whisper transcription
                    _localVoiceProgress('Transcribing…');
                    _dispatchVoiceEvent('listening', { active: false });
                    const resampled = _resampleTo16kHz(pcm, _localVoiceAudioCtx.sampleRate);

                    // Skip very short audio (< 0.5s at 16kHz)
                    if (resampled.length < 8000) { processing = false; return; }

                    const sttResult = await stt(resampled, { language });
                    const transcript = (sttResult.text || '').trim();

                    if (!transcript || !_localVoiceActive) { processing = false; return; }
                    // Filter hallucinated silence transcripts from Whisper
                    const lower = transcript.toLowerCase();
                    if (lower === 'you' || lower === 'thank you.' || lower === 'thanks for watching!' || lower.length < 3) {
                        processing = false;
                        _dispatchVoiceEvent('listening', { active: true });
                        return;
                    }

                    if (opts.onTranscript) opts.onTranscript(transcript);
                    _dispatchVoiceEvent('transcript', { text: transcript });

                    // 2. LLM — generate response via WebLLM with proper chat messages
                    _localVoiceProgress('Thinking…');
                    history.push({ role: 'user', content: transcript });

                    // Build proper chat messages array for WebLLM
                    const messages = [
                        { role: 'system', content: voiceInstructions },
                        ...history,
                    ];

                    // Convert Realtime API tool format to Chat Completions format for WebLLM
                    // Realtime: { type:'function', name, description, parameters }
                    // Chat:     { type:'function', function: { name, description, parameters } }
                    const chatTools = tools.length > 0 ? tools.map(t => ({
                        type: 'function',
                        function: { name: t.name, description: t.description || '', parameters: t.parameters || {} },
                    })) : [];
                    const llmOpts = chatTools.length > 0 ? { tools: chatTools } : undefined;
                    let llmResult = await sdk._callWebLLM(messages, null, llmOpts);

                    // If tool-enabled call failed, retry without tools (model may not support them)
                    if (!llmResult.ok && llmOpts) {
                        console.warn('[LocalVoice] Retrying without tools:', llmResult.error);
                        llmResult = await sdk._callWebLLM(messages);
                    }

                    // ── Tool call loop — execute tools until model gives a text response ──
                    let toolRounds = 0;
                    const MAX_TOOL_ROUNDS = 5;
                    while (llmResult.ok && llmResult.tool_calls && llmResult.tool_calls.length > 0 && toolRounds < MAX_TOOL_ROUNDS) {
                        toolRounds++;
                        // Add assistant message with tool_calls to history
                        history.push({ role: 'assistant', content: llmResult.result || null, tool_calls: llmResult.tool_calls });

                        for (const tc of llmResult.tool_calls) {
                            const funcName = tc.function?.name || '';
                            const argsStr = tc.function?.arguments || '{}';
                            const toolCallId = tc.id || `call_${Date.now()}`;

                            if (opts.onToolCall) opts.onToolCall(funcName, argsStr);
                            _dispatchVoiceEvent('tool_call', { name: funcName, arguments: argsStr });

                            // Handle quit tool
                            if (funcName === 'sys_voice_quit') {
                                history.push({ role: 'tool', tool_call_id: toolCallId, content: '{"ok":true,"action":"quit"}' });
                                sdk.stopLocalVoice();
                                return;
                            }

                            // Handle canvas tool — routes to llm.agent via shared helper
                            if (funcName === 'canvas') {
                                let request = '';
                                try { request = JSON.parse(argsStr).request || argsStr; } catch(e) { request = argsStr; }
                                _localVoiceProgress('Building canvas…');
                                const canvasOutput = await _runCanvasAgent(sdk, request);
                                if (opts.onToolResult) opts.onToolResult(funcName, canvasOutput);
                                _dispatchVoiceEvent('tool_result', { name: funcName, result: canvasOutput });
                                history.push({ role: 'tool', tool_call_id: toolCallId, content: canvasOutput });
                                continue;
                            }

                            // Dispatch tool via SDK cascade
                            _localVoiceProgress(`Running ${funcName.replace(/_/g, '.')}…`);
                            const traitPath = funcName.replace(/_/g, '.');
                            let callArgs = [];
                            try {
                                const parsed = JSON.parse(argsStr);
                                const traitInfo = await sdk.info(traitPath);
                                if (traitInfo && Array.isArray(traitInfo.params)) {
                                    callArgs = traitInfo.params.map(p => parsed[p.name] !== undefined ? parsed[p.name] : null);
                                } else {
                                    callArgs = Object.values(parsed);
                                }
                            } catch(_) {}

                            const toolResult = await sdk.call(traitPath, callArgs);
                            const output = JSON.stringify(toolResult.ok ? (toolResult.result !== undefined ? toolResult.result : toolResult) : { error: toolResult.error });
                            const truncated = output.length > 2000 ? output.slice(0, 2000) + '…(truncated)' : output;

                            if (opts.onToolResult) opts.onToolResult(funcName, truncated);
                            _dispatchVoiceEvent('tool_result', { name: funcName, result: truncated });

                            // After sys.spa actions: fire event for SPA bridge to execute
                            if (funcName === 'sys_spa' && toolResult.ok) {
                                const r = toolResult.result || toolResult;
                                if (r.spa_action) {
                                    window.dispatchEvent(new CustomEvent('traits-spa-action', { detail: r }));
                                }
                            }

                            // After sys.voice.mode actions: fire event for voice mode bridge
                            if (funcName === 'sys_voice_mode' && toolResult.ok) {
                                const r = toolResult.result || toolResult;
                                if (r.voice_mode_action) {
                                    window.dispatchEvent(new CustomEvent('traits-voice-mode', { detail: r }));
                                }
                            }

                            // After sys.audio actions: fire event for WebAudio bridge
                            if (funcName === 'sys_audio' && toolResult.ok) {
                                const r = toolResult.result || toolResult;
                                if (r.audio_action) {
                                    window.dispatchEvent(new CustomEvent('traits-audio-action', { detail: r }));
                                }
                            }

                            // Add tool response to history
                            history.push({ role: 'tool', tool_call_id: toolCallId, content: truncated });
                        }

                        // Re-call LLM with updated history (tool results)
                        _localVoiceProgress('Thinking…');
                        const followUpMessages = [
                            { role: 'system', content: voiceInstructions },
                            ...history,
                        ];
                        llmResult = await sdk._callWebLLM(followUpMessages, null, llmOpts);
                    }

                    let responseText = '';
                    if (llmResult.ok) {
                        responseText = typeof llmResult.result === 'string'
                            ? llmResult.result
                            : (llmResult.result?.content || JSON.stringify(llmResult.result));
                    } else {
                        console.error('[LocalVoice] LLM error:', llmResult.error);
                        responseText = 'Sorry, I could not generate a response.';
                    }
                    // Strip markdown artifacts
                    responseText = responseText.replace(/[*_`#]/g, '').replace(/\n+/g, ' ').trim();

                    if (!responseText || !_localVoiceActive) { processing = false; return; }

                    history.push({ role: 'assistant', content: responseText });
                    // Keep conversation window manageable
                    while (history.length > 20) history.shift();

                    if (opts.onResponse) opts.onResponse(responseText);
                    _dispatchVoiceEvent('response', { text: responseText });

                    // 3. TTS — synthesize speech with Kokoro
                    _localVoiceProgress('Speaking…');
                    _dispatchVoiceEvent('speaking', { text: responseText });

                    // Mute mic during TTS playback to prevent echo feedback
                    if (_localVoiceStream) {
                        _localVoiceStream.getAudioTracks().forEach(t => t.enabled = false);
                    }

                    try {
                        const audio = await tts.generate(responseText, { voice });
                        if (!_localVoiceActive) { processing = false; return; }

                        // Play synthesized audio
                        const blob = await audio.toBlob();
                        const audioUrl = URL.createObjectURL(blob);
                        const audioEl = new Audio(audioUrl);

                        await new Promise((resolve) => {
                            audioEl.onended = () => { URL.revokeObjectURL(audioUrl); resolve(); };
                            audioEl.onerror = () => { URL.revokeObjectURL(audioUrl); resolve(); };
                            audioEl.play().catch(resolve);
                        });
                    } finally {
                        // Unmute mic after playback
                        if (_localVoiceStream) {
                            _localVoiceStream.getAudioTracks().forEach(t => t.enabled = true);
                        }
                    }

                    _localVoiceProgress('');
                    _dispatchVoiceEvent('listening', { active: true });

                } catch(e) {
                    console.error('[LocalVoice] Turn error:', e);
                    if (opts.onError) opts.onError(e.message || String(e));
                    _dispatchVoiceEvent('error', { message: e.message || String(e) });
                }
                processing = false;
            };

            // Audio processing loop — captures PCM and detects silence
            _localVoiceProcessor.onaudioprocess = (e) => {
                if (!_localVoiceActive || processing) return;

                const input = e.inputBuffer.getChannelData(0);
                const rms = Math.sqrt(input.reduce((s, v) => s + v * v, 0) / input.length);

                if (rms > SILENCE_THRESHOLD) {
                    if (!speechStart) {
                        speechStart = Date.now();
                        _dispatchVoiceEvent('listening', { active: true, speaking: true });
                    }
                    silenceStart = 0;
                    audioBuffer.push(new Float32Array(input));
                } else if (speechStart) {
                    audioBuffer.push(new Float32Array(input)); // include trailing silence
                    if (!silenceStart) {
                        silenceStart = Date.now();
                    } else if (Date.now() - silenceStart > SILENCE_TIMEOUT
                               && Date.now() - speechStart > MIN_SPEECH) {
                        // Speech ended — capture and process
                        const pcm = _mergeFloat32Arrays(audioBuffer);
                        audioBuffer = [];
                        speechStart = 0;
                        silenceStart = 0;
                        processTurn(pcm);
                    }
                }
            };

            _localVoiceProgress('Listening…');
            if (opts.onProgress) opts.onProgress('Listening…');
            _dispatchVoiceEvent('listening', { active: true });

            return { ok: true, mode: 'local', voice, tools: tools.length };

        } catch(e) {
            await this.stopLocalVoice();
            return { ok: false, error: e.message || String(e) };
        } finally {
            window.removeEventListener('local-voice-progress', progressHandler);
        }
    }

    /**
     * Stop the local voice session.
     * @returns {Promise<void>}
     */
    async stopLocalVoice() {
        _localVoiceActive = false;
        if (_localVoiceProcessor) {
            try { _localVoiceProcessor.disconnect(); } catch(e) {}
            _localVoiceProcessor = null;
        }
        if (_localVoiceAudioCtx) {
            try { await _localVoiceAudioCtx.close(); } catch(e) {}
            _localVoiceAudioCtx = null;
        }
        if (_localVoiceStream) {
            _localVoiceStream.getTracks().forEach(t => t.stop());
            _localVoiceStream = null;
        }
        _localVoiceProgress('');
        _dispatchVoiceEvent('stopped', { mode: 'local' });
    }

    /**
     * Check if local voice session is active.
     * @returns {boolean}
     */
    isLocalVoiceActive() {
        return _localVoiceActive;
    }

    /**
     * Start Voxtral local-realtime voice session.
     * Uses Voxtral-Mini-4B-Realtime (ONNX/WebGPU) for STT + WebLLM for LLM response + Kokoro TTS.
     * Integrates full MCP tool-calling loop identical to startLocalVoice.
     * @param {Object} opts - voice, instructions, tools, onTranscript, onResponse, onToolCall, onToolResult, onProgress, onError
     */
    async startVoxtralVoice(opts = {}) {
        await this.stopVoxtralVoice();
        const voice = opts.voice || 'af_heart';

        if (typeof navigator === 'undefined' || !navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
            return { ok: false, error: 'getUserMedia not supported in this browser.' };
        }

        _voxtralVoiceActive = true;
        const enableTools = opts.tools !== false;
        _dispatchVoiceEvent('started', { voice, mode: 'local-realtime' });

        const progressHandler = (e) => {
            if (opts.onProgress) opts.onProgress(e.detail);
        };
        window.addEventListener('local-voice-progress', progressHandler);

        try {
            // ── Load voice instructions ──
            let voiceInstructions = opts.instructions || '';
            if (!voiceInstructions) {
                try { voiceInstructions = (localStorage.getItem('traits.voice.instructions') || '').trim(); } catch(_) {}
            }
            if (!voiceInstructions) {
                try {
                    if (wasm && wasm.vfs_read) {
                        const content = wasm.vfs_read('traits/sys/voice/realtime_instructions.md');
                        if (content) voiceInstructions = content;
                    }
                } catch(_) {}
            }
            if (!voiceInstructions) {
                voiceInstructions = 'You are a concise, helpful voice assistant. Keep responses short and conversational. You have access to function-calling tools that execute locally.';
            }

            // ── Load Voxtral (STT) + TTS in parallel ──
            _localVoiceProgress('Initializing Voxtral and TTS models…');
            if (opts.onProgress) opts.onProgress('Initializing Voxtral and TTS models…');
            const [voxtral, tts] = await Promise.all([
                _ensureVoxtral(opts.onProgress),
                _ensureTTS(),
            ]);
            if (!_voxtralVoiceActive) return { ok: false, error: 'Cancelled' };

            // ── Open mic at 16 kHz (Voxtral audio encoder requirement) ──
            _voxtralVoiceStream = await navigator.mediaDevices.getUserMedia({
                audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true }
            });
            _voxtralVoiceAudioCtx = new AudioContext({ sampleRate: 16000 });
            const source = _voxtralVoiceAudioCtx.createMediaStreamSource(_voxtralVoiceStream);
            _voxtralVoiceNode = _voxtralVoiceAudioCtx.createScriptProcessor(4096, 1, 1);
            source.connect(_voxtralVoiceNode);
            _voxtralVoiceNode.connect(_voxtralVoiceAudioCtx.destination);

            let audioBuffer = [];
            let speechStart = 0;
            let silenceStart = 0;
            let processing = false;
            const SILENCE_THRESHOLD = 0.015;
            const SILENCE_TIMEOUT = 1200;
            const MIN_SPEECH = 500;
            const sdk = this;

            const processTurn = async (pcm) => {
                processing = true;
                try {
                    // 1. STT — Voxtral transcription (audio already at 16 kHz, no resampling)
                    _localVoiceProgress('Transcribing with Voxtral…');
                    _dispatchVoiceEvent('listening', { active: false });

                    // Skip very short audio (< 0.5 s at 16 kHz)
                    if (pcm.length < 8000) { processing = false; return; }

                    let transcript = '';
                    const inputs = await voxtral.processor(null, pcm);
                    const streamer = new voxtral.TextStreamer(voxtral.processor.tokenizer, {
                        skip_special_tokens: true,
                        skip_prompt: true,
                        callback_function: (token) => { transcript += token; },
                    });
                    await voxtral.model.generate({ ...inputs, max_new_tokens: 512, streamer });
                    transcript = transcript.trim();

                    if (!transcript || !_voxtralVoiceActive) { processing = false; return; }
                    if (transcript.length < 3) {
                        processing = false;
                        _dispatchVoiceEvent('listening', { active: true });
                        return;
                    }

                    if (opts.onTranscript) opts.onTranscript(transcript);
                    _dispatchVoiceEvent('transcript', { text: transcript });

                    // 2. LLM — cloud model via llm.agent (OpenAI API + tool calling)
                    _localVoiceProgress('Thinking…');
                    const agentResult = await sdk.call('llm.agent', [transcript, voiceInstructions]);

                    // Surface tool call events from agent result
                    if (agentResult.ok && Array.isArray(agentResult.result?.tool_calls)) {
                        for (const tc of agentResult.result.tool_calls) {
                            const funcName = (tc.name || '').replace(/\./g, '_');
                            const argsStr = JSON.stringify(tc.args || {});
                            const resultStr = JSON.stringify(tc.result || {});
                            if (opts.onToolCall) opts.onToolCall(funcName, argsStr);
                            _dispatchVoiceEvent('tool_call', { name: funcName, arguments: argsStr });
                            if (opts.onToolResult) opts.onToolResult(funcName, resultStr);
                            _dispatchVoiceEvent('tool_result', { name: funcName, result: resultStr });
                            if (tc.name === 'sys.spa' && tc.result?.spa_action)
                                window.dispatchEvent(new CustomEvent('traits-spa-action', { detail: tc.result }));
                            if (tc.name === 'sys.voice.mode' && tc.result?.voice_mode_action)
                                window.dispatchEvent(new CustomEvent('traits-voice-mode', { detail: tc.result }));
                            if (tc.name === 'sys.audio' && tc.result?.audio_action)
                                window.dispatchEvent(new CustomEvent('traits-audio-action', { detail: tc.result }));
                        }
                    }

                    let responseText = '';
                    if (agentResult.ok && agentResult.result?.response) {
                        responseText = agentResult.result.response;
                    } else if (!agentResult.ok) {
                        console.error('[VoxtralVoice] Agent error:', agentResult.error);
                        responseText = 'Sorry, I could not generate a response.';
                    }
                    responseText = responseText.replace(/[*_`#]/g, '').replace(/\n+/g, ' ').trim();

                    if (!responseText || !_voxtralVoiceActive) { processing = false; return; }

                    if (opts.onResponse) opts.onResponse(responseText);
                    _dispatchVoiceEvent('response', { text: responseText });

                    // 3. TTS — Kokoro synthesis + playback
                    _localVoiceProgress('Speaking…');
                    _dispatchVoiceEvent('speaking', { text: responseText });

                    // Mute mic during TTS to prevent echo
                    if (_voxtralVoiceStream) {
                        _voxtralVoiceStream.getAudioTracks().forEach(t => t.enabled = false);
                    }
                    try {
                        const audio = await tts.generate(responseText, { voice });
                        if (!_voxtralVoiceActive) { processing = false; return; }
                        const blob = await audio.toBlob();
                        const audioUrl = URL.createObjectURL(blob);
                        const audioEl = new Audio(audioUrl);
                        await new Promise((resolve) => {
                            audioEl.onended = () => { URL.revokeObjectURL(audioUrl); resolve(); };
                            audioEl.onerror = () => { URL.revokeObjectURL(audioUrl); resolve(); };
                            audioEl.play().catch(resolve);
                        });
                    } finally {
                        if (_voxtralVoiceStream) {
                            _voxtralVoiceStream.getAudioTracks().forEach(t => t.enabled = true);
                        }
                    }

                    _localVoiceProgress('');
                    _dispatchVoiceEvent('listening', { active: true });
                } catch(e) {
                    console.error('[VoxtralVoice] Turn error:', e);
                    if (opts.onError) opts.onError(e.message || String(e));
                    _dispatchVoiceEvent('error', { message: e.message || String(e) });
                }
                processing = false;
            };

            // Audio capture + VAD loop
            _voxtralVoiceNode.onaudioprocess = (e) => {
                if (!_voxtralVoiceActive || processing) return;
                const input = e.inputBuffer.getChannelData(0);
                const rms = Math.sqrt(input.reduce((s, v) => s + v * v, 0) / input.length);
                if (rms > SILENCE_THRESHOLD) {
                    if (!speechStart) {
                        speechStart = Date.now();
                        _dispatchVoiceEvent('listening', { active: true, speaking: true });
                    }
                    silenceStart = 0;
                    audioBuffer.push(new Float32Array(input));
                } else if (speechStart) {
                    audioBuffer.push(new Float32Array(input));
                    if (!silenceStart) {
                        silenceStart = Date.now();
                    } else if (Date.now() - silenceStart > SILENCE_TIMEOUT
                               && Date.now() - speechStart > MIN_SPEECH) {
                        const pcm = _mergeFloat32Arrays(audioBuffer);
                        audioBuffer = [];
                        speechStart = 0;
                        silenceStart = 0;
                        processTurn(pcm);
                    }
                }
            };

            _localVoiceProgress('Listening…');
            if (opts.onProgress) opts.onProgress('Listening…');
            _dispatchVoiceEvent('listening', { active: true });

            return { ok: true, mode: 'local-realtime', voice };

        } catch(e) {
            await this.stopVoxtralVoice();
            return { ok: false, error: e.message || String(e) };
        } finally {
            window.removeEventListener('local-voice-progress', progressHandler);
        }
    }

    /**
     * Stop the Voxtral local-realtime voice session.
     */
    async stopVoxtralVoice() {
        _voxtralVoiceActive = false;
        if (_voxtralVoiceNode) {
            try { _voxtralVoiceNode.disconnect(); } catch(e) {}
            _voxtralVoiceNode = null;
        }
        if (_voxtralVoiceAudioCtx) {
            try { await _voxtralVoiceAudioCtx.close(); } catch(e) {}
            _voxtralVoiceAudioCtx = null;
        }
        if (_voxtralVoiceStream) {
            _voxtralVoiceStream.getTracks().forEach(t => t.stop());
            _voxtralVoiceStream = null;
        }
        _localVoiceProgress('');
        _dispatchVoiceEvent('stopped', { mode: 'local-realtime' });
    }

    /**
     * Check if Voxtral voice session is active.
     * @returns {boolean}
     */
    isVoxtralVoiceActive() {
        return _voxtralVoiceActive;
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

    /**
     * After a remote (helper/relay/REST) call that may have written canvas files,
     * sync canvas/app.html back from the remote VFS → WASM VFS → fire DOM event.
     * Fire-and-forget: never blocks the caller.
     */
    _syncCanvasFromRemote(path, args) {
        const relevant = path === 'llm.agent' ||
            (path === 'sys.vfs' && args?.[0] === 'write' && String(args?.[1] || '').startsWith('canvas/')) ||
            (path === 'sys.canvas');
        if (!relevant || typeof window === 'undefined') return;

        console.log('[canvas-sync] triggered by', path, 'args[0]:', args?.[0], 'helperReady:', helperReady);

        const readRemote = helperReady
            ? callHelper('sys.vfs', ['read', 'canvas/app.html'])
            : _relayCode()
                ? callRelay('sys.vfs', ['read', 'canvas/app.html'])
                : Promise.resolve(null);

        readRemote.then(r => {
            const content = r?.result?.content ?? r?.content ?? null;
            console.log('[canvas-sync] read result:', r?.ok, 'content len:', content?.length ?? 'null');
            if (content === null) return;
            // Mirror to WASM VFS so local reads stay consistent
            if (wasmReady) {
                try { wasm.call('sys.vfs', JSON.stringify(['write', 'canvas/app.html', content])); } catch(_) {}
            }
            window.dispatchEvent(new CustomEvent('traits-canvas-update', { detail: { content } }));
        }).catch((e) => { console.warn('[canvas-sync] error:', e); });
    }

    _callWasm(path, args) {
        const t0 = performance.now();
        try {
            // Refresh main-thread persistent VFS from localStorage before VFS/canvas reads.
            // The Worker WASM writes to localStorage but the main-thread in-memory VFS is stale.
            if ((path === 'sys.vfs' || path === 'sys.canvas') && wasm.pvfs_refresh) {
                try { wasm.pvfs_refresh(); } catch (_) {}
            }
            const raw = wasm.call(path, JSON.stringify(args));
            const dt = performance.now() - t0;
            const result = JSON.parse(raw);
            return { ok: true, result, dispatch: 'wasm', ms: Math.round(dt * 10) / 10 };
        } catch (e) {
            return { ok: false, error: e.message || String(e), dispatch: 'wasm' };
        }
    }

    async _callWebLLM(promptOrMessages, model, onTokenOrOpts) {
        // 5 minutes for first-time model download (~1.7 GB), subsequent calls are fast
        const TIMEOUT_MS = 300_000;
        const t0 = performance.now();
        _lastWebLLMStep = '';
        let aborted = false;
        // Accept either onToken function or opts object with {onToken, tools}
        const onToken = typeof onTokenOrOpts === 'function' ? onTokenOrOpts : onTokenOrOpts?.onToken;
        const tools = (typeof onTokenOrOpts === 'object' && onTokenOrOpts?.tools) || undefined;
        try {
            const result = await Promise.race([
                (async () => {
                    const engine = await _ensureWebLLM(model);
                    _webllmProgress('Running inference…');
                    // Accept either a string prompt or a messages array
                    const messages = Array.isArray(promptOrMessages)
                        ? promptOrMessages
                        : [{ role: 'user', content: promptOrMessages }];
                    console.log('[WebLLM] Starting inference, messages:', messages.length, tools ? `tools: ${tools.length}` : '');
                    let content = '';
                    let toolCalls = null;
                    const createOpts = { messages, temperature: 0.7, max_tokens: 1024 };
                    if (tools && tools.length > 0) createOpts.tools = tools;
                    if (typeof onToken === 'function') {
                        // Streaming mode: tokens arrive one at a time
                        createOpts.stream = true;
                        createOpts.stream_options = { include_usage: true };
                        const stream = await engine.chat.completions.create(createOpts);
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
                        const reply = await engine.chat.completions.create(createOpts);
                        const choice = reply.choices?.[0];
                        content = choice?.message?.content || '';
                        // Check for tool calls in the response
                        if (choice?.message?.tool_calls && choice.message.tool_calls.length > 0) {
                            toolCalls = choice.message.tool_calls;
                        }
                    }
                    if (aborted) return { ok: true, result: content, dispatch: 'webllm', model: _webllmModel, ms: Math.round((performance.now() - t0) * 10) / 10 };
                    const dt = performance.now() - t0;
                    console.log('[WebLLM] Inference done in', Math.round(dt), 'ms, chars:', content.length);
                    _webllmProgress('');
                    const out = {
                        ok: true,
                        result: content,
                        dispatch: 'webllm',
                        model: _webllmModel,
                        ms: Math.round(dt * 10) / 10,
                    };
                    if (toolCalls) out.tool_calls = toolCalls;
                    return out;
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

    // ── OpenAI streaming inference (sentinel dispatch from llm.prompt.openai.ws) ──
    async _callOpenAIStream(prompt, model, onToken) {
        const t0 = performance.now();
        const apiKey = await _ensureVoiceApiKey(this);
        if (!apiKey) {
            return { ok: false, error: 'OpenAI API key required. Set OPENAI_API_KEY in Settings > Secrets', dispatch: 'openai_stream' };
        }
        const modelId = model || 'gpt-4o-mini';
        try {
            const response = await fetch('https://api.openai.com/v1/chat/completions', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Bearer ${apiKey}`,
                },
                body: JSON.stringify({
                    model: modelId,
                    messages: [{ role: 'user', content: prompt }],
                    stream: true,
                }),
            });
            if (!response.ok) {
                const err = await response.json().catch(() => ({}));
                return { ok: false, error: err?.error?.message || `HTTP ${response.status}`, dispatch: 'openai_stream' };
            }
            const reader = response.body.getReader();
            const decoder = new TextDecoder();
            let fullText = '';
            let done = false;
            while (!done) {
                const { done: streamDone, value } = await reader.read();
                done = streamDone;
                if (!value) continue;
                const chunk = decoder.decode(value, { stream: !done });
                for (const line of chunk.split('\n')) {
                    const trimmed = line.trim();
                    if (!trimmed.startsWith('data: ')) continue;
                    const data = trimmed.slice(6);
                    if (data === '[DONE]') { done = true; break; }
                    try {
                        const parsed = JSON.parse(data);
                        const delta = parsed.choices?.[0]?.delta?.content || '';
                        if (delta) {
                            fullText += delta;
                            if (typeof onToken === 'function') onToken(delta);
                        }
                    } catch (_) {}
                }
            }
            return {
                ok: true,
                result: fullText,
                dispatch: 'openai_stream',
                model: modelId,
                ms: Math.round((performance.now() - t0) * 10) / 10,
            };
        } catch (e) {
            return { ok: false, error: e.message || String(e), dispatch: 'openai_stream' };
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
 * Browser MCP server backed by the Traits SDK call cascade.
 * Tool calls use `Traits.call()` so they can reach helper/relay/server native
 * traits when connected, while still supporting local WASM fallback.
 *
 * Supports three transports:
 *   1. Direct: call await mcpServer.message(jsonRpcString) → jsonRpcResponse
 *   2. BroadcastChannel: cross-tab MCP over a named channel
 *   3. MessagePort: iframe/worker/extension MCP over a MessagePort
 *
 * Usage:
 *   const mcp = new McpServer();                    // direct mode
 *   const resp = await mcp.message('{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}');
 *
 *   const mcp = new McpServer({ channel: 'traits-mcp' });  // cross-tab
 *   // Other tabs: new BroadcastChannel('traits-mcp').postMessage({jsonrpc:'2.0',...})
 *
 *   const mcp = new McpServer({ port: messagePort });       // worker/iframe
 */
class McpServer {
    constructor(opts = {}) {
        this._listeners = [];
        this._sdk = opts.sdk || getTraits();

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
    async message(jsonRpcString) {
        await this._sdk.init();

        let request;
        try {
            request = JSON.parse(jsonRpcString);
        } catch (e) {
            return JSON.stringify({ jsonrpc: '2.0', id: null, error: { code: -32700, message: `Parse error: ${e.message}` } });
        }

        if (request.id === undefined || request.id === null) return '';
        const id = request.id;
        const method = request.method || '';

        if (method === 'initialize') {
            const status = this._sdk.status || {};
            return JSON.stringify({
                jsonrpc: '2.0',
                id,
                result: {
                    protocolVersion: '2024-11-05',
                    capabilities: { tools: { listChanged: false } },
                    serverInfo: { name: 'traits-browser-mcp', version: status.version || null },
                    info: {
                        runtime: 'browser',
                        helper_connected: !!status.helper,
                        relay_connected: !!status.relay,
                        wasm_loaded: !!status.wasm,
                    },
                },
            });
        }

        if (method === 'ping') {
            return JSON.stringify({ jsonrpc: '2.0', id, result: {} });
        }

        if (method === 'tools/list') {
            const entries = await this._sdk.list();
            const tools = [];
            const sorted = [...entries].sort((a, b) => (a.path || '').localeCompare(b.path || ''));

            for (const entry of sorted) {
                const path = entry?.path || '';
                if (!path || path === 'sys.mcp' || path === 'kernel.main') continue;

                const properties = {};
                const required = [];
                for (const p of (entry.params || [])) {
                    const name = p?.name || 'arg';
                    const type = String(p?.type || 'string').toLowerCase();
                    const schemaType = (type === 'int' || type === 'integer')
                        ? 'integer'
                        : (type === 'float' || type === 'number')
                            ? 'number'
                            : (type === 'bool' || type === 'boolean')
                                ? 'boolean'
                                : 'string';
                    properties[name] = { type: schemaType };
                    if (p?.description) properties[name].description = p.description;
                    if (p?.required !== false) required.push(name);
                }

                tools.push({
                    name: path.replace(/\./g, '_'),
                    description: entry.description || '',
                    inputSchema: {
                        type: 'object',
                        properties,
                        ...(required.length ? { required } : {}),
                    },
                });
            }

            return JSON.stringify({ jsonrpc: '2.0', id, result: { tools } });
        }

        if (method === 'tools/call') {
            const params = request.params || {};
            const toolName = params.name;
            if (!toolName || typeof toolName !== 'string') {
                return JSON.stringify({ jsonrpc: '2.0', id, error: { code: -32602, message: 'Missing tool name' } });
            }

            const argumentsObj = (params.arguments && typeof params.arguments === 'object') ? { ...params.arguments } : {};
            const forceRaw = (argumentsObj.__dispatch || '').toString().toLowerCase();
            if ('__dispatch' in argumentsObj) delete argumentsObj.__dispatch;
            const force = forceRaw && ['wasm', 'native', 'helper', 'relay', 'rest'].includes(forceRaw)
                ? forceRaw
                : undefined;

            const traitPathRaw = toolName.replace(/_/g, '.');
            const parsed = parseDispatchTarget(traitPathRaw);
            const traitPath = parsed.cleanPath;

            const info = await this._sdk.info(traitPath);
            let args = [];
            if (info && Array.isArray(info.params) && info.params.length) {
                args = info.params.map((p) => Object.prototype.hasOwnProperty.call(argumentsObj, p.name) ? argumentsObj[p.name] : null);
            } else {
                args = Object.values(argumentsObj);
            }

            const callRes = await this._sdk.call(
                parsed.target ? `${traitPath}@${parsed.target}` : traitPath,
                args,
                force ? { force } : {}
            );

            if (!callRes.ok) {
                return JSON.stringify({
                    jsonrpc: '2.0',
                    id,
                    error: { code: -32602, message: callRes.error || `Dispatch failed for: ${traitPath}` },
                });
            }

            const text = typeof callRes.result === 'string'
                ? callRes.result
                : JSON.stringify(callRes.result, null, 2);

            return JSON.stringify({
                jsonrpc: '2.0',
                id,
                result: {
                    content: [{ type: 'text', text }],
                    dispatch: callRes.dispatch,
                },
            });
        }

        return JSON.stringify({
            jsonrpc: '2.0',
            id,
            error: { code: -32601, message: `Method not found: ${method}` },
        });
    }

    /** Process a parsed JSON-RPC object. Returns parsed response (or null for notifications). */
    async handle(jsonRpcObj) {
        const resp = await this.message(JSON.stringify(jsonRpcObj));
        if (!resp) return null;
        try { return JSON.parse(resp); } catch { return null; }
    }

    /** Listen for MCP events (for logging/debugging). */
    onMessage(fn) { this._listeners.push(fn); }

    async _handleTransportMessage(data, reply) {
        const msg = typeof data === 'string' ? data : JSON.stringify(data);
        for (const fn of this._listeners) try { fn('request', msg); } catch {}
        const resp = await this.message(msg);
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
