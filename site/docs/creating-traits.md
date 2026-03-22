---
sidebar_position: 9
---

# Creating Traits

Add new functionality to the kernel by creating traits. Each trait needs a TOML definition and a Rust source file.

## Step 1: Create the directory

```bash
mkdir -p traits/sys/my_trait
```

## Step 2: Write the definition

Create `traits/sys/my_trait/my_trait.trait.toml`:

```toml
[trait]
description = "Does something useful"
version = "v260322"
author = "system"
tags = ["sys", "utility"]

[signature]
params = [
  { name = "input", type = "string", description = "Input text", required = true },
]

[signature.returns]
type = "string"
description = "Processed output"

[implementation]
language = "rust"
source = "builtin"
entry = "my_trait"
```

## Step 3: Write the implementation

Create `traits/sys/my_trait/my_trait.rs`:

```rust
use serde_json::Value;

pub fn my_trait(args: &[Value]) -> Value {
    let input = args.first()
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Value::String(format!("Processed: {}", input))
}
```

## Step 4: Build and test

```bash
cargo build --release

# Test via CLI
./target/release/traits call sys.my_trait "hello"

# Test via REST
curl -X POST http://127.0.0.1:8090/traits/sys/my_trait \
  -d '{"args": ["hello"]}'
```

The build system automatically discovers and compiles the new trait.

## Step 5: Add tests (optional)

Create `traits/sys/my_trait/my_trait.features.json`:

```json
{
  "feature": "sys.my_trait processes input",
  "tests": [
    {
      "name": "basic processing",
      "input": ["hello"],
      "assertions": [
        { "type": "contains", "expected": "Processed: hello" }
      ]
    }
  ]
}
```

Run tests:

```bash
./target/release/traits test_runner 'sys.my_trait'
```

## Adding a web page trait

Web page traits return HTML:

```rust
use serde_json::Value;

pub fn my_page(_args: &[Value]) -> Value {
    Value::String(r#"<!DOCTYPE html>
<html>
<body><h1>My Page</h1></body>
</html>"#.to_string())
}
```

Set `provides = ["www/webpage"]` in the trait definition, then wire the route in `kernel.serve`:

```toml
# In traits/kernel/serve/serve.trait.toml
[requires]
"/my-page" = "www/webpage"

[bindings]
"/my-page" = "www.my_page"
```

## Background traits

For long-running traits (like servers), add `background = true`:

```toml
[trait]
background = true
```

And implement the `start()` function instead:

```rust
pub async fn start(args: &[crate::types::TraitValue])
    -> Result<crate::types::TraitValue, Box<dyn std::error::Error + Send + Sync>>
{
    // Long-running logic here
    Ok(crate::types::TraitValue::Map(Default::default()))
}
```

## Adding a CLI formatter

For custom CLI output, create a `*_cli.rs` companion:

```rust
// my_trait_cli.rs
pub fn format_cli(result: &serde_json::Value) -> String {
    // Custom formatting for terminal output
    format!("Result: {}", result)
}
```

The build system auto-discovers `_cli.rs` files alongside trait definitions.
