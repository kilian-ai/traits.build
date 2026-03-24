use serde_json::Value;
use maud::{html, DOCTYPE, PreEscaped};

pub fn wasm_page(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "traits.build — WASM Kernel" }
                link rel="stylesheet" href="/static/www/wasm/wasm.css" {}
            }
            body {
                div.container {
                    header {
                        h1 { "traits.build " span.accent { "wasm" } }
                        p.subtitle { "Kernel running in your browser via WebAssembly" }
                        div #status .status { "Loading WASM module..." }
                    }

                    div.panels {
                        // Left: Trait browser
                        div.browser-panel {
                            div.search-wrap {
                                input #traitSearch type="text" placeholder="Search traits..." autocomplete="off" {}
                            }
                            div.filter-bar {
                                label {
                                    input #filterCallable type="checkbox" {}
                                    " WASM only"
                                }
                                span #traitCount .count {}
                            }
                            div #traitList .trait-list {}
                        }

                        // Right: Detail + Call panel
                        div.detail-panel {
                            div #traitDetail .detail hidden="true" {
                                div.detail-header {
                                    h2 #detailName {}
                                    span #detailVersion .version {}
                                    span #detailCallable .badge {}
                                }
                                p #detailDesc .desc {}

                                div #detailParams .params-section {}

                                div #callSection .call-section hidden="true" {
                                    h3 { "Call" }
                                    div #argsForm .args-form {}
                                    div.actions {
                                        button #btnCall .primary { "Run" }
                                        span #elapsed .elapsed {}
                                    }
                                }

                                div #resultSection .result-section hidden="true" {
                                    div.result-header {
                                        h3 { "Result" }
                                        button #btnCopy .secondary { "Copy" }
                                    }
                                    pre #resultOutput .result {}
                                }
                            }

                            div #welcomePanel .welcome {
                                h2 { "Welcome" }
                                p { "The traits kernel is loaded directly in your browser as a WebAssembly module." }
                                p { "Select a trait from the list to see its details. Traits marked "
                                    span.badge.callable { "WASM" }
                                    " run locally in your browser. Traits marked "
                                    span.badge.server { "Server" }
                                    " are called transparently via the REST API."
                                }
                                div #kernelInfo .kernel-info {}
                            }
                        }
                    }
                }
                (PreEscaped(r#"<script type="module" src="/static/www/wasm/wasm.js"></script>"#))
            }
        }
    };
    Value::String(markup.into_string())
}
