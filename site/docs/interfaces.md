---
sidebar_position: 5
---

# Interface System

Traits declare dependencies via typed interfaces. This makes all dependencies explicit, introspectable, and swappable without changing the caller's code.

## How it works

The interface system uses three `.trait.toml` fields:

1. **`provides`** — declares which interfaces a trait implements
2. **`[requires]`** — declares dependencies by interface path
3. **`[bindings]`** — wires concrete traits to required interfaces

## Example: kernel.serve

`kernel.serve` needs a web page trait for each URL path and a dispatcher:

```toml
[trait]
provides = ["kernel/serve"]

[requires]
"/" = "www/webpage"
"/admin" = "www/webpage"
"/docs/api" = "www/webpage"
dispatcher = "kernel/dispatcher"

[bindings]
"/" = "www.traits.build"
"/admin" = "www.admin"
"/docs/api" = "www.docs.api"
dispatcher = "kernel.dispatcher"
```

When the server receives a request for `/docs/api`, it:
1. Looks up the keyed binding: `"/docs/api"` → `"www.docs.api"`
2. Calls `www.docs.api` with no arguments
3. Returns the resulting HTML

## Resolution chain

When resolving a required interface, the system checks in order:

1. **Per-call overrides** — passed in the request body
2. **Global bindings** — from runtime configuration
3. **Caller bindings** — from the `[bindings]` section of the calling trait
4. **Auto-discover** — find any trait that `provides` the interface

## URL-keyed bindings

A unique feature is that `[requires]` keys can be URL paths. This is how page routes work:

```toml
# The key is a URL path, the value is an interface
[requires]
"/" = "www/webpage"
"/admin" = "www/webpage"

# The binding maps the same key to a concrete trait
[bindings]
"/" = "www.traits.build"
"/admin" = "www.admin"
```

This pattern lets you wire HTTP routes purely through trait configuration.

## Interface table

Current interfaces in use:

| Interface | Provider(s) | Used by |
|-----------|-------------|---------|
| `kernel/dispatcher` | `kernel.dispatcher` | `kernel.main`, `kernel.serve`, `sys.cli` |
| `kernel/registry` | `kernel.registry` | `kernel.main`, `kernel.dispatcher` |
| `kernel/config` | `kernel.config` | `kernel.main`, `sys.cli` |
| `kernel/globals` | `kernel.globals` | `kernel.main` |
| `kernel/serve` | `kernel.serve` | `sys.cli` |
| `kernel/types` | `kernel.types` | — |
| `kernel/plugin_api` | `kernel.plugin_api` | — |
| `www/webpage` | `www.traits.build`, `www.admin`, `www.docs.api` | `kernel.serve` |
| `sys/cli` | `sys.cli` | `kernel.main` |
| `sys/openapi` | `sys.openapi` | — |

## Adding a page route

To add a new page at `/my-page`:

1. Create a trait that returns HTML:
   ```rust
   pub fn my_page(_args: &[Value]) -> Value {
       Value::String("<h1>Hello</h1>".to_string())
   }
   ```

2. Set `provides = ["www/webpage"]` in its `.trait.toml`

3. Add the route to `kernel.serve`:
   ```toml
   [requires]
   "/my-page" = "www/webpage"

   [bindings]
   "/my-page" = "www.my_page"
   ```

4. Rebuild: `cargo build --release`
