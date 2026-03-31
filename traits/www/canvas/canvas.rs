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
                        #canvas-container {
                            width: 100%; min-height: calc(100vh - 100px);
                            padding: 20px; position: relative;
                        }
                        .canvas-empty {
                            display: flex; flex-direction: column; align-items: center;
                            justify-content: center; height: 60vh; color: #555;
                        }
                        .canvas-empty .icon { font-size: 48px; margin-bottom: 16px; opacity: 0.5; }
                        .canvas-empty p { font-size: 14px; }
                        .canvas-empty code { color: var(--accent); font-size: 13px; }
                    "#))
                }
            }
            body {
                div .canvas-header {
                    h1 { "traits.build " span .accent { "canvas" } }
                    div .actions {
                        button #btnClear { "Clear" }
                        button #btnSource { "View Source" }
                    }
                }
                div #canvas-container {
                    div .canvas-empty #canvas-empty {
                        div .icon { "🎨" }
                        p { "Canvas is empty — use " code { "sys.canvas set \"<html>\"" } " or voice to draw." }
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
                        };

                        const container = document.getElementById('canvas-container');
                        const empty = document.getElementById('canvas-empty');
                        let sourceMode = false;

                        function renderCanvas(content) {
                            if (!content) {
                                container.innerHTML = '';
                                container.appendChild(empty);
                                empty.style.display = 'flex';
                                return;
                            }
                            empty.style.display = 'none';
                            // Remove previous canvas styles
                            document.querySelectorAll('style[data-canvas]').forEach(s => s.remove());
                            // Base style for injected content: ensure visibility on dark bg
                            const base = document.createElement('style');
                            base.dataset.canvas = '1';
                            base.textContent = `
                                #canvas-container { color: #e0e0e0; }
                                #canvas-container svg { fill: #e0e0e0; stroke: #e0e0e0; }
                                #canvas-container svg text { fill: #e0e0e0; }
                                #canvas-container canvas { display: block; }
                                #canvas-container h1, #canvas-container h2, #canvas-container h3,
                                #canvas-container p, #canvas-container span, #canvas-container div {
                                    color: inherit;
                                }
                            `;
                            document.head.appendChild(base);

                            // Extract HTML body from full documents, use as-is for fragments
                            let html = content;
                            if (/<body[\s>]/i.test(content)) {
                                const doc = new DOMParser().parseFromString(content, 'text/html');
                                doc.querySelectorAll('head style').forEach(style => {
                                    const s = document.createElement('style');
                                    s.dataset.canvas = '1';
                                    s.textContent = style.textContent;
                                    document.head.appendChild(s);
                                });
                                html = doc.body.innerHTML;
                            }

                            // Separate scripts from HTML before injecting
                            const tmp = document.createElement('div');
                            tmp.innerHTML = html;
                            const scriptSources = [];
                            tmp.querySelectorAll('script').forEach(s => {
                                scriptSources.push({ text: s.textContent, attrs: Array.from(s.attributes).map(a => [a.name, a.value]) });
                                s.remove();
                            });
                            // Move inline style tags to head
                            tmp.querySelectorAll('style').forEach(style => {
                                const s = document.createElement('style');
                                s.dataset.canvas = '1';
                                s.textContent = style.textContent;
                                document.head.appendChild(s);
                                style.remove();
                            });

                            // Inject non-script HTML first
                            container.innerHTML = tmp.innerHTML;

                            // Execute scripts via new Function() — reliable execution + proper error catching
                            // Wrapped in IIFE scope so const/let declarations don't clash across renders
                            for (const src of scriptSources) {
                                if (src.text) {
                                    try { (new Function(src.text))(); }
                                    catch (e) { console.error('canvas script error:', e); }
                                }
                            }
                        }

                        async function loadCanvas() {
                            try {
                                const sdk = window._traitsSDK;
                                if (!sdk) return;
                                const res = await sdk.call('sys.canvas', ['get']);
                                const content = res?.result?.content || res?.content || '';
                                renderCanvas(content);
                            } catch(e) { console.warn('canvas load:', e); }
                        }

                        // Listen for live updates from voice/SDK
                        window.addEventListener('traits-canvas-update', (e) => {
                            const content = e.detail?.content;
                            if (content !== undefined) {
                                renderCanvas(content);
                            } else {
                                loadCanvas();
                            }
                        });

                        // Clear button
                        document.getElementById('btnClear').addEventListener('click', async () => {
                            const sdk = window._traitsSDK;
                            if (sdk) await sdk.call('sys.canvas', ['clear']);
                            renderCanvas('');
                        });

                        // View Source toggle
                        document.getElementById('btnSource').addEventListener('click', async () => {
                            sourceMode = !sourceMode;
                            const btn = document.getElementById('btnSource');
                            if (sourceMode) {
                                const sdk = window._traitsSDK;
                                const res = sdk ? await sdk.call('sys.canvas', ['get']) : null;
                                const content = res?.result?.content || res?.content || '';
                                container.innerHTML = '<pre style="white-space:pre-wrap;word-break:break-all;color:#888;font-size:13px;padding:20px;"></pre>';
                                container.querySelector('pre').textContent = content || '(empty)';
                                btn.textContent = 'Live View';
                            } else {
                                btn.textContent = 'View Source';
                                loadCanvas();
                            }
                        });

                        // Initial load
                        loadCanvas();
                    })();
                "#)) }
            }
        }
    };
    Value::String(markup.into_string())
}
