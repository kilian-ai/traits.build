---
sidebar_position: 10
---

# Deployment

traits.build is deployed to [Fly.io](https://fly.io) as a single Docker image.

## Live instance

- **URL:** [https://traits.build](https://traits.build)
- **API docs:** [https://traits.build/docs/api](https://traits.build/docs/api)
- **Health:** [https://traits.build/health](https://traits.build/health)
- **Region:** `iad` (Ashburn, Virginia)
- **Resources:** shared CPU, 1 vCPU, 512 MB RAM

## Dockerfile

The multi-stage Dockerfile builds a minimal image:

```dockerfile
# Stage 1: Build
FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# Stage 2: Runtime
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/traits /usr/local/bin/traits
COPY --from=builder /app/traits /app/traits
COPY --from=builder /app/traits.toml /app/traits.toml
WORKDIR /app
CMD ["traits"]
```

No Node.js, Python, or other runtimes — just the Rust binary, trait definitions, and TLS certs.

## Fly.io configuration

```toml
# fly.toml
app = "your-fly-app"

[build]

[env]
TRAITS_PORT = "8090"

[http_service]
internal_port = 8090
force_https = true
auto_stop_machines = "stop"
auto_start_machines = true
min_machines_running = 0

[[http_service.checks]]
interval = "30s"
timeout = "5s"
grace_period = "10s"
method = "GET"
path = "/health"
```

## Deploy commands

```bash
# Build for amd64 (Fly runs Linux/amd64)
docker buildx build --platform linux/amd64 \
  -t registry.fly.io/your-fly-app:latest .

# Deploy
fly deploy --now --local-only \
  --image registry.fly.io/your-fly-app:latest
```

## Admin dashboard

The `/admin` endpoint provides deploy, scale, and destroy controls protected by HTTP Basic Auth:

```bash
# Set the admin password
fly secrets set ADMIN_PASSWORD="your-password"
```

Access at `https://traits.build/admin` with username `admin`.

## Auto-scaling

Fly.io auto-scales between 0 and 2 machines:
- **0 machines** when idle (no traffic)
- **Auto-starts** on first request (~2 sec cold start)
- **Scales to 2** under load

## Relay pairing code persistence

When a helper is started with `RELAY_URL=https://relay.traits.build traits serve`, the relay pairing code is now persisted and reused on reconnect when possible.

- Browser disconnect keeps the saved code locally so reconnect does not require retyping it.
- Helper reconnect attempts to reclaim the previous code via `sys.config` (`sys.serve.RELAY_CODE`).
- If the code is still available, users can reconnect with the same pairing code after helper restarts.

## One-shot helper script behavior

`curl -fsSL https://traits.build/local/helper.sh | bash` now:

- defaults to `RELAY_URL=https://relay.traits.build` for any `serve` invocation when `RELAY_URL` is unset,
- auto-upgrades legacy `RELAY_URL=https://traits-build.fly.dev` to `https://relay.traits.build`,
- auto-reexecs `serve` from a downloaded helper file when started via `curl ... | bash` to recover normal interactive terminal behavior,
- reattaches stdin from `/dev/tty` for `serve` when launched via a pipe so the REPL remains interactive,
- `sys.serve` now also attempts a server-side `/dev/tty` reattach before disabling REPL if stdin is not a TTY,
- and if raw key-event mode cannot be initialized, the CLI automatically falls back to a line-mode REPL on `/dev/tty`.

Line-mode input normalizes Enter to the same carriage-return event used by raw mode, preventing command concatenation (e.g. `helpps`).

Troubleshooting override:

- Set `TRAITS_REPL_LINE_MODE=1` before `traits serve` to force line-mode REPL (no raw key handling).
