---
sidebar_position: 7
---

# REST API

Every trait is callable via HTTP. The server runs on port 8090 by default.

For the full interactive reference with all endpoints, schemas, and examples, see the [API Reference (Redoc)](https://traits.build/docs/api).

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

## Page routes

Static HTML pages are served via keyed interface bindings:

| URL | Trait | Description |
|-----|-------|-------------|
| `/` | `www.traits.build` | Landing page |
| `/admin` | `www.admin` | Admin dashboard (Basic Auth) |
| `/docs` | `www.docs` | Documentation |
| `/docs/api` | `www.docs.api` | API documentation (Redoc) |

## OpenAPI spec

The OpenAPI 3.0 specification is generated dynamically by the `sys.openapi` trait. The [interactive API reference](https://traits.build/docs/api) renders this spec via Redoc and is the authoritative endpoint reference — it always reflects the currently deployed traits and their signatures.
