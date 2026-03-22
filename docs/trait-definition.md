---
sidebar_position: 4
---

# Trait Definition

Every trait is defined by a `.trait.toml` file alongside its Rust source. The TOML declares the trait's identity, signature, and wiring.

## File structure

Each trait lives in its own directory under `traits/{namespace}/{name}/`:

```
traits/sys/checksum/
├── checksum.trait.toml     # Definition
├── checksum.rs             # Implementation
└── checksum.features.json  # Tests (optional)
```

## Full template

```toml
[trait]
description = "Short description of what this trait does"
version = "v260322"
author = "system"
tags = ["namespace", "category"]
provides = ["namespace/interface"]

[signature]
params = [
  { name = "param1", type = "string", description = "First parameter", required = true },
  { name = "param2", type = "int", description = "Optional param", optional = true },
]

[signature.returns]
type = "string"
description = "What the trait returns"

[implementation]
language = "rust"
source = "builtin"
entry = "function_name"

[requires]
dep = "namespace/interface"

[bindings]
dep = "namespace.concrete_trait"
```

## Sections

### `[trait]`

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Human-readable summary |
| `version` | string | `vYYMMDD` format, auto-updated by build |
| `author` | string | Creator identifier |
| `tags` | array | Categorization labels |
| `provides` | array | Interface paths this trait implements |
| `background` | bool | If `true`, uses async `start()` entry point |

### `[signature]`

#### `params`

Each parameter is an object with:

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Parameter name |
| `type` | string | [Type](#types) identifier |
| `description` | string | What this parameter does |
| `required` / `optional` | bool | Whether the param is required |

#### `returns`

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Return [type](#types) |
| `description` | string | What the return value represents |

### `[implementation]`

| Field | Value | Description |
|-------|-------|-------------|
| `language` | `"rust"` | Always Rust in this branch |
| `source` | `"builtin"` or `"kernel"` | `builtin` for sys/www, `kernel` for kernel modules |
| `entry` | string | Function name to call |

### `[requires]` and `[bindings]`

See [Interfaces](#interfaces) for details on the dependency system.

## Types

The type system maps to both Rust and JSON:

| Type | Rust | JSON | Example |
|------|------|------|---------|
| `string` | `String` | `"text"` | `"hello"` |
| `int` | `i64` | `42` | `42` |
| `float` | `f64` | `3.14` | `3.14` |
| `bool` | `bool` | `true` | `true` |
| `bytes` | `Vec<u8>` | `"hex..."` | `"deadbeef"` |
| `list<T>` | `Vec<T>` | `[...]` | `[1, 2, 3]` |
| `map<K,V>` | `HashMap` | `{...}` | `{"a": 1}` |
| `any` | dynamic | any | anything |
| `null` | `()` | `null` | `null` |

## Implementation pattern

```rust
use serde_json::Value;

pub fn my_trait(args: &[Value]) -> Value {
    let param1 = args.first()
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    serde_json::json!({
        "ok": true,
        "result": param1
    })
}
```

Key conventions:
- Function signature is always `fn(args: &[Value]) -> Value`
- Args are positional, matching the `params` order in TOML
- Return any valid `serde_json::Value`
- Access kernel globals via `crate::globals::REGISTRY.get()` etc.
