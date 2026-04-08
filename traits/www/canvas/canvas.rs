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
                        fabToggle.addEventListener('click', () => {
                            fabMenu.classList.toggle('show');
                            fabToggle.classList.toggle('open');
                        });
                        // Close FAB menu when clicking outside
                        document.addEventListener('click', (e) => {
                            if (!e.target.closest('#canvas-fab')) {
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

                        // Register cleanup: auto-save canvas and remove window.traits when navigating away
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
                    })();
                "#)) }
            }
        }
    };
    Value::String(markup.into_string())
}
