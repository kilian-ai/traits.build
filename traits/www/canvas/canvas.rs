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
                            padding: 8px 16px; border-bottom: 1px solid var(--border);
                            background: #111; height: 40px; box-sizing: border-box;
                        }
                        .canvas-header h1 { font-size: 14px; font-weight: 500; margin: 0; }
                        .canvas-header h1 .accent { color: var(--accent); }
                        .canvas-header .actions { display: flex; gap: 6px; align-items: center; }
                        .canvas-header button {
                            background: transparent; border: 1px solid var(--border);
                            color: var(--fg); padding: 3px 10px; border-radius: 4px;
                            cursor: pointer; font-size: 11px;
                        }
                        .canvas-header button:hover { border-color: var(--accent); color: var(--accent); }
                        .canvas-header button.save-btn { border-color: #2a5; color: #2a5; }
                        .canvas-header button.save-btn:hover { border-color: #3c8; color: #3c8; }

                        /* Project bar */
                        #project-bar {
                            display: none; padding: 4px 16px; background: #0d0d0d;
                            border-bottom: 1px solid var(--border);
                            overflow-x: auto; white-space: nowrap; height: 28px; box-sizing: border-box;
                        }
                        #project-bar.has-projects { display: flex; gap: 6px; align-items: center; }
                        #project-bar .project-chip {
                            display: inline-flex; align-items: center; gap: 4px;
                            padding: 2px 8px; border-radius: 4px; font-size: 10px;
                            background: #181818; border: 1px solid var(--border);
                            color: #aaa; cursor: pointer; flex-shrink: 0;
                        }
                        #project-bar .project-chip:hover { border-color: var(--accent); color: var(--accent); }
                        #project-bar .project-chip .del {
                            color: #555; cursor: pointer; font-size: 12px; margin-left: 2px;
                        }
                        #project-bar .project-chip .del:hover { color: #f44; }
                        #project-bar .project-label { font-size: 10px; color: #555; margin-right: 4px; flex-shrink: 0; }

                        #canvas-frame {
                            width: 100%; border: none; background: #0a0a0a;
                        }
                        .canvas-empty {
                            display: flex; flex-direction: column; align-items: center;
                            justify-content: center; height: 60vh; color: #555;
                        }
                        .canvas-empty .icon { font-size: 48px; margin-bottom: 16px; opacity: 0.5; }
                        .canvas-empty p { font-size: 14px; text-align: center; line-height: 1.6; }
                        .canvas-empty code { color: var(--accent); font-size: 12px; }
                    "#))
                }
            }
            body {
                div .canvas-header {
                    h1 { "traits.build " span .accent { "canvas" } }
                    div .actions {
                        button #btnSave .save-btn { "Save As" }
                        button #btnReload { "Reload" }
                        button #btnClear { "Clear" }
                        button #btnSource { "Source" }
                    }
                }
                div #project-bar {}
                div #canvas-container {
                    div .canvas-empty #canvas-empty {
                        div .icon { "🎨" }
                        p {
                            "Canvas is empty." br;
                            "Ask the agent to build something, or use " code { "sys.canvas set \"<html>...\"" }
                        }
                    }
                    iframe #canvas-frame style="display:none" sandbox="allow-scripts allow-same-origin allow-popups allow-forms" {}
                }

                script { (PreEscaped(r#"
                    (function() {
                        const sdk = () => window._traitsSDK;
                        const container = document.getElementById('canvas-container');
                        const empty = document.getElementById('canvas-empty');
                        const frame = document.getElementById('canvas-frame');
                        const projectBar = document.getElementById('project-bar');
                        let sourceMode = false;

                        // ── Resize iframe to fill remaining viewport ──
                        function resizeFrame() {
                            const top = frame.getBoundingClientRect().top;
                            frame.style.height = (window.innerHeight - top) + 'px';
                        }
                        window.addEventListener('resize', resizeFrame);

                        // ── Inject traits SDK bridge into iframe ──
                        function injectBridge(iframeWindow) {
                            try {
                                iframeWindow.traits = {
                                    call: (path, args) => sdk()?.call(path, args || []),
                                    list: (ns) => sdk()?.call('sys.list', ns ? [ns] : []),
                                    info: (path) => sdk()?.call('sys.info', [path]),
                                    canvas: (action, content) => {
                                        const a = content !== undefined ? [action, content] : [action];
                                        return sdk()?.call('sys.canvas', a);
                                    },
                                    vfs: (action, ...a) => sdk()?.call('sys.vfs', [action, ...a]),
                                };
                            } catch(e) { /* cross-origin safety */ }
                        }

                        // ── Render SPA content in iframe ──
                        function renderCanvas(content) {
                            if (!content) {
                                frame.style.display = 'none';
                                frame.removeAttribute('srcdoc');
                                empty.style.display = 'flex';
                                return;
                            }
                            empty.style.display = 'none';
                            frame.style.display = 'block';
                            resizeFrame();
                            frame.srcdoc = content;
                            frame.onload = () => injectBridge(frame.contentWindow);
                        }

                        // ── Load canvas from VFS via sys.canvas ──
                        async function loadCanvas() {
                            try {
                                const s = sdk();
                                if (!s) return;
                                const res = await s.call('sys.canvas', ['get']);
                                const content = res?.result?.content || res?.content || '';
                                renderCanvas(content);
                            } catch(e) { console.warn('canvas load:', e); }
                        }

                        // ── Project management (VFS-backed) ──
                        async function renderProjectBar() {
                            try {
                                const s = sdk();
                                if (!s) return;
                                const res = await s.call('sys.canvas', ['projects']);
                                const projects = res?.result?.projects || res?.projects || [];
                                if (!projects.length) {
                                    projectBar.className = '';
                                    projectBar.innerHTML = '';
                                    return;
                                }
                                projectBar.className = 'has-projects';
                                projectBar.innerHTML = '<span class="project-label">Projects:</span>' +
                                    projects.map(function(name) {
                                        var escaped = name.replace(/"/g, '&quot;');
                                        return '<span class="project-chip" data-name="' + escaped + '">' +
                                            name +
                                            ' <span class="del" data-del="' + escaped + '">&times;</span>' +
                                            '</span>';
                                    }).join('');
                                projectBar.querySelectorAll('.project-chip').forEach(function(chip) {
                                    chip.addEventListener('click', function(e) {
                                        if (e.target.classList.contains('del')) return;
                                        loadProject(chip.dataset.name);
                                    });
                                });
                                projectBar.querySelectorAll('.del').forEach(function(del) {
                                    del.addEventListener('click', function(e) {
                                        e.stopPropagation();
                                        deleteProject(del.dataset.del);
                                    });
                                });
                            } catch(e) { console.warn('project bar:', e); }
                        }

                        async function loadProject(name) {
                            try {
                                const s = sdk();
                                if (!s) return;
                                await s.call('sys.canvas', ['load', name]);
                                await loadCanvas();
                            } catch(e) { console.warn('load project:', e); }
                        }

                        async function deleteProject(name) {
                            if (!confirm('Delete project "' + name + '"?')) return;
                            try {
                                const s = sdk();
                                if (s) await s.call('sys.canvas', ['delete_project', name]);
                                renderProjectBar();
                            } catch(e) { console.warn('delete project:', e); }
                        }

                        // ── Button handlers ──
                        document.getElementById('btnSave').addEventListener('click', async () => {
                            const s = sdk();
                            if (!s) return;
                            const res = await s.call('sys.canvas', ['get']);
                            const content = res?.result?.content || res?.content || '';
                            if (!content) { alert('Canvas is empty.'); return; }
                            const name = prompt('Project name:');
                            if (!name || !name.trim()) return;
                            await s.call('sys.canvas', ['save', name.trim()]);
                            renderProjectBar();
                        });

                        document.getElementById('btnReload').addEventListener('click', () => {
                            sourceMode = false;
                            document.getElementById('btnSource').textContent = 'Source';
                            loadCanvas();
                        });

                        document.getElementById('btnClear').addEventListener('click', async () => {
                            const s = sdk();
                            if (s) await s.call('sys.canvas', ['clear']);
                            renderCanvas('');
                        });

                        document.getElementById('btnSource').addEventListener('click', async () => {
                            sourceMode = !sourceMode;
                            const btn = document.getElementById('btnSource');
                            if (sourceMode) {
                                const s = sdk();
                                const res = s ? await s.call('sys.canvas', ['get']) : null;
                                const content = res?.result?.content || res?.content || '';
                                frame.style.display = 'none';
                                empty.style.display = 'none';
                                // Show source in a pre element
                                let pre = container.querySelector('#canvas-source');
                                if (!pre) {
                                    pre = document.createElement('pre');
                                    pre.id = 'canvas-source';
                                    pre.style.cssText = 'white-space:pre-wrap;word-break:break-all;color:#888;font-size:12px;padding:16px;margin:0;overflow:auto;height:calc(100vh - 80px);background:#0a0a0a;';
                                    container.appendChild(pre);
                                }
                                pre.textContent = content || '(empty)';
                                pre.style.display = 'block';
                                btn.textContent = 'Live';
                            } else {
                                const pre = container.querySelector('#canvas-source');
                                if (pre) pre.style.display = 'none';
                                btn.textContent = 'Source';
                                loadCanvas();
                            }
                        });

                        // ── Listen for live updates ──
                        window.addEventListener('traits-canvas-update', (e) => {
                            if (sourceMode) return;
                            const content = e.detail?.content;
                            if (content !== undefined) {
                                renderCanvas(content);
                            } else {
                                loadCanvas();
                            }
                        });

                        // ── Initial load ──
                        loadCanvas();
                        renderProjectBar();

                        // ── Cleanup on navigation ──
                        window._pageCleanup = () => {
                            frame.removeAttribute('srcdoc');
                            frame.style.display = 'none';
                        };
                    })();
                "#)) }
            }
        }
    };
    Value::String(markup.into_string())
}
