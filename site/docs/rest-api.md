---
sidebar_position: 7
---

# REST API

Every trait is callable via HTTP. The server runs on port 8090 by default.

## Calling traits

```
POST /traits/{namespace}/{name}
Content-Type: application/json

{"args": [...]}
```

### Positional arguments (array)

```bash
curl -X POST http://127.0.0.1:8090/traits/sys/checksum \
  -H 'Content-Type: application/json' \
  -d '{"args": ["hash", "hello"]}'
```

### Named arguments (object)

```bash
curl -X POST http://127.0.0.1:8090/traits/sys/checksum \
  -H 'Content-Type: application/json' \
  -d '{"args": {"action": "hash", "data": "hello"}}'
```

Named arguments are matched to parameter names defined in the trait's signature.

### Response format

```json
{
  "result": { "ok": true, "checksum": "2cf24dba..." },
  "error": null
}
```

On error:

```json
{
  "result": null,
  "error": "Expected 2 arguments, got 0"
}
```

## Streaming (SSE)

Append `?stream=1` to get Server-Sent Events:

```bash
curl http://127.0.0.1:8090/traits/some/trait?stream=1 \
  -X POST -d '{"args": []}'
```

Response is `text/event-stream` with `data: {...}\n\n` frames.

## Introspection endpoints

### GET /health

Server health with uptime and trait count:

```json
{
  "status": "healthy",
  "version": "v260322",
  "trait_count": 28,
  "namespace_count": 3,
  "uptime_human": "2h 15m 30s",
  "uptime_seconds": 8130
}
```

### GET /metrics

Prometheus-compatible text format:

```
# HELP traits_total Total number of registered traits
# TYPE traits_total gauge
traits_total 28
```

### GET /traits

Hierarchical tree of all traits:

```json
{
  "sys": {
    "checksum": { "path": "sys.checksum", "description": "...", ... },
    "list": { ... }
  },
  "kernel": { ... },
  "www": { ... }
}
```

### GET /traits/\{path\}

Detailed info for a specific trait:

```json
{
  "path": "sys.checksum",
  "description": "Compute deterministic SHA-256 checksums",
  "version": "v260322",
  "signature": {
    "params": [
      { "name": "action", "type": "String", "description": "...", "optional": false },
      { "name": "data", "type": "Any", "description": "...", "optional": true }
    ],
    "returns": "Any"
  }
}
```

## Page routes

Static HTML pages are served via keyed interface bindings:

| URL | Trait | Description |
|-----|-------|-------------|
| `/` | `www.traits.build` | Landing page |
| `/admin` | `www.admin` | Admin dashboard (Basic Auth) |
| `/docs/api` | `www.docs.api` | API documentation (Redoc) |

## Interactive API docs

Visit [/docs/api](https://polygrait-api.fly.dev/docs/api) for the full interactive Redoc documentation with all endpoints, schemas, and examples.

The OpenAPI 3.0 specification is generated dynamically by the `sys.openapi` trait.
