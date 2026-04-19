use maud::{html, DOCTYPE};
use serde_json::Value;

pub fn wanix(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Wanix Shell - traits.build" }
                style {
                    "
                    :root {
                        --bg: #0b111d;
                        --panel: #111a2b;
                        --line: #2a3957;
                        --text: #e5efff;
                        --muted: #95a6c9;
                        --accent: #6de0b5;
                    }
                    * { box-sizing: border-box; }
                    body {
                        margin: 0;
                        font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial;
                        background: radial-gradient(1200px 760px at 12% -10%, #1a2b48 0%, var(--bg) 60%);
                        color: var(--text);
                    }
                    .wrap {
                        max-width: 1240px;
                        margin: 0 auto;
                        padding: 20px;
                    }
                    .hero {
                        border: 1px solid var(--line);
                        background: linear-gradient(180deg, #15233d, #101a2e);
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
                        background: #1a2840;
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
                        background: #0a0f19;
                    }
                    .frame-head {
                        display: flex;
                        align-items: center;
                        justify-content: space-between;
                        gap: 12px;
                        padding: 10px 12px;
                        border-bottom: 1px solid var(--line);
                        background: #101a2c;
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
                        background: #080c14;
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
                        div class="badge" { "wanix" }
                        div class="badge" { "browser shell" }
                        h1 { "Wanix Shell Runtime" }
                        p {
                            "This page embeds the live Wanix runtime shell from wanix.run directly in the SPA, "
                            "so you can run and test the shell environment adjacent to Linux/WASM and Munal pages."
                        }
                        div class="actions" {
                            a class="btn" href="https://wanix.run" target="_blank" rel="noopener noreferrer" { "Open Wanix in new tab" }
                            a class="btn" href="#/linux" { "Open Linux/WASM" }
                            a class="btn" href="#/testing" { "Open Testing" }
                        }
                    }

                    div class="frame-wrap" {
                        div class="frame-head" {
                            span class="ok" { "Shell active via embedded wanix.run" }
                            span { "Use this for runtime experimentation while we evaluate deeper native integration." }
                        }
                        iframe
                            title="Wanix Shell"
                            src="https://wanix.run"
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
