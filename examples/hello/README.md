# examples/hello — Component Model, every language

A `hello.<lang>` trait implementation in every language that compiles to a
WebAssembly Component Model component. All seven implementations expose the
**same WIT contract** and become a callable trait via `gen-trait`:

```bash
traits call hello.rust   "world"
traits call hello.go     "world"
traits call hello.javascript "world"
traits call hello.python "world"
traits call hello.c      "world"
traits call hello.cpp    "world"
traits call hello.zig    "world"
```

## Shared WIT contract

Every implementation exports the same shape — a single function returning a
JSON string wrapped in `result<string, string>`:

```wit
package hello:lang@0.1.0;

interface hello {
    hello: func(name: string) -> result<string, string>;
}

world hello-world {
    export hello;
}
```

## Build everything

```bash
bash examples/hello/build.sh
```

The script auto-detects each toolchain. Missing toolchains are skipped with a
note pointing at the installer.

## Toolchains

| Language   | Toolchain                | Install (macOS)                                |
|------------|--------------------------|------------------------------------------------|
| Rust       | `cargo-component`        | `cargo install cargo-component --locked`       |
| Go         | TinyGo + `wit-bindgen-go`| `brew install tinygo` + `go install go.bytecodealliance.org/cmd/wit-bindgen-go@latest` |
| JavaScript | `jco` (`@bytecodealliance/jco`) | already vendored in node — uses `npx jco`     |
| Python     | `componentize-py`        | `pip3 install --user componentize-py`          |
| C          | wasi-sdk + `wit-bindgen` | `brew install wasi-sdk` + `cargo install wit-bindgen-cli` |
| C++        | wasi-sdk + `wit-bindgen` | (same as C)                                    |
| Zig        | Zig 0.14+ + `wit-bindgen`| `brew install zig` + `cargo install wit-bindgen-cli`      |

## After build

Each `*.component.wasm` is registered via `gen-trait`:

```bash
./tools/gen-trait/target/release/gen-trait \
    examples/hello/<lang>/hello-<lang>.component.wasm \
    --as hello.<lang>
```
