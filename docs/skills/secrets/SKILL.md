---
name: secrets
description: |
  How to use the sys.secrets trait for managing secrets in traits.build.
  Covers storing, retrieving, and using secrets safely in custom traits.
---

# Secrets Management — traits.build

> Store and use secrets (API keys, tokens, passwords) safely in traits.
> Secrets are encrypted at rest, zeroized on drop, and never exposed in logs or API responses.

## Security Model

```
User / MCP Input   →   Safe zone (no secrets in data flow)
sys.secrets store  →   Encrypted at rest (AES-256 + per-secret encryption)
SecretContext      →   Scoped access (only declared secrets available)
Tool Execution     →   Trusted zone (secret values accessible)
After Execution    →   Secrets zeroized from memory
```

**Six layers of protection:**

1. **At rest** — AES-256 encryption, key from env var or auto-generated key file
2. **In memory** — `Secret` type auto-zeroizes on drop
3. **Data flow** — Secrets never enter trait args or return values
4. **Access control** — Per-trait `allowed_secrets` declaration
5. **Execution boundary** — Secrets created just-in-time, dropped immediately after
6. **Developer safety** — `Debug` is masked (`***`), `get` only confirms existence

## Quick Reference

### Store a secret (CLI)

```bash
traits call sys.secrets set stripe_key sk_live_abc123
traits call sys.secrets set db_password postgres://user:pass@host/db
```

### Store a secret (MCP tool)

```json
{
  "name": "mcp_traits-build_sys_secrets",
  "arguments": { "action": "set", "id": "stripe_key", "value": "sk_live_abc123" }
}
```

### Store a secret (REST)

```bash
curl -X POST https://traits.build/traits/sys/secrets \
  -H 'Content-Type: application/json' \
  -d '{"args": ["set", "stripe_key", "sk_live_abc123"]}'
```

### List secrets (CLI)

```bash
traits call sys.secrets list
```

Returns IDs only — values are never exposed:

```json
{
  "ok": true,
  "action": "list",
  "secrets": ["db_password", "stripe_key"],
  "count": 2
}
```

### Check if a secret exists

```bash
traits call sys.secrets get stripe_key
```

Returns existence only — never the value:

```json
{
  "ok": true,
  "action": "get",
  "id": "stripe_key",
  "exists": true
}
```

### Delete a secret

```bash
traits call sys.secrets delete stripe_key
```

### Resolve a secret context

```bash
traits call sys.secrets resolve stripe_key,db_password
```

Returns which of the requested secrets are available (not their values):

```json
{
  "ok": true,
  "action": "resolve",
  "requested": ["stripe_key", "db_password"],
  "available": ["stripe_key"],
  "count": 1
}
```

---

## Using Secrets in Custom Traits (Rust)

When building a trait that needs secrets, use `SecretContext` for scoped, safe access.

### 1. Import the secrets module

Since `sys.secrets` is a builtin trait, its Rust code is compiled into the binary.
Access the `SecretContext` type from the compiled module:

```rust
// In your trait's .rs file
use crate::secrets::SecretContext;
```

Or if accessing from a compiled trait path:

```rust
// The secrets module is auto-registered at crate level
// Access SecretContext directly
```

### 2. Declare allowed secrets

In your trait implementation, explicitly declare which secrets it needs:

```rust
pub fn my_trait_dispatch(args: &[Value]) -> Value {
    // Declare which secrets this trait is allowed to access
    let allowed_secrets = &["stripe_key"];

    // Resolve only the allowed secrets — creates a scoped context
    let ctx = crate::secrets::SecretContext::resolve(allowed_secrets);

    // Use the secret
    match ctx.get("stripe_key") {
        Some(api_key) => {
            // api_key is &str — use it directly
            // It will be zeroized when ctx is dropped
            serde_json::json!({ "ok": true, "message": "API call succeeded" })
        }
        None => {
            serde_json::json!({
                "error": "stripe_key not configured",
                "hint": "Run: traits call sys.secrets set stripe_key <your-key>"
            })
        }
    }
    // ← ctx dropped here, all secrets zeroized from memory
}
```

### 3. Reference secrets in JSON input (for MCP/REST callers)

When users reference secrets in tool input, use the `$secret` convention:

```json
{
  "apiKey": { "$secret": "stripe_key" }
}
```

Your trait code resolves these references:

```rust
fn resolve_secret_refs(input: &Value, ctx: &SecretContext) -> Value {
    match input {
        Value::Object(map) => {
            if let Some(Value::String(id)) = map.get("$secret") {
                // Replace $secret reference with actual value
                match ctx.get(id) {
                    Some(val) => Value::String(val.to_string()),
                    None => Value::Null,
                }
            } else {
                let mut out = serde_json::Map::new();
                for (k, v) in map {
                    out.insert(k.clone(), resolve_secret_refs(v, ctx));
                }
                Value::Object(out)
            }
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|v| resolve_secret_refs(v, ctx)).collect())
        }
        other => other.clone(),
    }
}
```

---

## Full Example: Trait with Secret Access

Here's a complete example of a trait that uses secrets:

### Trait definition (`my_api/my_api.trait.toml`)

```toml
[trait]
description = "Call an external API using a stored secret key"
version = "v260323"
author = "system"
tags = ["api", "example"]

[[signature.params]]
name = "action"
type = "string"
description = "API action to perform"

[signature.returns]
type = "object"
description = "API response"

[implementation]
language = "rust"
source = "builtin"
entry = "my_api"
```

### Implementation (`my_api/my_api.rs`)

```rust
use serde_json::Value;

/// Secrets this trait is allowed to access
const ALLOWED_SECRETS: &[&str] = &["my_api_key"];

pub fn my_api_dispatch(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("");
    my_api(action)
}

pub fn my_api(action: &str) -> Value {
    // 1. Resolve secrets — scoped to this trait
    let ctx = crate::secrets::SecretContext::resolve(ALLOWED_SECRETS);

    // 2. Check required secret is available
    let api_key = match ctx.get("my_api_key") {
        Some(key) => key.to_string(), // Clone the value before ctx is used further
        None => return serde_json::json!({
            "error": "my_api_key not configured",
            "hint": "Run: traits call sys.secrets set my_api_key <your-key>"
        }),
    };

    // 3. Use the secret
    match action {
        "status" => {
            // Use api_key for your API call here
            let _ = &api_key; // placeholder
            serde_json::json!({ "ok": true, "status": "connected" })
        }
        _ => serde_json::json!({ "error": format!("Unknown action: {}", action) }),
    }
    // 4. ctx dropped here → secrets zeroized from memory
}
```

---

## Secret ID Naming Convention

Use descriptive, namespaced IDs:

| Pattern | Example | Use Case |
|---------|---------|----------|
| `service_key` | `stripe_key` | API keys |
| `service_token` | `github_token` | OAuth/bearer tokens |
| `service_password` | `db_password` | Passwords |
| `service_secret` | `webhook_secret` | Signing secrets |
| `env.VAR_NAME` | `env.DATABASE_URL` | Environment-style secrets |

Rules: alphanumeric characters, underscores (`_`), and dots (`.`) only.

---

## Key Storage

Secrets are stored encrypted on disk:

| Environment | Store Location | Key Source |
|-------------|---------------|------------|
| Production (Fly.io) | `/data/secrets.enc` | `TRAITS_SECRET_KEY` env var (Fly secret) |
| Development | `~/.traits/secrets.enc` | Auto-generated `~/.traits/.secret_key` |

### Set up production key

```bash
# On Fly.io: set a persistent encryption key
fly secrets set TRAITS_SECRET_KEY="your-32-char-random-string-here"
```

### Key hierarchy

```
TRAITS_SECRET_KEY (env var, highest priority)
    └── derives 32-byte AES key via SHA-256
        └── encrypts secrets.enc (the store file)
            └── each secret value is encrypted individually inside the store
```

---

## What This Does NOT Solve

Be explicit about limitations:

- **Malicious traits** can still exfiltrate secrets they're allowed to access
- **Prompt injection** could trigger secret usage through trait dispatch
- **Memory scraping** is possible for advanced attackers (zeroize reduces window)
- **No audit logging** yet — consider adding a `sys.secrets.audit` trait
- **No secret rotation** — delete + re-set is the current workflow

---

## Actions Reference

| Action | Args | Description |
|--------|------|-------------|
| `set` | `id`, `value` | Store an encrypted secret |
| `get` | `id` | Check if a secret exists (never returns value) |
| `delete` | `id` | Remove a secret from the store |
| `list` | — | List all secret IDs |
| `resolve` | `id1,id2,...` | Check which secrets are available |
