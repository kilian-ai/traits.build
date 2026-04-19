use serde_json::Value;
use maud::{html, DOCTYPE};

pub fn testing(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "traits.build — Testing abstraction layer" }
                link rel="stylesheet" href="/static/www/testing/testing.css" {}
            }
            body {
                div.container {
                    header {
                        h1 { "traits-runtimes " span.accent { "testing" } }
                        p.subtitle { "Comprehensive test suite for the I/O abstraction layer" }
                    }

                    nav.test-nav {
                        button.nav-btn.active data-section="overview" { "Overview" }
                        button.nav-btn data-section="examples" { "Examples" }
                        button.nav-btn data-section="filesystem" { "Filesystem" }
                        button.nav-btn data-section="sockets" { "Sockets" }
                        button.nav-btn data-section="runtime" { "Runtime" }
                        button.nav-btn data-section="relay" { "Relay API" }
                    }

                    // Overview Section
                    section #overview.test-section.active {
                        div.section-header {
                            h2 { "Abstraction Layer Overview" }
                        }

                        div.card {
                            h3 { "What is traits-runtimes?" }
                            p {
                                "A Rust-level I/O abstraction layer that enables production services "
                                "to compile to native, WASM, and WASI targets without code changes."
                            }
                            ul {
                                li { strong { "Same code" } " runs on multiple platforms" }
                                li { strong { "Socket abstractions" } " for network I/O" }
                                li { strong { "Filesystem abstractions" } " for file operations" }
                                li { strong { "Timer abstractions" } " for async delays" }
                                li { strong { "Process abstractions" } " for command execution" }
                            }
                        }

                        div.card {
                            h3 { "Three Backend Implementations" }
                            div.backends {
                                div.backend {
                                    h4 { "Native" }
                                    p { "Uses tokio for production-ready performance" }
                                    code { "native::init_native_runtime()" }
                                }
                                div.backend {
                                    h4 { "WASM" }
                                    p { "Relay bridge for browser execution" }
                                    code { "wasm::init_wasm_runtime(url, code)" }
                                }
                                div.backend {
                                    h4 { "WASI" }
                                    p { "WebAssembly System Interface" }
                                    code { "wasi::init_wasi_runtime()" }
                                }
                            }
                        }

                        div.card {
                            h3 { "Testing Strategy" }
                            p { "The abstraction layer is validated through three independent examples:" }
                            ol {
                                li { strong { "file_ops" } " — Filesystem operations (read, write, mkdir, list)" }
                                li { strong { "socket_echo" } " — Network I/O (bind, listen, connect, send, receive)" }
                                li { strong { "multi_target_test" } " — Cross-platform compilation (same code, multiple targets)" }
                            }
                        }
                    }

                    // Examples Section
                    section #examples.test-section {
                        div.section-header {
                            h2 { "Running the Examples" }
                            p { "Follow these commands to build and run each example:" }
                        }

                        div.card {
                            h3 { "1. Filesystem Operations" }
                            p { "Tests: create, read, write, mkdir, list_dir, metadata, remove_file" }
                            div.code-block {
                                code {
                                    "# Build the abstraction layer with examples\n"
                                    "cargo build -p traits-runtimes\n\n"
                                    "# Run filesystem test\n"
                                    "cargo run -p traits-runtimes --example file_ops\n\n"
                                    "# Expected output:\n"
                                    "# === All filesystem tests passed! ==="
                                }
                            }
                            details {
                                summary { "What does this test?" }
                                ul {
                                    li { "Creates a directory in /tmp/traits_runtime_test" }
                                    li { "Writes test files using abstraction Filesystem" }
                                    li { "Reads files back and validates content" }
                                    li { "Tests metadata retrieval (file size, permissions)" }
                                    li { "Lists directories and counts entries" }
                                    li { "Removes files and verifies cleanup" }
                                }
                            }
                        }

                        div.card {
                            h3 { "2. Socket Operations" }
                            p { "Tests: bind, listen, accept, connect, send, receive" }
                            div.code-block {
                                code {
                                    "# Run socket echo server test\n"
                                    "cargo run -p traits-runtimes --example socket_echo\n\n"
                                    "# Server listens on 127.0.0.1:19999\n"
                                    "# Client connects and sends: \"Hello from client!\"\n"
                                    "# Server echoes back: \"ECHO: Hello from client!\"\n\n"
                                    "# Expected output:\n"
                                    "# === All socket tests passed! ==="
                                }
                            }
                            details {
                                summary { "What does this test?" }
                                ul {
                                    li { "Creates a listener on a local socket" }
                                    li { "Retrieves local address and verifies binding" }
                                    li { "Accepts incoming connections" }
                                    li { "Sends and receives messages" }
                                    li { "Validates message round-trip integrity" }
                                }
                            }
                        }

                        div.card {
                            h3 { "3. Multi-Target Compilation" }
                            p { "Validates same code compiles for native and WASM targets" }
                            div.code-block {
                                code {
                                    "# Run multi-target test\n"
                                    "cargo run -p traits-runtimes --example multi_target_test\n\n"
                                    "# To compile for WASM:\n"
                                    "cargo build -p traits-runtimes --example multi_target_test \\\n"
                                    "  --target wasm32-unknown-unknown\n\n"
                                    "# Expected output:\n"
                                    "# === All multi-target tests passed! ==="
                                }
                            }
                            details {
                                summary { "What does this test?" }
                                ul {
                                    li { "Runs filesystem operations on current target" }
                                    li { "Runs timer operations (cross-platform)" }
                                    li { "Runs process operations (native-only stub on WASM)" }
                                    li { "Validates conditional compilation works" }
                                    li { "Confirms same source compiles on multiple targets" }
                                }
                            }
                        }
                    }

                    // Filesystem Section
                    section #filesystem.test-section {
                        div.section-header {
                            h2 { "Filesystem Abstraction Tests" }
                        }

                        div.card {
                            h3 { "Test Matrix" }
                            table.test-matrix {
                                tr {
                                    th { "Operation" }
                                    th { "Native" }
                                    th { "WASM (Relay)" }
                                    th { "WASI" }
                                    th { "Notes" }
                                }
                                tr {
                                    td { "create(path)" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Create new file" }
                                }
                                tr {
                                    td { "open(path)" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Open existing file" }
                                }
                                tr {
                                    td { "read_to_string()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Read entire file" }
                                }
                                tr {
                                    td { "write()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Write/overwrite file" }
                                }
                                tr {
                                    td { "mkdir()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Create directory" }
                                }
                                tr {
                                    td { "remove_file()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Delete file" }
                                }
                                tr {
                                    td { "remove_dir()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Delete directory" }
                                }
                                tr {
                                    td { "list_dir()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "List directory contents" }
                                }
                                tr {
                                    td { "metadata()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Get file info" }
                                }
                            }
                        }

                        div.card {
                            h3 { "Path Sandboxing (Native)" }
                            p { "The native backend enforces security constraints:" }
                            ul {
                                li { strong { "Relative paths only" } " — Prevent directory traversal" }
                                li { strong { "Root directory constraint" } " — All paths resolved relative to root" }
                                li { strong { "Backslash rejection" } " — Reject `..` escapes" }
                                li { strong { "Absolute path rejection" } " — Cannot access system files" }
                            }
                        }
                    }

                    // Sockets Section
                    section #sockets.test-section {
                        div.section-header {
                            h2 { "Socket Abstraction Tests" }
                        }

                        div.card {
                            h3 { "Test Matrix" }
                            table.test-matrix {
                                tr {
                                    th { "Operation" }
                                    th { "Native" }
                                    th { "WASM (Relay)" }
                                    th { "WASI" }
                                    th { "Notes" }
                                }
                                tr {
                                    td { "bind(addr)" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Create listening socket" }
                                }
                                tr {
                                    td { "connect(addr)" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Establish connection" }
                                }
                                tr {
                                    td { "accept()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Wait for incoming connection" }
                                }
                                tr {
                                    td { "read()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Receive data" }
                                }
                                tr {
                                    td { "write_all()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Send all data" }
                                }
                                tr {
                                    td { "peer_addr()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Get peer address" }
                                }
                                tr {
                                    td { "local_addr()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Get local address" }
                                }
                                tr {
                                    td { "shutdown()" }
                                    td { "✓" } td { "✓" } td { "✓" }
                                    td { "Close connection" }
                                }
                            }
                        }

                        div.card {
                            h3 { "Echo Server Protocol" }
                            p { "The socket_echo example implements a simple protocol:" }
                            ol {
                                li { "Server creates listener on 127.0.0.1:19999" }
                                li { "Client connects to server" }
                                li { "Client sends message: " code { "Hello from client!" } }
                                li { "Server receives and prefixes with " code { "ECHO: " } }
                                li { "Server sends back: " code { "ECHO: Hello from client!" } }
                                li { "Client verifies message integrity" }
                            }
                        }
                    }

                    // Runtime Section
                    section #runtime.test-section {
                        div.section-header {
                            h2 { "Runtime Context & Initialization" }
                        }

                        div.card {
                            h3 { "How to Use the Abstraction" }
                            p { "The abstraction is initialized once at startup:" }
                            div.code-block {
                                code {
                                    "use traits_runtimes::*;\n\n"
                                    "#[tokio::main]\n"
                                    "async fn main() {\n"
                                    "    // Choose backend at startup\n"
                                    "    let runtime = native::init_native_runtime(Some(\n"
                                    "        PathBuf::from(\"/tmp\")\n"
                                    "    ));\n"
                                    "    init_runtime(runtime)?;\n\n"
                                    "    // Use abstraction in your service\n"
                                    "    let rt = get_runtime()?;\n"
                                    "    let mut socket = rt.socket_factory\n"
                                    "        .bind(\"127.0.0.1:8080\".parse()?)\n"
                                    "        .await?;\n"
                                    "}\n"
                                }
                            }
                        }

                        div.card {
                            h3 { "Backend Switching" }
                            p { "Switch backends without changing service code:" }
                            div.code-block {
                                code {
                                    "// Native backend (production)\n"
                                    "let rt = native::init_native_runtime(Some(root));\n\n"
                                    "// WASM backend (browser with relay)\n"
                                    "let rt = wasm::init_wasm_runtime(\n"
                                    "    \"https://relay.traits.build\".into(),\n"
                                    "    \"ABCD\".into()  // pairing code\n"
                                    ");\n\n"
                                    "// WASI backend (serverless)\n"
                                    "let rt = wasi::init_wasi_runtime();\n"
                                }
                            }
                        }

                        div.card {
                            h3 { "Error Handling" }
                            p { "Comprehensive error enum for all I/O failures:" }
                            ul {
                                li { code { "RuntimeError::Io" } " — I/O operation failed" }
                                li { code { "RuntimeError::NotConnected" } " — No active relay connection" }
                                li { code { "RuntimeError::Timeout" } " — Operation exceeded timeout" }
                                li { code { "RuntimeError::Unsupported" } " — Operation not supported on target" }
                                li { code { "RuntimeError::InvalidState" } " — Runtime not initialized" }
                                li { code { "RuntimeError::PathNotAllowed" } " — Path violates sandbox" }
                            }
                        }
                    }

                    // Relay API Section
                    section #relay.test-section {
                        div.section-header {
                            h2 { "Relay API Testing" }
                            p { "Test WASM backend communication with the relay server" }
                        }

                        div.card {
                            h3 { "Relay Endpoints" }
                            table.test-matrix {
                                tr {
                                    th { "Endpoint" }
                                    th { "Purpose" }
                                    th { "Backend" }
                                }
                                tr {
                                    td { "/relay/register" }
                                    td { "Get pairing code (4 char)" }
                                    td { "HTTP POST" }
                                }
                                tr {
                                    td { "/relay/call" }
                                    td { "Sync RPC call" }
                                    td { "HTTP POST" }
                                }
                                tr {
                                    td { "/relay/poll" }
                                    td { "Long-poll for requests" }
                                    td { "HTTP GET" }
                                }
                                tr {
                                    td { "/relay/respond" }
                                    td { "Send response back" }
                                    td { "HTTP POST" }
                                }
                                tr {
                                    td { "/relay/status" }
                                    td { "Check if code active" }
                                    td { "HTTP GET" }
                                }
                            }
                        }

                        div.card {
                            h3 { "Test Relay with curl" }
                            div.code-block {
                                code {
                                    "# 1. Register pairing code\n"
                                    "CODE=$(curl -s -X POST \\\n"
                                    "  https://relay.traits.build/relay/register \\\n"
                                    "  | jq -r '.code')\n"
                                    "echo \"Code: $CODE\"\n\n"
                                    "# 2. Execute shell command in relay\n"
                                    "curl -s -X POST \\\n"
                                    "  https://relay.traits.build/relay/call \\\n"
                                    "  -H 'Content-Type: application/json' \\\n"
                                    "  -d \"{\\\"code\\\":\\\"$CODE\\\",\\\n"
                                    "       \\\"path\\\":\\\"linux.exec\\\",\\\n"
                                    "       \\\"args\\\":[\\\"pwd\\\"]}\"\n\n"
                                    "# 3. Check status\n"
                                    "curl -s https://relay.traits.build/relay/status?code=$CODE\n"
                                }
                            }
                        }

                        div.card {
                            h3 { "Relay-Backed Trait Paths" }
                            ul {
                                li { code { "linux.exec" } " — Execute shell command" }
                                li { code { "linux.vfs.read" } " — Read file" }
                                li { code { "linux.vfs.write" } " — Write file" }
                                li { code { "linux.vfs.list" } " — List directory" }
                                li { code { "linux.vfs.mkdir" } " — Create directory" }
                                li { code { "linux.vfs.delete" } " — Remove file" }
                            }
                        }
                    }

                    footer.footer {
                        p {
                            "For more information, see the "
                            a href="https://github.com/kilian-ai/traits.build" target="_blank" {
                                "traits.build repository"
                            }
                        }
                    }
                }
                script src="/static/www/testing/testing.js" {}
            }
        }
    };
    Value::String(markup.into_string())
}
