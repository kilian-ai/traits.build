use maud::{html, DOCTYPE};
use serde_json::Value;

pub fn apptron(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Apptron Shell - traits.build" }
                style {
                    "
                    :root {
                        --bg: #0b121c;
                        --panel: #121f33;
                        --line: #2b4268;
                        --text: #e6f0ff;
                        --muted: #97abcf;
                        --accent: #6dc2ff;
                    }
                    * { box-sizing: border-box; }
                    body {
                        margin: 0;
                        font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial;
                        background: radial-gradient(1200px 760px at 15% -20%, #20395c 0%, var(--bg) 58%);
                        color: var(--text);
                    }
                    .wrap {
                        max-width: 1240px;
                        margin: 0 auto;
                        padding: 20px;
                    }
                    .hero {
                        border: 1px solid var(--line);
                        background: linear-gradient(180deg, #1a2f4d, #13253f);
                        border-radius: 14px;
                        padding: 16px;
                        margin-bottom: 14px;
                    }
                    h1 {
                        margin: 0 0 8px;
                        font-size: 26px;
                    }
                    p {
                        margin: 0;
                        color: var(--muted);
                        line-height: 1.5;
                    }
                    .badge {
                        display: inline-block;
                        border-radius: 999px;
                        border: 1px solid var(--line);
                        padding: 4px 10px;
                        font-size: 12px;
                        color: var(--muted);
                        margin-right: 8px;
                        margin-bottom: 8px;
                    }
                    .actions {
                        margin-top: 12px;
                        display: flex;
                        gap: 8px;
                        flex-wrap: wrap;
                    }
                    .btn {
                        text-decoration: none;
                        border: 1px solid var(--line);
                        background: #1f3555;
                        color: var(--text);
                        padding: 8px 10px;
                        border-radius: 8px;
                        font-size: 13px;
                    }
                    .btn:hover { border-color: var(--accent); }
                    .frame-wrap {
                        border: 1px solid var(--line);
                        border-radius: 14px;
                        overflow: hidden;
                        background: #090f19;
                    }
                    .frame-head {
                        display: flex;
                        align-items: center;
                        justify-content: space-between;
                        gap: 12px;
                        padding: 10px 12px;
                        border-bottom: 1px solid var(--line);
                        background: #111f34;
                        font-size: 13px;
                    }
                    .ok {
                        color: var(--accent);
                        font-weight: 600;
                    }
                    iframe {
                        width: 100%;
                        height: calc(100vh - 240px);
                        min-height: 620px;
                        border: 0;
                        display: block;
                        background: #070b13;
                    }
                    @media (max-width: 720px) {
                        .wrap { padding: 12px; }
                        iframe {
                            height: calc(100vh - 220px);
                            min-height: 520px;
                        }
                    }
                    "
                }
            }
            body {
                div class="wrap" {
                    div class="hero" {
                        div class="badge" { "apptron" }
                        div class="badge" { "forked" }
                        div class="badge" { "linux shell" }
                        h1 { "Apptron Runtime Shell" }
                        p {
                            "This page runs a locally hosted Apptron runtime shell from traits.build static assets, "
                            "including vendored Apptron bundle + runtime so it runs in our own system."
                        }
                        div class="actions" {
                            a class="btn" href="/static/apptron/index.html" target="_blank" rel="noopener noreferrer" { "Open local Apptron shell" }
                            a class="btn" href="#/wanix" { "Open Wanix" }
                            a class="btn" href="#/linux" { "Open Linux/WASM" }
                            a class="btn" href="#/testing" { "Open Testing" }
                        }
                    }

                    div class="frame-wrap" {
                        div class="frame-head" {
                            span class="ok" { "Shell active via local /static/apptron runtime" }
                            span { "Vendored Apptron assets + sys bundle hosted by traits.build." }
                        }
                        iframe
                            title="Apptron Shell"
                            src="/static/apptron/index.html"
                            loading="eager"
                            allow="clipboard-read; clipboard-write"
                            referrerpolicy="strict-origin-when-cross-origin" {}
                    }
                }
            }
        }
    };

    Value::String(markup.into_string())
}
