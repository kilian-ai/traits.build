use serde_json::Value;
use maud::{html, DOCTYPE};

pub fn playground(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "traits.build — Playground" }
                link rel="stylesheet" href="/static/www/playground/playground.css" {}
            }
            body {
                div.container data-trait="sys.list" data-handler="initPlayground" {
                    header {
                        h1 { "traits.build " span.accent { "playground" } }
                        p.subtitle { "Call any trait interactively" }
                    }

                    // Trait selector
                    div.selector {
                        div.search-wrap {
                            input #traitSearch type="text" placeholder="Search traits..." autocomplete="off" {}
                            div #traitDropdown .dropdown {}
                        }
                    }

                    // Trait info + params form (hidden until selected)
                    div #traitPanel .panel hidden="true" {
                        div.trait-header {
                            h2 #traitName {}
                            span #traitDesc .desc {}
                        }
                        div #paramsForm .params {}
                        div.actions {
                            button #btnRun .primary { "Run" }
                            span #elapsed .elapsed {}
                        }
                    }

                    // Result area
                    div #resultPanel .result-panel hidden="true" {
                        div.result-header {
                            h3 { "Result" }
                            button #btnCopy .secondary { "Copy" }
                        }
                        pre #resultOutput .result {}
                    }
                }
                script src="/static/www/playground/playground.js" {}
            }
        }
    };
    Value::String(markup.into_string())
}

