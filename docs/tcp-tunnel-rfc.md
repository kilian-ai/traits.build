# TCP Port Tunnel — RFC

Expose any TCP port (sshd, ftpd, syncthing, etc.) from an Alpine Linux guest (v86 or www.linux WASM) to external clients through the relay infrastructure.

## What we have today (no changes needed)

| Component | Location | Relevant capability |
|---|---|---|
| `cloudflare:sockets connect()` | `relay/src/index.js:1` | Real outbound TCP from CF Worker edge |
| `FsBridge` Durable Object | `relay/src/index.js:1528` | **Exact WS↔WS byte-splice primitive needed** |
| `/fs9p/host|client` route pattern | `relay/src/index.js:1735` | WS-pair bridge (host connects, client connects, bytes flow) |
| WISP v1 server (`/wisp`) | `relay/src/index.js:882` | Full TCP mux already working for v86 |
| HMAC signed tokens | `relay/src/index.js:36` | Auth without re-entering codes |
| `traits-build` Fly.io app | `fly.toml` | Rust binary, persistent volume, can add TCP services |

The `FsBridge` DO is the exact pattern we need:
- Guest connects WebSocket as "host" → `hostWs`
- External client connects WebSocket as "client" → `clientWs`
- Every `message` event is forwarded to the other side (binary transparent)
- Close on either side propagates to the other

---

## Architecture

```
[External TCP client]
        |  raw TCP (port 22 / 21 / 22000 / etc.)
        v
[Fly.io TCP gateway: traits-build-ports]   ← NEW Fly app (does not touch running traits-build)
        |  WebSocket to wss://tunnel.traits.build/port/client?code=XXXX&port=22
        v
[Cloudflare Worker: traits-build-tunnel]   ← NEW CF Worker (fork of relay, not modifying it)
 PortSession Durable Object (per code)
        ^  WebSocket from wss://tunnel.traits.build/port/guest?code=XXXX&port=22
        |
[Alpine Linux guest — websocat bridge]
        |  tcp:localhost:22
        v
[sshd / ftpd / syncthing / any TCP service]
```

**For WebSocket-native clients** (no raw TCP needed):
```
ssh -o ProxyCommand="websocat wss://tunnel.traits.build/port/client?code=XXXX&port=22 -" user@dummy
```
No Fly TCP gateway required. Pure CF Worker.

---

## Implementation Phases

### Phase 1 — Fork relay → New CF Worker `traits-build-tunnel`

**Directory:** `relay-tunnel/` (sibling to `relay/`, isolated from running system)

**`relay-tunnel/wrangler.toml`:**
```toml
name = "traits-build-tunnel"
account_id = "266790f1b479dc950d5833befe740e29"
main = "src/index.js"
compatibility_date = "2024-09-23"
workers_dev = true

routes = [
  { pattern = "tunnel.traits.build", custom_domain = true }
]

[durable_objects]
bindings = [
  { name = "PORT_SESSION", class_name = "PortSession" }
]

[[migrations]]
tag = "v1"
new_classes = ["PortSession"]
```

**`relay-tunnel/src/index.js` — minimal new endpoints:**

```
GET  /health
POST /port/register   { code?, ports: [22, 21, 22000, 8384] }
                      → { code, token }   (HMAC-signed 30-day token)
WS   /port/guest?code=XXXX&port=22        ← guest connects here
WS   /port/client?code=XXXX&port=22       ← external client connects here
GET  /port/status?code=XXXX               → { registered_ports, guest_connected, client_connected, ... }
POST /port/unregister { code }
GET  /port/debug?code=XXXX
```

**`PortSession` DO** — thin wrapper over `FsBridge` pattern, extended with:
- `registeredPorts: Set<number>` — ports the guest has declared
- `guestWs: Map<port, WebSocket>` — one WS per port per session
- `clientWs: Map<port, WebSocket>` — one WS per port per connected client
- Concurrent client handling: if a new client connects and one is active, queue (or reject with 503)
- Eviction TTL: 5-minute idle drops the code

**Guest-side usage (Phase 1, 1:1 sessions):**
```sh
# Install websocat (Alpine apk)
apk add --no-cache websocat

# Start sshd
/usr/sbin/sshd

# Register ports
curl -sX POST https://tunnel.traits.build/port/register \
  -H 'Content-Type: application/json' \
  -d '{"ports":[22]}' | tee /tmp/tunnel.json

CODE=$(cat /tmp/tunnel.json | python3 -c "import json,sys; print(json.load(sys.stdin)['code'])")

# Bridge sshd to relay
websocat --binary wss://tunnel.traits.build/port/guest?code=$CODE\&port=22 tcp:localhost:22 &
echo "SSH tunnel: ssh -o ProxyCommand='websocat wss://tunnel.traits.build/port/client?code=$CODE&port=22 -' user@dummy"
```

---

### FTP note (implemented helper behavior)

`local/tunnel-up.sh` now treats FTP as a multi-port service:

- If port `21` is requested, it auto-adds passive ports `FTP_PASV_MIN..FTP_PASV_MAX` (defaults `30000-30010`) to the registration set.
- It auto-starts `vsftpd` on `127.0.0.1:21` with that fixed passive range, so control + data channels can both traverse relay.

Use environment overrides for custom ranges:

```sh
FTP_PASV_MIN=31000 FTP_PASV_MAX=31020 \
  sh <(curl -sS https://www.traits.build/local/tunnel-up.sh) 21
```

On macOS/Linux host, use the companion listener helper to map control + passive
ports locally in one command:

```sh
sh <(curl -sS https://www.traits.build/local/tunnel-listen-ftp.sh) CODE 2121 21 30000 30010
```

Then point your FTP GUI/client to `127.0.0.1:2121` in passive mode.

### Phase 2 — Multi-port & concurrent sessions

For syncthing (needs continuous connection + multi-stream) and FTP (control+data channels), Phase 1's 1:1 model is limiting. Phase 2 upgrades `PortSession` to use a connection-ID model:

```
POST /port/accept?code=XXXX&port=22   → { conn_id }   (called by Fly gateway on new TCP accept)
WS   /port/guest/stream?code=XXXX&conn_id=abc123
WS   /port/client/stream?code=XXXX&conn_id=abc123
```

**Guest-side with `websocat --exec`:**
```sh
# Listen for new connection slots, spawn a websocat for each
while true; do
  CONN=$(curl -sX POST https://tunnel.traits.build/port/wait?code=$CODE\&port=22)
  CONN_ID=$(echo $CONN | python3 -c "import json,sys; print(json.load(sys.stdin)['conn_id'])")
  websocat --binary \
    wss://tunnel.traits.build/port/guest/stream?code=$CODE\&conn_id=$CONN_ID \
    tcp:localhost:22 &
done
```

Or with a small Go binary `tunnel-agent` that handles this loop cleanly and ships with the initramfs.

---

### Phase 3 — Fly.io TCP gateway

Required to expose **raw ports** where the client cannot use `websocat`/ProxyCommand (e.g., native SSH clients, FTP GUIs, Syncthing without config changes).

**New Fly app: `traits-build-ports`** (`fly-tcp-gw/fly.toml`):
```toml
app = "traits-build-ports"
primary_region = "sjc"

[build]
  dockerfile = "Dockerfile"

[[services]]
  protocol = "tcp"
  internal_port = 2222
  [[services.ports]]
    port = 22              # external port 22 → internal 2222

[[services]]
  protocol = "tcp"
  internal_port = 2121
  [[services.ports]]
    port = 21              # ftpd

[[services]]
  protocol = "tcp"
  internal_port = 22000
  [[services.ports]]
    port = 22000           # syncthing sync protocol

[[services]]
  protocol = "tcp"
  internal_port = 8384
  [[services.ports]]
    port = 8384            # syncthing web GUI
```

**Gateway binary** (`fly-tcp-gw/src/main.rs`):
- Reads `TUNNEL_RELAY_URL` env (default `wss://tunnel.traits.build`)
- Reads `TUNNEL_CODE` env (the pairing code to forward to)
- For each configured port/listener:
  - `tokio::net::TcpListener::bind("0.0.0.0:2222")`
  - On accept: opens a WebSocket to `{relay}/port/client?code={code}&port={port}`
  - `tokio::io::copy_bidirectional` between TCP stream and WebSocket framing
- Uses `tokio-tungstenite` for WebSocket and standard `tokio` TCP
- ~150 lines of Rust

**DNS:**
```
tunnel.traits.build   → CF Worker (CNAME to traits-build-tunnel.workers.dev)
ports.traits.build    → Fly.io traits-build-ports app (A record)
```

External clients connect to `ports.traits.build:22` for SSH.

---

### Phase 4 — Agent integration

Add tunnel setup to the wanix-agent script and the browser-side JS agent:

**`wanix-agent` additions** (shell script in guest):
```sh
# After GOAL is parsed, check for tunnel-related goals:
#   "open ssh tunnel", "expose port", "start syncthing", etc.
# Auto-calls: tunnel_up <port> to establish relay connection
```

**New agent tool: `tunnel`** (alongside `shell`):
```json
{
  "type": "function",
  "function": {
    "name": "tunnel",
    "description": "Expose a local TCP port through relay tunnel. Returns public access URL.",
    "parameters": {
      "type": "object",
      "properties": {
        "port": { "type": "integer", "description": "Local port to expose (22=ssh, 21=ftp, 8384=syncthing)" }
      },
      "required": ["port"]
    }
  }
}
```

**Browser JS agent** (`runJSAgent` in `traits/www/linux/linux.rs`):
- Intercepts `tunnel <port>` commands
- Calls `wss://tunnel.traits.build/port/register` → gets code
- Sends `websocat` setup command to guest shell
- Displays connection info in terminal UI

---

## Cloudflare Worker Constraints

| Constraint | Impact | Mitigation |
|---|---|---|
| `cloudflare:sockets` can't reach CF-hosted IPs | Outbound TCP to CF-fronted APIs fails | Already documented in WISP code; not relevant for reverse tunnel use case |
| Workers can't listen on arbitrary TCP ports | Can't expose `:22` directly from CF | Phase 3 Fly.io gateway handles this |
| Durable Object WS connections close after ~10 minutes idle | Long-lived SSH sessions may drop | `websocat --ping-interval 30` keeps alive; DO session monitors pings |
| CF Worker 128MB memory limit | Limits concurrent tunnel WS pairs per isolate | One PortSession DO per code; memory is per-DO, each DO is small |

---

## Service Naming

| Service | Location | Domain |
|---|---|---|
| Traits relay (existing, unchanged) | `relay/` → CF Worker | `relay.traits.build` |
| TCP tunnel relay (new fork) | `relay-tunnel/` → CF Worker | `tunnel.traits.build` |
| TCP port gateway (new Fly app) | `fly-tcp-gw/` → Fly.io | `ports.traits.build` |

---

## Guest Quick-Start (Phase 1)

```sh
#!/bin/sh
# tunnel-up.sh — expose a local port through relay-tunnel
# Usage: tunnel-up.sh 22    (exposes sshd)

PORT="${1:-22}"
apk add --no-cache websocat curl python3 2>/dev/null

RESP=$(curl -sX POST https://tunnel.traits.build/port/register \
  -H 'Content-Type: application/json' \
  -d "{\"ports\":[$PORT]}")
CODE=$(printf '%s' "$RESP" | python3 -c "import json,sys; print(json.load(sys.stdin)['code'])")

echo "Starting tunnel for port $PORT → code $CODE"
websocat --binary \
  "wss://tunnel.traits.build/port/guest?code=${CODE}&port=${PORT}" \
  "tcp:localhost:${PORT}" &

echo ""
echo "Connect via WebSocket proxy:"
echo "  ssh -o ProxyCommand=\"websocat wss://tunnel.traits.build/port/client?code=${CODE}&port=${PORT} -\" root@dummy"
echo ""
echo "Connect via raw TCP (after Phase 3):"
echo "  ssh root@ports.traits.build   (code=$CODE configured in gateway)"
```

---

## Implementation Order

1. `relay-tunnel/` — fork CF Worker with `PortSession` DO (reuse `FsBridge` template, add port routing)
2. Deploy to `tunnel.traits.build` via `wrangler deploy` — zero risk to existing relay
3. `local/tunnel-up.sh` — guest script shipped with Alpine initramfs or downloadable
4. Test Phase 1 end-to-end with `websocat` ProxyCommand SSH
5. `fly-tcp-gw/` — add Rust TCP gateway for raw port exposure
6. Add `tunnel` tool to wanix-agent and browser JS agent
