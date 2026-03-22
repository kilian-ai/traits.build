---
sidebar_position: 2
---

# Getting Started

## Prerequisites

- **Rust** (stable, 1.75+)
- **Git**

No Node.js, Python, or external dependencies needed at runtime.

## Clone and build

```bash
git clone https://github.com/kilian-ai/traits.build.git
cd traits.build
cargo build --release
```

The build system (`build.rs`) automatically:
1. Discovers all `.trait.toml` files in `traits/`
2. Generates dispatch tables and module declarations
3. Computes version strings and checksums
4. Compiles everything into a single binary at `target/release/traits`

## Run the server

```bash
./target/release/traits serve --port 8090
```

You'll see output like:

```
INFO  Loaded 28 trait definitions
INFO  Page route '/' → www.traits.build
INFO  Page route '/admin' → www.admin
INFO  Page route '/docs/api' → www.docs.api
INFO  Starting Traits server on port 8090 (3 page routes)
```

## Try the CLI

```bash
# List all traits
./target/release/traits list

# Call a trait
./target/release/traits checksum hash "hello"

# Get trait info
./target/release/traits info sys.checksum

# Run tests
./target/release/traits test_runner '*'
```

## Try the REST API

```bash
# Health check
curl http://127.0.0.1:8090/health

# List traits (tree view)
curl http://127.0.0.1:8090/traits

# Call a trait
curl -X POST http://127.0.0.1:8090/traits/sys/checksum \
  -H 'Content-Type: application/json' \
  -d '{"args": ["hash", "test"]}'

# Get trait info
curl http://127.0.0.1:8090/traits/sys/checksum
```

## Browse the documentation

- **Landing page:** [http://127.0.0.1:8090/](http://127.0.0.1:8090/)
- **API docs (Redoc):** [http://127.0.0.1:8090/docs/api](http://127.0.0.1:8090/docs/api)
- **OpenAPI spec:** `POST http://127.0.0.1:8090/traits/sys/openapi`

## Next steps

- [Architecture](#architecture) — understand how the kernel works
- [Trait Definition](#trait-definition) — learn the `.trait.toml` format
- [Creating Traits](#creating-traits) — add your own traits
