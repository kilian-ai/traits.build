# serve

## Purpose

HTTP server kernel trait providing the REST API for the traits platform. Uses `background = true` for generic async dispatch. The `serve()` sync stub returns an error; `start(args)` is the async entry point called by the generic background dispatch mechanism. Supports interface-driven page routing via keyed `[requires]`/`[bindings]` sections — URL paths are mapped to page traits through the interface resolution system.

## Exports

* `serve(args)` — sync trait stub (returns error)
* `start(args)` — async entry point for background dispatch (extracts port, starts server)
* `start_server(config, port)` — async function that starts the HTTP server

---

## serve

### Purpose

Trait dispatch stub. Returns an error because the HTTP server requires the async runtime.

### Inputs

* `_args`: ignored

### Outputs

* JSON: `{"error": "kernel.serve is a background trait — dispatched via start()"}`

### Side Effects

none

---

## start (pub async)

### Purpose

Async entry point for background dispatch. Called by the generic background trait mechanism in the dispatcher.

### Inputs

* `args`: slice of TraitValue — first element optionally an Int used as port

### Outputs

* `Result<TraitValue, Box<dyn Error>>` — returns `{"ok": true}` map on success

### State

reads:
* `globals::CONFIG` (cloned)

writes:
* none (delegates to start_server)

### Side Effects

* Starts the HTTP server (blocks until shutdown)

### Dependencies

* `crate::globals::CONFIG`
* `start_server()`

### Flow

1. Extract port from first arg (if Int), otherwise fall back to `config.traits.port`
2. Clone config from global
3. Call `start_server(config, port)`
4. Return `{"ok": true}` map

### Edge Cases

* No CONFIG global set: returns error "No config available"
* No port arg and no config port: uses config default

---

## AppState (private struct)

### Purpose

Shared application state for actix-web handlers.

### Fields

* `dispatcher`: Dispatcher — the trait dispatcher instance

---

## CallQuery (private struct)

### Purpose

Query parameters for trait call endpoint.

### Fields

* `stream`: Option<String> — "1" or "true" to enable SSE streaming

---

## call_trait (private async)

### Purpose

Handle `POST /traits/{path...}` — call a trait with JSON body.

### Inputs

* `state`: shared AppState
* `path`: URL path (slashes converted to dots)
* `body`: CallRequest JSON (args as array or named object)
* `query`: CallQuery (optional stream flag)

### Outputs

* HttpResponse with CallResponse JSON

### State

reads: state.dispatcher, registry (for kwarg resolution)
writes: none

### Side Effects

* Dispatches trait call which may have arbitrary side effects

### Flow

1. Convert URL path slashes to dots
2. Convert args: if Array, map to TraitValues; if Object (kwargs), resolve by param names from registry (supporting underscore/hyphen variants); otherwise empty
3. Build CallConfig from interface_overrides and trait_overrides
4. If `?stream=1`, delegate to `call_trait_sse`
5. Call dispatcher.call()
6. Map result/error to appropriate HTTP status:
   - Ok → 200 with result
   - NotFound → 404
   - ArgCount → 400
   - TypeMismatch → 400
   - Timeout → 504
   - Other error → 500

### Edge Cases

* Named args with hyphens: "telegram-token" resolves to param "telegram_token"
* Object args for unknown trait: returns empty arg list
* Streaming mode: delegates to SSE handler

---

## call_trait_sse (private async)

### Purpose

Handle streaming trait calls via Server-Sent Events.

### Inputs

* `state`: shared AppState
* `trait_path`: resolved trait path
* `args`: Vec<TraitValue>
* `config`: CallConfig

### Outputs

* HttpResponse with SSE stream or error

### Flow

1. Create mpsc channel (capacity 64)
2. Call dispatcher.call_stream() with sender
3. On success: wrap receiver as SSE stream (`data: {json}\n\n` format)
4. Set headers: text/event-stream, no-cache, keep-alive, no buffering
5. On error: return appropriate HTTP error

---

## health_check (private async)

### Purpose

Handle `GET /health` — return server health status.

### Outputs

* JSON: `{"status": "healthy", "version": "<cargo_pkg_version>"}`

---

## metrics (private async)

### Purpose

Handle `GET /metrics` — return Prometheus-format metrics.

### Flow

1. Call `sys.registry` with "count" action
2. Format as Prometheus gauge text
3. On error: return error text

---

## list_traits (private async)

### Purpose

Handle `GET /traits` — return trait registry tree.

### Flow

1. Call `sys.registry` with "tree" action
2. Return JSON result

---

## get_trait_info (private async)

### Purpose

Handle `GET /traits/{path}` — return info for a specific trait.

### Flow

1. Convert path slashes to dots
2. If empty path, delegate to list_traits
3. Call `sys.info` with the trait path
4. If result contains "error" key, return 404
5. Otherwise return 200 with trait info

---

## serve_page (private async)

### Purpose

Dynamic page handler that resolves keyed interface bindings from kernel.serve's `[requires]`/`[bindings]`. Each key is a URL path (e.g. "/", "/admin"), resolved to a page trait via the dispatcher's interface resolution system.

### Inputs

* `state`: shared AppState
* `req`: HttpRequest — the incoming request (URL path extracted)

### Outputs

* HttpResponse — HTML from the resolved page trait, or 404/500

### State

reads:
* `state.dispatcher` (resolve_keyed, call)

writes:
* none

### Side Effects

* Calls the resolved page trait which may have side effects

### Dependencies

* `Dispatcher::resolve_keyed(url_path, caller_path)`
* `Dispatcher::call()`

### Flow

1. Extract URL path from request
2. Call `dispatcher.resolve_keyed(url_path, "kernel.serve")` to find the page trait
3. If no trait bound for this path, return 404 HTML: "No page trait bound for this path."
4. Call the resolved page trait with empty args
5. If result is String: return as text/html
6. If result is other type: JSON-stringify and return as text/html
7. On error: return 500 with error message

### Edge Cases

* URL path not in [requires]/[bindings]: returns 404
* Page trait returns non-string (e.g. Map): falls through to JSON representation
* Page trait execution error: returns 500 with plain text error

### Example

`GET /` → resolve_keyed("/", "kernel.serve") → "www.traits.build" → call → HTML response

---

## start_server

### Purpose

Create the dispatcher from pre-initialized globals and start the HTTP server with keyed page route resolution.

### Inputs

* `config`: Config — server configuration
* `port`: u16 — TCP port to bind

### Outputs

* `Result<(), Box<dyn std::error::Error>>`

### State

reads:
* `globals::REGISTRY` (cloned)
* `config.traits.timeout`

writes:
* none (globals already initialized by caller)

### Side Effects

* Resolves and logs all keyed page routes from kernel.serve's [requires]/[bindings]
* Binds TCP port
* Runs HTTP server until shutdown

### Dependencies

* `crate::globals::REGISTRY`
* `Dispatcher::new(registry, timeout)`
* `Dispatcher::resolve_all_keyed("kernel.serve")`
* `actix_web::HttpServer`
* `actix_cors::Cors`

### Flow

1. Clone registry from `globals::REGISTRY` (panics if not initialized)
2. Create `Dispatcher::new(registry, config.traits.timeout)`
3. Call `dispatcher.resolve_all_keyed("kernel.serve")` to get all keyed page routes
4. Log each page route: "Page route '/' → www.traits.build"
5. Wrap dispatcher in web::Data for shared state
6. Log startup: port and number of page routes
7. Configure actix-web App:
   - CORS: allow any origin, method, header (max-age 3600)
   - Routes: /health (GET), /metrics (GET), /traits (GET), /traits/ (GET), /traits/{path} (POST for call, GET for info)
   - Default service: `serve_page` handler (catches all non-API routes for page resolution)
8. Set workers to 2
9. Bind to `0.0.0.0:{port}`
10. Run server

### Edge Cases

* REGISTRY global not set: panics with "Registry must be initialized before starting server"
* Port already in use: returns bind error
* No keyed routes defined: server starts with 0 page routes, all non-API paths return 404

---

## Internal Structure

`serve()` is a sync stub — `start()` is the async entry point called by the generic background dispatch mechanism — `start_server()` does the actual server setup. The server uses actix-web with CORS enabled. API routes (`/health`, `/metrics`, `/traits/...`) map to dedicated handler functions. All other routes fall through to `serve_page` via `default_service`, which resolves URL paths to page traits using the dispatcher's keyed interface resolution. The resolution chain: kernel.serve's `[bindings]` → `[requires]` → interface auto-discover.

## Notes

* CORS allows any origin — intended for development and local GUI access
* SSE streaming uses tokio mpsc channel with capacity 64
* Named args resolve underscore/hyphen variants for flexibility
* Workers fixed at 2 for reduced resource usage
* Page routing uses `resolve_keyed(url_path, "kernel.serve")` — URL path keys in `[requires]` map to interfaces, `[bindings]` provide concrete implementations
* Non-API paths with no matching keyed binding return 404 HTML
* `start_server` expects globals to be pre-initialized (REGISTRY, CONFIG) — it does not bootstrap
