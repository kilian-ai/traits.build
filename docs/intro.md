---
slug: /
sidebar_position: 1
---

# traits.build

**traits.build** is a composable function kernel written in pure Rust. Every capability — from HTTP serving to SHA-256 hashing to the registry itself — is a **trait**: a typed, self-describing function defined in TOML and compiled into a single binary.

> **Terminology note:** A "trait" in this project means a registered, typed function — not a Rust language `trait`. The naming is intentional: like Rust traits, these are composable abstractions with defined contracts. Context always disambiguates.

## What makes it different

- **Traits all the way down.** The kernel itself is built from traits. `kernel.serve` is a trait. `kernel.registry` is a trait. There is no special framework code — just traits calling traits.
- **Single binary.** All 28 traits compile into one ~2 MB executable with zero runtime dependencies.
- **Interface system.** Traits declare dependencies via typed interfaces, wired at startup. Swap implementations without changing callers.
- **Dual access.** Every trait is callable via CLI (`traits checksum hash "hello"`) and REST API (`POST /traits/sys/checksum`).

## Quick example

```bash
# Call a trait via CLI
traits checksum hash "hello world"
# → 7509e5bda0c762d2bac7f90d758b5b2263fa01ccbc...

# Same trait via REST
curl -X POST http://127.0.0.1:8090/traits/sys/checksum \
  -H 'Content-Type: application/json' \
  -d '{"args": ["hash", "hello world"]}'
```

## Architecture at a glance

```
┌───────────────────────────────────────────┐
│              traits binary                 │
├─────────┬──────────┬──────────────────────┤
│ kernel  │   sys    │        www           │
│ .serve  │ .list    │ .traits.build        │
│ .config │ .checksum│ .admin               │
│ .registry│.test_   │ .docs.api            │
│ .dispatch│ runner  │ .admin.deploy        │
│ ...     │ ...      │ ...                  │
└─────────┴──────────┴──────────────────────┘
         ↕ interface bindings ↕
```

Ready to get started? Head to [Getting Started](#getting-started).
