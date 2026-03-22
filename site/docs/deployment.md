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
app = "polygrait-api"

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
  -t registry.fly.io/polygrait-api:latest .

# Deploy
fly deploy --now --local-only \
  --image registry.fly.io/polygrait-api:latest
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
