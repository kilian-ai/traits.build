use maud::{html, DOCTYPE};
use serde_json::Value;

pub fn munal(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Munal OS / Browser Integration - traits.build" }
                style {
                    "
                    :root {
                        --bg: #0b1018;
                        --panel: #121b2a;
                        --line: #273652;
                        --text: #dbe7ff;
                        --muted: #92a1bf;
                        --ok: #33c48d;
                        --warn: #f6c358;
                        --bad: #ff7a7a;
                    }
                    * { box-sizing: border-box; }
                    body {
                        margin: 0;
                        font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
                        background: radial-gradient(1200px 700px at 80% -10%, #1f2f50 0%, var(--bg) 60%);
                        color: var(--text);
                    }
                    .wrap {
                        max-width: 980px;
                        margin: 0 auto;
                        padding: 24px;
                    }
                    .hero {
                        border: 1px solid var(--line);
                        background: linear-gradient(180deg, #152038, #0f1828);
                        border-radius: 14px;
                        padding: 18px;
                        margin-bottom: 16px;
                    }
                    h1 { margin: 0 0 8px; font-size: 26px; }
                    p { color: var(--muted); line-height: 1.5; }
                    .row {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
                        gap: 12px;
                        margin: 12px 0;
                    }
                    .card {
                        border: 1px solid var(--line);
                        background: var(--panel);
                        border-radius: 12px;
                        padding: 14px;
                    }
                    .badge {
                        display: inline-block;
                        border-radius: 999px;
                        border: 1px solid var(--line);
                        padding: 4px 10px;
                        font-size: 12px;
                        color: var(--muted);
                        margin-right: 8px;
                    }
                    .ok { color: var(--ok); }
                    .warn { color: var(--warn); }
                    .bad { color: var(--bad); }
                    code {
                        background: #0b1323;
                        border: 1px solid var(--line);
                        border-radius: 6px;
                        padding: 2px 6px;
                        color: #b8ccff;
                    }
                    .actions { margin-top: 12px; display: flex; flex-wrap: wrap; gap: 8px; }
                    a.btn {
                        text-decoration: none;
                        color: var(--text);
                        border: 1px solid var(--line);
                        background: #15243a;
                        border-radius: 8px;
                        padding: 8px 10px;
                        font-size: 13px;
                    }
                    ul { padding-left: 18px; color: var(--muted); }
                    li { margin-bottom: 7px; }
                    "
                }
            }
            body {
                div class="wrap" {
                    div class="hero" {
                        span class="badge" { "new page" }
                        span class="badge" { "adjacent to Linux" }
                        h1 { "Munal OS browser bootstrap" }
                        p {
                            "This page tracks direct browser bring-up of Askannz/munal-os from traits.build. "
                            "The Linux page remains available while we port kernel runtime assumptions."
                        }
                    }

                    div class="row" {
                        div class="card" {
                            h3 { "Compatibility status" }
                            p {
                                span class="bad" { "Kernel target today: x86_64-unknown-uefi (QEMU/UEFI)" }
                                br;
                                span class="warn" { "Browser target requested: wasm32 browser kernel" }
                            }
                            p { "Current blocker is architectural: UEFI + x86_64 hardware assumptions in the Munal kernel." }
                        }
                        div class="card" {
                            h3 { "What is already done" }
                            ul {
                                li { "Feature branch created: " code { "feat/munal-os-browser-page" } }
                                li { "Upstream repo cloned and build pipeline audited" }
                                li { "SPA route added for this dedicated integration page" }
                            }
                        }
                    }

                    div class="card" {
                        h3 { "Next porting milestones" }
                        ul {
                            li { "Extract a browser host layer replacing UEFI/PCI/virtio assumptions" }
                            li { "Compile Munal kernel core to wasm32 with browser driver shims" }
                            li { "Boot shell-first mode on this page, then enable desktop surface" }
                        }
                    }

                    div class="card" {
                        h3 { "Launch points" }
                        p { "Use these while the direct kernel port is in progress:" }
                        div class="actions" {
                            a class="btn" href="https://github.com/Askannz/munal-os" target="_blank" { "Open munal-os repo" }
                            a class="btn" href="#/linux" { "Open Linux/WASM shell" }
                            a class="btn" href="#/testing" { "Open runtime testing page" }
                        }
                    }
                }
            }
        }
    };

    Value::String(markup.into_string())
}
