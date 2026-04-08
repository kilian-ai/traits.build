use serde_json::Value;
use maud::{html, DOCTYPE, PreEscaped};

pub fn canvas(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "traits.build — Canvas" }
                style {
                    (PreEscaped(r#"
                        :root { --bg: #0a0a0a; --fg: #e0e0e0; --accent: #00e0ff; --border: #222; }
                        body { margin: 0; padding: 0; background: var(--bg); color: var(--fg); font-family: system-ui, sans-serif; }
                        .canvas-header {
                            display: flex; align-items: center; justify-content: space-between;
                            padding: 12px 20px; border-bottom: 1px solid var(--border);
                            background: #111;
                        }
                        .canvas-header h1 { font-size: 16px; font-weight: 500; }
                        .canvas-header h1 .accent { color: var(--accent); }
                        .canvas-header .actions { display: flex; gap: 8px; }
                        .canvas-header button {
                            background: transparent; border: 1px solid var(--border);
                            color: var(--fg); padding: 4px 12px; border-radius: 4px;
                            cursor: pointer; font-size: 12px;
                        }
                        .canvas-header button:hover { border-color: var(--accent); color: var(--accent); }
                        .canvas-header button.save-btn { border-color: #2a5; color: #2a5; }
                        .canvas-header button.save-btn:hover { border-color: #3c8; color: #3c8; }

                        /* Project bar */
                        #project-bar {
                            display: none; padding: 6px 20px; background: #0d0d0d;
                            border-bottom: 1px solid var(--border);
                            overflow-x: auto; white-space: nowrap;
                        }
                        #project-bar.has-projects { display: flex; gap: 6px; align-items: center; }
                        #project-bar .project-chip {
                            display: inline-flex; align-items: center; gap: 4px;
                            padding: 3px 10px; border-radius: 4px; font-size: 11px;
                            background: #181818; border: 1px solid var(--border);
                            color: #aaa; cursor: pointer; flex-shrink: 0;
                        }
                        #project-bar .project-chip:hover { border-color: var(--accent); color: var(--accent); }
                        #project-bar .project-chip .del {
                            color: #555; cursor: pointer; font-size: 13px; margin-left: 2px;
                        }
                        #project-bar .project-chip .del:hover { color: #f44; }
                        #project-bar .project-label { font-size: 10px; color: #555; margin-right: 4px; flex-shrink: 0; }

                        #canvas-container {
                            width: 100%; min-height: calc(100vh - 100px);
                            padding: 40px 20px; position: relative;
                            display: flex; justify-content: center; align-items: flex-start;
                        }
                        .canvas-empty {
                            display: flex; flex-direction: column; align-items: center;
                            justify-content: center; height: 60vh; color: #555;
                        }
                        .canvas-empty .icon { font-size: 48px; margin-bottom: 16px; opacity: 0.5; }
                        .canvas-empty p { font-size: 14px; }
                        .canvas-empty code { color: var(--accent); font-size: 13px; }

                        /* Phone frame */
                        #phone-frame {
                            display: none;
                            position: relative;
                            width: 430px;
                            background: #1a1a1c;
                            border-radius: 50px;
                            border: 2px solid #3a3a3c;
                            box-shadow: 0 0 0 1px #111, 0 30px 80px rgba(0,0,0,0.8), inset 0 0 0 1px #2a2a2c;
                            padding: 18px 16px 22px;
                            flex-shrink: 0;
                        }
                        #phone-frame.visible { display: block; }
                        .phone-notch {
                            width: 120px; height: 34px;
                            background: #1a1a1c;
                            border-radius: 0 0 22px 22px;
                            margin: 0 auto 10px;
                            position: relative; z-index: 2;
                            display: flex; align-items: center; justify-content: center;
                            gap: 8px;
                        }
                        .phone-notch .camera {
                            width: 12px; height: 12px; border-radius: 50%;
                            background: #111; border: 1px solid #2a2a2c;
                        }
                        .phone-notch .speaker {
                            width: 50px; height: 6px; border-radius: 3px;
                            background: #111; border: 1px solid #222;
                        }
                        #phone-viewport {
                            width: 398px;
                            height: 844px;
                            border-radius: 36px;
                            overflow: hidden;
                            background: #000;
                            position: relative;
                            border: none;
                            display: block;
                        }
                        .phone-home-bar {
                            width: 134px; height: 5px;
                            background: #555; border-radius: 3px;
                            margin: 10px auto 0;
                        }

                        /* FAB menu */
                        #canvas-fab {
                            position: fixed; bottom: 20px; right: 20px; z-index: 9990;
                        }
                        #canvas-fab .fab-btn {
                            width: 44px; height: 44px; border-radius: 50%;
                            background: rgba(124,92,252,0.15); border: 1px solid rgba(124,92,252,0.4);
                            color: #b8a4fc; font-size: 20px; cursor: pointer;
                            display: flex; align-items: center; justify-content: center;
                            backdrop-filter: blur(8px); transition: transform 0.2s, background 0.2s;
                        }
                        #canvas-fab .fab-btn:hover { background: rgba(124,92,252,0.25); transform: scale(1.08); }
                        #canvas-fab .fab-btn.open { transform: rotate(45deg); }
                        #canvas-fab .fab-menu {
                            display: none; position: absolute; bottom: 52px; right: 0;
                            background: rgba(20,20,25,0.95); border: 1px solid #333;
                            border-radius: 8px; padding: 4px 0; min-width: 160px;
                            backdrop-filter: blur(12px); box-shadow: 0 4px 20px rgba(0,0,0,0.5);
                        }
                        #canvas-fab .fab-menu.show { display: block; }
                        #canvas-fab .fab-menu button {
                            display: flex; align-items: center; gap: 8px; width: 100%;
                            padding: 8px 14px; border: none; background: none;
                            color: #ccc; font-size: 13px; cursor: pointer; text-align: left;
                        }
                        #canvas-fab .fab-menu button:hover { background: rgba(124,92,252,0.12); color: #fff; }
                        #canvas-fab .fab-menu button .fab-icon { width: 18px; text-align: center; flex-shrink: 0; }

                        /* Voice Chat Modal */
                        #voice-chat-modal {
                            display: none; position: fixed;
                            bottom: 74px; right: 20px;
                            width: 340px; height: 460px;
                            background: rgba(14,14,18,0.97);
                            border: 1px solid rgba(124,92,252,0.4);
                            border-radius: 12px; z-index: 9995;
                            box-shadow: 0 8px 32px rgba(0,0,0,0.6);
                            backdrop-filter: blur(16px);
                            flex-direction: column;
                        }
                        #voice-chat-modal.vcm-open { display: flex; }
                        .vcm-header {
                            display: flex; justify-content: space-between; align-items: center;
                            padding: 10px 14px; border-bottom: 1px solid rgba(255,255,255,0.07);
                            cursor: move; flex-shrink: 0;
                        }
                        .vcm-title { color: #b8a4fc; font-size: 13px; font-weight: 600; letter-spacing: 0.02em; }
                        .vcm-close {
                            background: none; border: none; color: #555; font-size: 18px;
                            cursor: pointer; padding: 0 2px; line-height: 1; transition: color 0.15s;
                        }
                        .vcm-close:hover { color: #fff; }
                        .vcm-log {
                            flex: 1; overflow-y: auto; padding: 10px 12px;
                            display: flex; flex-direction: column; gap: 5px;
                        }
                        .vcm-log::-webkit-scrollbar { width: 4px; }
                        .vcm-log::-webkit-scrollbar-thumb { background: #333; border-radius: 2px; }
                        .vcm-msg {
                            padding: 5px 10px; border-radius: 8px; font-size: 12px;
                            line-height: 1.5; max-width: 95%; word-break: break-word;
                        }
                        .vcm-msg.user { background: rgba(124,92,252,0.2); color: #d4c8ff; align-self: flex-end; }
                        .vcm-msg.assistant { background: rgba(30,30,38,0.9); color: #e0e0e0; align-self: flex-start; border: 1px solid rgba(255,255,255,0.06); }
                        .vcm-msg.tool { background: rgba(0,200,100,0.08); color: #4ade80; font-family: monospace; font-size: 11px; align-self: flex-start; }
                        .vcm-msg.tool-result { background: rgba(56,189,248,0.07); color: #7dd3fc; font-family: monospace; font-size: 11px; align-self: flex-start; }
                        .vcm-msg.system { color: #555; font-size: 11px; font-style: italic; align-self: center; }
                        .vcm-input-row {
                            display: flex; gap: 6px; padding: 8px 10px;
                            border-top: 1px solid rgba(255,255,255,0.07); flex-shrink: 0;
                        }
                        #vcmInput {
                            flex: 1; background: rgba(255,255,255,0.06); border: 1px solid #333;
                            border-radius: 6px; color: #eee; padding: 6px 10px; font-size: 13px; outline: none;
                        }
                        #vcmInput:focus { border-color: rgba(124,92,252,0.55); }
                        #vcmSend {
                            background: rgba(124,92,252,0.25); border: 1px solid rgba(124,92,252,0.45);
                            border-radius: 6px; color: #b8a4fc; padding: 6px 12px;
                            cursor: pointer; font-size: 16px; transition: background 0.15s;
                        }
                        #vcmSend:hover { background: rgba(124,92,252,0.45); }

                        /* P2P Share Modal */
                        #share-modal {
                            display: none; position: fixed;
                            bottom: 74px; right: 20px;
                            width: 320px;
                            background: rgba(14,14,18,0.97);
                            border: 1px solid rgba(124,92,252,0.4);
                            border-radius: 12px; z-index: 9996;
                            box-shadow: 0 8px 40px rgba(0,0,0,0.7);
                            backdrop-filter: blur(16px);
                            flex-direction: column;
                        }
                        #share-modal.sm-open { display: flex; }
                        .sm-header {
                            display: flex; justify-content: space-between; align-items: center;
                            padding: 10px 14px; border-bottom: 1px solid rgba(255,255,255,0.07);
                            cursor: move; flex-shrink: 0;
                        }
                        .sm-title { color: #b8a4fc; font-size: 13px; font-weight: 600; }
                        .sm-close {
                            background: none; border: none; color: #555; font-size: 18px;
                            cursor: pointer; padding: 0 2px; line-height: 1; transition: color 0.15s;
                        }
                        .sm-close:hover { color: #fff; }
                        .sm-body {
                            padding: 18px 16px 16px;
                            display: flex; flex-direction: column; align-items: center; gap: 14px;
                        }
                        .sm-label { color: #888; font-size: 12px; text-align: center; }
                        .sm-code-display {
                            display: flex; gap: 8px;
                        }
                        .sm-code-char {
                            width: 52px; height: 64px;
                            background: rgba(124,92,252,0.1);
                            border: 1px solid rgba(124,92,252,0.4);
                            border-radius: 8px; color: #b8a4fc;
                            font-size: 28px; font-weight: 700; font-family: monospace;
                            display: flex; align-items: center; justify-content: center;
                            letter-spacing: 0;
                        }
                        .sm-code-inputs { display: flex; gap: 8px; }
                        .sm-ci {
                            width: 52px; height: 64px; text-align: center;
                            background: rgba(255,255,255,0.06);
                            border: 1px solid rgba(124,92,252,0.35);
                            border-radius: 8px; color: #d4c8ff;
                            font-size: 26px; font-weight: 700; font-family: monospace;
                            outline: none; text-transform: uppercase; caret-color: #b8a4fc;
                        }
                        .sm-ci:focus { border-color: rgba(124,92,252,0.8); background: rgba(124,92,252,0.08); }
                        .sm-link-row {
                            display: flex; gap: 6px; width: 100%; align-items: center;
                        }
                        .sm-link {
                            flex: 1; font-size: 10px; color: #555; overflow: hidden;
                            text-overflow: ellipsis; white-space: nowrap; user-select: text;
                        }
                        .sm-btn {
                            background: rgba(124,92,252,0.2); border: 1px solid rgba(124,92,252,0.4);
                            border-radius: 6px; color: #b8a4fc; padding: 5px 12px;
                            font-size: 12px; cursor: pointer; flex-shrink: 0; transition: background 0.15s;
                        }
                        .sm-btn:hover { background: rgba(124,92,252,0.4); }
                        .sm-btn-primary {
                            background: rgba(124,92,252,0.3); border: 1px solid rgba(124,92,252,0.6);
                            border-radius: 8px; color: #d4c8ff; padding: 9px 28px;
                            font-size: 14px; cursor: pointer; transition: background 0.15s; width: 100%;
                        }
                        .sm-btn-primary:hover { background: rgba(124,92,252,0.5); }
                        .sm-status {
                            font-size: 12px; color: #888; text-align: center; min-height: 16px;
                        }
                        .sm-status.ok { color: #4ade80; }
                        .sm-status.err { color: #f87171; }
                        .sm-progress {
                            width: 100%; height: 3px; background: rgba(255,255,255,0.08);
                            border-radius: 2px; overflow: hidden;
                        }
                        .sm-progress-bar {
                            height: 100%; background: linear-gradient(90deg,#7c5cfc,#b8a4fc);
                            border-radius: 2px; transition: width 0.3s;
                        }

                    "#))
                }
            }
            body {
                div .canvas-header {
                    h1 { "traits.build " span .accent { "canvas" } }
                    div .actions {
                        button #btnSave .save-btn { "Save" }
                        button #btnClear { "Clear" }
                        button #btnSource { "View Source" }
                    }
                }
                div #project-bar {}
                div #canvas-container {
                    div .canvas-empty #canvas-empty {
                        div .icon { "🎨" }
                        p { "Canvas is empty — use " code { "sys.canvas set \"<html>\"" } " or voice to draw." }
                    }
                    div #phone-frame {
                        div .phone-notch {
                            div .speaker {}
                            div .camera {}
                        }
                        iframe #phone-viewport sandbox="allow-scripts allow-same-origin" {}
                        div .phone-home-bar {}
                    }
                }

                // FAB menu
                div #canvas-fab {
                    button .fab-btn #fabToggle { "+" }
                    div .fab-menu #fabMenu {
                        button #fabNew {
                            span .fab-icon { "✨" }
                            span { "New Canvas" }
                        }
                        button #fabVoice {
                            span .fab-icon { "🎤" }
                            span { "Start Voice" }
                        }
                        button #fabSplats {
                            span .fab-icon { "🔮" }
                            span { "Splat Viewer" }
                        }
                        button #fabShare {
                            span .fab-icon { "📤" }
                            span { "Share Project" }
                        }
                        button #fabReceive {
                            span .fab-icon { "📥" }
                            span { "Receive Project" }
                        }
                    }
                }

                // P2P project share modal
                div #share-modal {
                    div .sm-header {
                        span .sm-title #smTitle { "📤  Share Project" }
                        button .sm-close #smClose { "×" }
                    }
                    div .sm-body #smBody {}
                }

                // Voice chat floating modal
                div #voice-chat-modal {
                    div .vcm-header {
                        span .vcm-title { "💬  Voice Chat" }
                        button .vcm-close #vcmClose { "×" }
                    }
                    div .vcm-log #vcmLog {}
                    div .vcm-input-row {
                        input #vcmInput type="text" placeholder="Type to voice agent…" {}
                        button #vcmSend { "↑" }
                    }
                }

                script { (PreEscaped(r#"
                    (function() {
                        // ── Canvas SDK: thin global API for scripts injected into the canvas ──
                        // Available as `traits.call(...)`, `traits.list()`, etc. in canvas scripts
                        const _sdk = () => window._traitsSDK;
                        window.traits = {
                            /** Call any trait: traits.call('skills.spotify.play', ['spotify:track:...']) */
                            call: (path, args) => _sdk()?.call(path, args || []),
                            /** List traits, optional namespace filter: traits.list('skills') */
                            list: (ns) => _sdk()?.call('sys.list', ns ? [ns] : []),
                            /** Get trait info: traits.info('skills.spotify.play') */
                            info: (path) => _sdk()?.call('sys.info', [path]),
                            /** Update the canvas itself: traits.canvas('set', html) */
                            canvas: (action, content) => {
                                const args = content !== undefined ? [action, content] : [action];
                                return _sdk()?.call('sys.canvas', args);
                            },
                            /** Echo text to the terminal: traits.echo('hello') */
                            echo: (text) => _sdk()?.call('sys.echo', [text]),
                            /** Play audio: traits.audio('tone', 440, 0.5) */
                            audio: (action, ...a) => _sdk()?.call('sys.audio', [action, ...a]),
                        };

                        const container = document.getElementById('canvas-container');
                        const empty = document.getElementById('canvas-empty');
                        const projectBar = document.getElementById('project-bar');
                        let sourceMode = false;

                        // ── Project management ──
                        const PROJECT_PFX = 'traits.canvas.project.';

                        function getProjects() {
                            const projects = [];
                            for (let i = 0; i < localStorage.length; i++) {
                                const k = localStorage.key(i);
                                if (k && k.startsWith(PROJECT_PFX)) {
                                    const name = k.slice(PROJECT_PFX.length);
                                    try {
                                        const data = JSON.parse(localStorage.getItem(k));
                                        projects.push({ name, length: (data.content || '').length, saved: data.saved });
                                    } catch(_) { projects.push({ name, length: 0 }); }
                                }
                            }
                            return projects.sort((a, b) => (b.saved || 0) - (a.saved || 0));
                        }

                        function renderProjectBar() {
                            const projects = getProjects();
                            if (!projects.length) {
                                projectBar.className = '';
                                projectBar.innerHTML = '';
                                return;
                            }
                            projectBar.className = 'has-projects';
                            projectBar.innerHTML = '<span class="project-label">Projects:</span>' +
                                projects.map(p =>
                                    '<span class="project-chip" data-name="' + p.name.replace(/"/g, '&quot;') + '">' +
                                    p.name +
                                    ' <span class="del" data-del="' + p.name.replace(/"/g, '&quot;') + '">&times;</span>' +
                                    '</span>'
                                ).join('');
                            // Click handlers
                            projectBar.querySelectorAll('.project-chip').forEach(chip => {
                                chip.addEventListener('click', (e) => {
                                    if (e.target.classList.contains('del')) return;
                                    loadProject(chip.dataset.name);
                                });
                            });
                            projectBar.querySelectorAll('.del').forEach(del => {
                                del.addEventListener('click', (e) => {
                                    e.stopPropagation();
                                    const name = del.dataset.del;
                                    if (confirm('Delete project "' + name + '"?')) {
                                        localStorage.removeItem(PROJECT_PFX + name);
                                        renderProjectBar();
                                    }
                                });
                            });
                        }

                        async function loadProject(name) {
                            try {
                                const raw = localStorage.getItem(PROJECT_PFX + name);
                                if (!raw) return;
                                const proj = JSON.parse(raw);
                                const sdk = window._traitsSDK;
                                if (sdk) await sdk.call('sys.canvas', ['set', proj.content]);
                                renderCanvas(proj.content);
                            } catch(e) { console.warn('load project:', e); }
                        }

                        async function saveProject() {
                            const sdk = window._traitsSDK;
                            if (!sdk) return;
                            const res = await sdk.call('sys.canvas', ['get']);
                            const content = res?.result?.content || res?.content || '';
                            if (!content) { alert('Canvas is empty — nothing to save.'); return; }
                            const name = prompt('Project name:');
                            if (!name || !name.trim()) return;
                            localStorage.setItem(PROJECT_PFX + name.trim(), JSON.stringify({ content, saved: Date.now() }));
                            renderProjectBar();
                        }

                        // Save button
                        document.getElementById('btnSave').addEventListener('click', saveProject);

                        // Listen for external project changes (from voice/MCP bridge)
                        window.addEventListener('traits-canvas-projects-changed', renderProjectBar);

                        // Initial render
                        renderProjectBar();

                        const phoneFrame    = document.getElementById('phone-frame');
                        const phoneViewport = document.getElementById('phone-viewport');
                        let _currentContent = '';

                        // Bridge script injected into every iframe render.
                        // Exposes window.traits, forward calls to parent SDK.
                        // Also adds querySelector shim for legacy #phone-viewport/#canvas-container selectors.
                        const BRIDGE = `<script>(function(){
                            var sdk = function(){ return window.parent._traitsSDK; };
                            window.traits = {
                                call:   function(p,a)   { return sdk() && sdk().call(p, a||[]); },
                                list:   function(ns)    { return sdk() && sdk().call('sys.list', ns?[ns]:[]); },
                                info:   function(p)     { return sdk() && sdk().call('sys.info', [p]); },
                                echo:   function(t)     { return sdk() && sdk().call('sys.echo', [t]); },
                                canvas: function(a,c)   { return sdk() && sdk().call('sys.canvas', c!==undefined?[a,c]:[a]); },
                                audio:  function(a)     {
                                    var r = Array.prototype.slice.call(arguments, 1);
                                    return sdk() && sdk().call('sys.audio', [a].concat(r));
                                },
                            };
                            // querySelector compat: strip legacy outer-DOM prefixes
                            var _qs = document.querySelector.bind(document);
                            document.querySelector = function(s) {
                                return _qs(s.replace(/^#phone-viewport\s+/,'').replace(/^#canvas-container\s+/,''));
                            };
                        })();<\/script>`;

                        function renderCanvas(content) {
                            if (!content) {
                                if (_currentContent === '') return; // already cleared
                                _currentContent = '';
                                phoneFrame.classList.remove('visible');
                                container.appendChild(empty);
                                empty.style.display = 'flex';
                                return;
                            }
                            if (content === _currentContent) return; // skip srcdoc reset if content unchanged
                            _currentContent = content;
                            empty.style.display = 'none';
                            phoneFrame.classList.add('visible');
                            // No global style injection — iframe provides full CSS isolation.
                            document.querySelectorAll('style[data-canvas]').forEach(s => s.remove());

                            // Build a complete HTML document, injecting the traits bridge.
                            let fullHtml = content.trim();
                            if (!/<html[\s>]/i.test(fullHtml)) {
                                // Fragment — wrap in a minimal document
                                fullHtml = '<!DOCTYPE html><html><head>' +
                                    '<meta charset="UTF-8">' +
                                    '<style>*{margin:0;padding:0;box-sizing:border-box}' +
                                    'html,body{width:390px;height:844px;overflow:hidden;background:#0a0a0a;color:#e0e0e0}' +
                                    'canvas{display:block}</style>' +
                                    BRIDGE + '</head><body>' + fullHtml + '</body></html>';
                            } else {
                                // Full document — inject bridge after opening <head> tag
                                if (/<head\b[^>]*>/i.test(fullHtml)) {
                                    fullHtml = fullHtml.replace(/(<head\b[^>]*>)/i, '$1' + BRIDGE);
                                } else {
                                    fullHtml = fullHtml.replace(/(<html\b[^>]*>)/i, '$1<head>' + BRIDGE + '</head>');
                                }
                            }

                            phoneViewport.srcdoc = fullHtml;
                        }

                        async function loadCanvas() {
                            try {
                                const sdk = window._traitsSDK;
                                if (!sdk) return;
                                const res = await sdk.call('sys.canvas', ['get']);
                                const content = res?.result?.content || res?.content || '';
                                renderCanvas(content || '');
                            } catch(e) { console.warn('canvas load:', e); }
                        }

                        // Read canvas/app.html directly from localStorage (shared by Worker + main-thread WASM).
                        // Bypasses the in-memory WASM VFS which may be stale.
                        function readCanvasFromStorage() {
                            try {
                                const raw = localStorage.getItem('traits.pvfs');
                                if (!raw) return '';
                                const files = JSON.parse(raw);
                                return files['canvas/app.html'] || '';
                            } catch(_) { return ''; }
                        }

                        // Listen for live updates from voice/SDK
                        window.addEventListener('traits-canvas-update', (e) => {
                            const content = e.detail?.content;
                            if (content !== undefined) {
                                __lastContent = content; // keep poller in sync — avoid duplicate re-render
                                renderCanvas(content);
                            } else {
                                // Re-read from localStorage (Worker may have written)
                                const stored = readCanvasFromStorage();
                                if (stored) { __lastContent = stored; renderCanvas(stored); }
                                else loadCanvas();
                            }
                        });

                        // Clear button
                        document.getElementById('btnClear').addEventListener('click', async () => {
                            const sdk = window._traitsSDK;
                            if (sdk) await sdk.call('sys.canvas', ['clear']);
                            renderCanvas('');
                        });

                        // View Source toggle
                        document.getElementById('btnSource').addEventListener('click', () => {
                            sourceMode = !sourceMode;
                            const btn = document.getElementById('btnSource');
                            if (sourceMode) {
                                phoneFrame.classList.add('visible');
                                const escaped = (_currentContent || '(empty)')
                                    .replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
                                phoneViewport.srcdoc = '<!DOCTYPE html><html><head><style>' +
                                    'body{margin:0;background:#0a0a0a}' +
                                    'pre{white-space:pre-wrap;word-break:break-all;color:#888;' +
                                    'font-size:12px;padding:16px;height:100vh;box-sizing:border-box;overflow:auto}' +
                                    '</style></head><body><pre>' + escaped + '</pre></body></html>';
                                btn.textContent = 'Live View';
                            } else {
                                btn.textContent = 'View Source';
                                renderCanvas(_currentContent);
                            }
                        });

                        // Don't auto-render stale content from previous sessions.
                        // Initialise __lastContent so the poller ignores pre-existing storage
                        // and only fires on NEW writes from the agent.
                        let __lastContent = readCanvasFromStorage();

                        // Poll localStorage for agent writes (1 s — backup for missed events)
                        const _pollId = setInterval(() => {
                            try {
                                if (sourceMode) return;
                                const content = readCanvasFromStorage();
                                if (content && content !== __lastContent) {
                                    __lastContent = content;
                                    renderCanvas(content);
                                }
                            } catch(_) {}
                        }, 1000);

                        // ── FAB menu ──
                        const fabToggle = document.getElementById('fabToggle');
                        const fabMenu = document.getElementById('fabMenu');
                        fabToggle.addEventListener('click', (e) => {
                            e.stopPropagation();
                            fabMenu.classList.toggle('show');
                            fabToggle.classList.toggle('open');
                        });
                        // Close FAB menu when clicking outside
                        document.addEventListener('click', (e) => {
                            if (!e.target.closest('#canvas-fab') && !e.target.closest('#voice-chat-modal')) {
                                fabMenu.classList.remove('show');
                                fabToggle.classList.remove('open');
                            }
                        });
                        // New Canvas button
                        document.getElementById('fabNew').addEventListener('click', async () => {
                            fabMenu.classList.remove('show');
                            fabToggle.classList.remove('open');
                            const sdk = window._traitsSDK;
                            if (sdk) await sdk.call('sys.canvas', ['clear']);
                            __lastContent = '';
                            renderCanvas('');
                        });

                        // Voice button
                        document.getElementById('fabVoice').addEventListener('click', () => {
                            fabMenu.classList.remove('show');
                            fabToggle.classList.remove('open');
                            // Dispatch voice start via the global voice control bridge
                            window.dispatchEvent(new CustomEvent('traits-voice-control', { detail: { voice_control_action: 'start' } }));
                        });
                        // Splat viewer button
                        document.getElementById('fabSplats').addEventListener('click', async () => {
                            fabMenu.classList.remove('show');
                            fabToggle.classList.remove('open');
                            try {
                                const sdk = window._traitsSDK;
                                if (!sdk) return;
                                const splats = await sdk.call('www.splats', ['render']);
                                const html = (typeof splats === 'string') ? splats : splats?.result;
                                if (html && typeof html === 'string') {
                                    await sdk.call('sys.canvas', ['set', html]);
                                    renderCanvas(html);
                                }
                            } catch(e) { console.warn('splat load:', e); }
                        });

                        // Share / Receive project via WebRTC P2P
                        document.getElementById('fabShare').addEventListener('click', () => {
                            fabMenu.classList.remove('show'); fabToggle.classList.remove('open');
                            smOpen('send');
                        });
                        document.getElementById('fabReceive').addEventListener('click', () => {
                            fabMenu.classList.remove('show'); fabToggle.classList.remove('open');
                            smOpen('receive');
                        });

                        // ── P2P Share Modal ──
                        const shareModal   = document.getElementById('share-modal');
                        const smTitleEl    = document.getElementById('smTitle');
                        const smBodyEl     = document.getElementById('smBody');
                        const smCloseBtn   = document.getElementById('smClose');
                        const RELAY        = 'https://relay.traits.build';
                        let _smAbort = false;

                        smCloseBtn.addEventListener('click', (e) => { e.stopPropagation(); smHide(); });

                        // Drag
                        (() => {
                            let drag = false, ox = 0, oy = 0;
                            shareModal.querySelector('.sm-header').addEventListener('mousedown', (e) => {
                                drag = true;
                                shareModal.style.right = 'auto'; shareModal.style.bottom = 'auto';
                                ox = e.clientX - shareModal.offsetLeft;
                                oy = e.clientY - shareModal.offsetTop;
                            });
                            document.addEventListener('mousemove', (e) => {
                                if (!drag) return;
                                shareModal.style.left = (e.clientX - ox) + 'px';
                                shareModal.style.top  = (e.clientY - oy) + 'px';
                            });
                            document.addEventListener('mouseup', () => { drag = false; });
                        })();

                        function smOpen(mode) {
                            _smAbort = false;
                            shareModal.classList.add('sm-open');
                            smTitleEl.textContent = mode === 'send' ? '\ud83d\udce4\u2002Share Project' : '\ud83d\udce5\u2002Receive Project';
                            if (mode === 'send') smStartSend();
                            else smStartReceive();
                        }
                        function smHide() {
                            _smAbort = true;
                            shareModal.classList.remove('sm-open');
                        }
                        function smStatus(text, cls) {
                            const el = smBodyEl.querySelector('.sm-status');
                            if (!el) return;
                            el.textContent = text;
                            el.className = 'sm-status' + (cls ? ' ' + cls : '');
                        }
                        function smProgress(pct) {
                            const bar = smBodyEl.querySelector('.sm-progress-bar');
                            if (bar) bar.style.width = pct + '%';
                        }

                        // ── SENDER ──
                        // Strategy: register relay code, display it, wait for receiver to
                        // call path='project.recv' via relay/call, then respond with the
                        // full project content. No WebRTC — content travels through the
                        // relay (Cloudflare Worker). Two round-trips, always works.
                        async function smStartSend() {
                            smBodyEl.innerHTML = '<div class="sm-status">Registering\u2026</div>';

                            let code;
                            try {
                                const r = await fetch(RELAY + '/relay/register', { method: 'POST' });
                                if (!r.ok) throw new Error('HTTP ' + r.status);
                                const j = await r.json(); code = j.code;
                            } catch(e) {
                                smBodyEl.innerHTML = '<div class="sm-status err">\u26a0 Relay unreachable: ' + e.message + '</div>';
                                return;
                            }
                            if (_smAbort) return;

                            const shareUrl = location.origin + location.pathname + '#/canvas?receive=' + code;
                            smBodyEl.innerHTML = `
                                <div class="sm-label">Share this code with the receiver:</div>
                                <div class="sm-code-display">
                                    ${code.split('').map(c => `<div class="sm-code-char">${c}</div>`).join('')}
                                </div>
                                <div class="sm-link-row">
                                    <span class="sm-link" title="${shareUrl}">${shareUrl}</span>
                                    <button class="sm-btn" id="smCopyBtn">Copy</button>
                                </div>
                                <div class="sm-progress"><div class="sm-progress-bar" style="width:0%"></div></div>
                                <div class="sm-status">Waiting for receiver\u2026</div>
                            `;
                            smBodyEl.querySelector('#smCopyBtn').addEventListener('click', function() {
                                navigator.clipboard.writeText(shareUrl).then(() => {
                                    this.textContent = 'Copied!';
                                    setTimeout(() => { this.textContent = 'Copy'; }, 1600);
                                });
                            });

                            // Long-poll: wait for receiver to knock
                            let knock;
                            try {
                                const r = await fetch(RELAY + '/relay/poll?code=' + code, { signal: AbortSignal.timeout(120000) });
                                if (!r.ok) throw new Error('poll ' + r.status);
                                knock = await r.json();
                            } catch(e) {
                                if (!_smAbort) smStatus('\u26a0 Timed out — no receiver connected.', 'err');
                                return;
                            }
                            if (_smAbort) return;

                            smStatus('Receiver connected! Reading project\u2026');
                            smProgress(40);

                            // Read the current project
                            let content = '';
                            try {
                                const sdk = window._traitsSDK;
                                if (sdk) {
                                    const res = await sdk.call('sys.canvas', ['get']);
                                    content = res?.result?.content || res?.content || '';
                                }
                            } catch(_) {}
                            if (!content) content = document.getElementById('phone-viewport')?.srcdoc || '';

                            smStatus('Sending project\u2026');
                            smProgress(70);

                            // Respond with the project content
                            try {
                                await fetch(RELAY + '/relay/respond', {
                                    method: 'POST',
                                    headers: { 'Content-Type': 'application/json' },
                                    body: JSON.stringify({ code, id: knock.id, result: { content } }),
                                });
                            } catch(e) {
                                smStatus('\u26a0 Failed to send project: ' + e.message, 'err');
                                return;
                            }

                            smProgress(100);
                            smStatus('\u2713 Project sent!', 'ok');
                            setTimeout(() => smHide(), 1800);
                        }

                        // ── RECEIVER ──
                        async function smStartReceive() {
                            const autoCode = (() => {
                                try { const h = location.hash.split('?')[1]; return h ? new URLSearchParams(h).get('receive') || '' : ''; } catch(_) { return ''; }
                            })();
                            smBodyEl.innerHTML = `
                                <div class="sm-label">Enter the 4-letter code from the sender:</div>
                                <div class="sm-code-inputs" id="smCodeInputs">
                                    <input class="sm-ci" maxlength="1" inputmode="text" autocomplete="off" spellcheck="false">
                                    <input class="sm-ci" maxlength="1" inputmode="text" autocomplete="off" spellcheck="false">
                                    <input class="sm-ci" maxlength="1" inputmode="text" autocomplete="off" spellcheck="false">
                                    <input class="sm-ci" maxlength="1" inputmode="text" autocomplete="off" spellcheck="false">
                                </div>
                                <button class="sm-btn-primary" id="smConnectBtn">Receive \u2192</button>
                                <div class="sm-progress" style="opacity:0"><div class="sm-progress-bar" style="width:0%"></div></div>
                                <div class="sm-status"></div>
                            `;
                            const inputs = Array.from(smBodyEl.querySelectorAll('.sm-ci'));
                            inputs.forEach((inp, i) => {
                                inp.addEventListener('input', () => {
                                    inp.value = inp.value.toUpperCase().replace(/[^A-Z0-9]/g, '');
                                    if (inp.value && i < 3) inputs[i + 1].focus();
                                });
                                inp.addEventListener('keydown', (e) => {
                                    if (e.key === 'Backspace' && !inp.value && i > 0) inputs[i - 1].focus();
                                    if (e.key === 'Enter') smDoReceive(inputs);
                                });
                                inp.addEventListener('paste', (e) => {
                                    const txt = (e.clipboardData.getData('text') || '').toUpperCase().replace(/[^A-Z0-9]/g, '');
                                    inputs.forEach((b, j) => { b.value = txt[j] || ''; });
                                    e.preventDefault();
                                    inputs[Math.min(txt.length, 3)].focus();
                                });
                            });
                            smBodyEl.querySelector('#smConnectBtn').addEventListener('click', () => smDoReceive(inputs));
                            if (autoCode.length >= 4) {
                                autoCode.toUpperCase().split('').forEach((c, i) => { if (inputs[i]) inputs[i].value = c; });
                                setTimeout(() => smDoReceive(inputs), 300);
                            } else {
                                inputs[0].focus();
                            }
                        }

                        async function smDoReceive(inputs) {
                            const code = inputs.map(i => i.value).join('').toUpperCase();
                            if (code.length < 4) { smStatus('Enter all 4 characters.'); return; }

                            smBodyEl.querySelector('#smConnectBtn').disabled = true;
                            smBodyEl.querySelector('.sm-progress').style.opacity = '1';
                            smStatus('Contacting sender\u2026');
                            smProgress(30);

                            let result;
                            try {
                                const r = await fetch(RELAY + '/relay/call', {
                                    method: 'POST',
                                    headers: { 'Content-Type': 'application/json' },
                                    body: JSON.stringify({ code, path: 'project.recv', args: [] }),
                                });
                                if (!r.ok) throw new Error('HTTP ' + r.status);
                                const j = await r.json();
                                if (j.error) throw new Error(j.error);
                                result = j.result;
                            } catch(e) {
                                smStatus('\u26a0 ' + e.message, 'err');
                                smBodyEl.querySelector('#smConnectBtn').disabled = false;
                                return;
                            }
                            if (_smAbort) return;

                            smStatus('Applying project\u2026');
                            smProgress(80);
                            await smApplyProject(result?.content || result);
                        }

                        async function smApplyProject(content) {
                            if (!content) return;
                            try {
                                const sdk = window._traitsSDK;
                                if (sdk) await sdk.call('sys.canvas', ['set', content]);
                            } catch(_) {}
                            __lastContent = content;
                            renderCanvas(content);
                            smStatus('\u2713 Project received!', 'ok');
                            smProgress(100);
                            setTimeout(() => smHide(), 1800);
                        }
                        window._pageCleanup = async () => {
                            clearInterval(_pollId);
                            fabMenu.classList.remove('show');
                            // Remove injected canvas styles so they don't bleed into other pages
                            document.querySelectorAll('style[data-canvas]').forEach(s => s.remove());
                            // Auto-save canvas content before leaving
                            try {
                                const sdk = window._traitsSDK;
                                if (sdk) {
                                    const res = await sdk.call('sys.canvas', ['get']);
                                    const content = res?.result?.content || res?.content || '';
                                    if (content) {
                                        localStorage.setItem('traits.canvas.project._autosave', JSON.stringify({ content, saved: Date.now() }));
                                    }
                                }
                            } catch(_) {}
                            try { delete window.traits; } catch(_) {}
                        };

                        // ── Voice Chat Modal ──
                        const vcModal   = document.getElementById('voice-chat-modal');
                        const vcmLog    = document.getElementById('vcmLog');
                        const vcmInput  = document.getElementById('vcmInput');
                        const vcmSendBtn = document.getElementById('vcmSend');
                        const vcmCloseBtn = document.getElementById('vcmClose');

                        function vcmOpen()  { vcModal.classList.add('vcm-open'); }
                        function vcmHide()  { vcModal.classList.remove('vcm-open'); }
                        function vcmToggle() {
                            vcModal.classList.contains('vcm-open') ? vcmHide() : vcmOpen();
                        }

                        function vcmAppend(role, text) {
                            if (!text) return;
                            const el = document.createElement('div');
                            el.className = 'vcm-msg ' + role;
                            el.textContent = text;
                            vcmLog.appendChild(el);
                            vcmLog.scrollTop = vcmLog.scrollHeight;
                        }

                        // Long-press (+) button to open the chat modal
                        (() => {
                            let _pressTimer = null;
                            fabToggle.addEventListener('pointerdown', () => {
                                _pressTimer = setTimeout(() => {
                                    _pressTimer = null;
                                    fabMenu.classList.remove('show');
                                    fabToggle.classList.remove('open');
                                    vcmToggle();
                                }, 500);
                            });
                            fabToggle.addEventListener('pointerup', () => { if (_pressTimer) clearTimeout(_pressTimer); });
                            fabToggle.addEventListener('pointercancel', () => { if (_pressTimer) clearTimeout(_pressTimer); });
                            // Double-click also opens the modal immediately
                            fabToggle.addEventListener('dblclick', (e) => {
                                e.stopPropagation();
                                fabMenu.classList.remove('show');
                                fabToggle.classList.remove('open');
                                vcmToggle();
                            });
                        })();

                        vcmCloseBtn.addEventListener('click', (e) => { e.stopPropagation(); vcmHide(); });

                        // Drag-to-reposition via header
                        (() => {
                            let drag = false, ox = 0, oy = 0;
                            const header = vcModal.querySelector('.vcm-header');
                            header.addEventListener('mousedown', (e) => {
                                drag = true;
                                vcModal.style.right = 'auto';
                                vcModal.style.bottom = 'auto';
                                ox = e.clientX - vcModal.offsetLeft;
                                oy = e.clientY - vcModal.offsetTop;
                            });
                            document.addEventListener('mousemove', (e) => {
                                if (!drag) return;
                                vcModal.style.left = (e.clientX - ox) + 'px';
                                vcModal.style.top  = (e.clientY - oy) + 'px';
                            });
                            document.addEventListener('mouseup', () => { drag = false; });
                        })();

                        // Voice events → chat log
                        window.addEventListener('voice-event', (e) => {
                            const d = e.detail;
                            switch (d.type) {
                                case 'started':
                                    vcmAppend('system', '🎤 Voice session started');
                                    break;
                                case 'stopped':
                                case 'disconnected':
                                    vcmAppend('system', '⏹ Voice session ended');
                                    break;
                                case 'transcript':
                                    vcmAppend('user', d.text);
                                    break;
                                case 'response':
                                    vcmAppend('assistant', d.text);
                                    break;
                                case 'tool_call': {
                                    let args = d.arguments || '';
                                    try { args = JSON.stringify(JSON.parse(args), null, 0).slice(0, 140); } catch(_) { args = args.slice(0, 100); }
                                    vcmAppend('tool', '\u26A1 ' + d.name + '(' + args + ')');
                                    break;
                                }
                                case 'tool_result': {
                                    const preview = (d.result || '').slice(0, 140);
                                    vcmAppend('tool-result', '\u2713 ' + d.name + ': ' + preview);
                                    break;
                                }
                                case 'error':
                                    vcmAppend('system', '\u26A0 ' + d.message);
                                    break;
                            }
                        });

                        // Send typed message to voice model
                        function vcmSendText() {
                            const text = vcmInput.value.trim();
                            if (!text) return;
                            const sdk = window._traitsSDK;
                            if (!sdk || !sdk.sendVoiceText) {
                                vcmAppend('system', '\u26A0 Voice not active — start voice first');
                                return;
                            }
                            const ok = sdk.sendVoiceText(text);
                            if (ok) {
                                vcmAppend('user', text);
                                vcmInput.value = '';
                            } else {
                                vcmAppend('system', '\u26A0 Voice not connected');
                            }
                        }
                        vcmSendBtn.addEventListener('click', vcmSendText);
                        vcmInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') vcmSendText(); });
                    })();
                "#)) }
            }
        }
    };
    Value::String(markup.into_string())
}
