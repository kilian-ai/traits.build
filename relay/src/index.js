/**
 * traits.build relay — Cloudflare Worker + Durable Objects
 *
 * One RelaySession DO per pairing code. The DO holds all in-flight state
 * in memory, so long-poll coordination is instant and zero-latency.
 *
 * Routes:
 *   GET  /health
 *   POST /relay/register      → { code }
 *   POST /relay/connect       { code } → { token, code }   (HMAC-signed token)
 *   GET  /relay/poll?code=    → {id, path, args} when a call arrives, 204 on timeout
 *   POST /relay/call          { code|token, path, args } → { result, error }
 *   POST /relay/respond       { code, id, result }
 *   GET  /relay/status?code=  → { active, age_seconds, code }
 *   GET  /relay/status?token= → same, validated from signed token
 *
 * Signed tokens (requires RELAY_SECRET worker secret):
 *   After a client enters the 4-char pairing code, call /relay/connect to get a
 *   HMAC-SHA256 signed token { code, relay, iat, exp }. The token is stateless —
 *   the relay verifies its signature without any persistent store. Clients save the
 *   token in localStorage and use it for all future status checks and calls without
 *   re-entering the pairing code.
 *
 *   Setup:  npx wrangler secret put RELAY_SECRET
 *           (generate with: openssl rand -base64 32)
 */

// ── HMAC-SHA256 token signing (Web Crypto) ────────────────────────────────────

async function _getHmacKey(secret) {
  return crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign', 'verify'],
  );
}

const TOKEN_TTL_SECS = 86400 * 30; // 30 days

async function signToken(code, relayOrigin, secret) {
  const payload = {
    code,
    relay: relayOrigin,
    iat: Math.floor(Date.now() / 1000),
    exp: Math.floor(Date.now() / 1000) + TOKEN_TTL_SECS,
  };
  const payloadBytes = new TextEncoder().encode(JSON.stringify(payload));
  const key = await _getHmacKey(secret);
  const sig = await crypto.subtle.sign('HMAC', key, payloadBytes);
  const payloadB64 = btoa(JSON.stringify(payload));
  const sigB64 = btoa(String.fromCharCode(...new Uint8Array(sig)));
  return `${payloadB64}.${sigB64}`;
}

async function verifyToken(token, secret) {
  try {
    const dot = token.lastIndexOf('.');
    if (dot === -1) return null;
    const payloadB64 = token.slice(0, dot);
    const sigB64    = token.slice(dot + 1);
    const payload   = JSON.parse(atob(payloadB64));
    // Check expiry client-side before hitting crypto
    if (!payload.exp || Date.now() / 1000 > payload.exp) return null;
    const key       = await _getHmacKey(secret);
    const sigBytes  = Uint8Array.from(atob(sigB64), c => c.charCodeAt(0));
    const dataBytes = new TextEncoder().encode(JSON.stringify(payload));
    const valid     = await crypto.subtle.verify('HMAC', key, sigBytes, dataBytes);
    return valid ? payload : null;
  } catch(_) { return null; }
}

// ── CORS ─────────────────────────────────────────────────────────────────────

function cors() {
  return {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET,POST,OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type,Authorization",
  };
}

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json", ...cors() },
  });
}

// ── Linux tunnel helpers (raw IPv4 packets over WebSocket) ──────────────────

function _checksum(bytes) {
  let sum = 0;
  for (let i = 0; i < bytes.length; i += 2) {
    sum += ((bytes[i] << 8) | (bytes[i + 1] || 0));
  }
  while (sum >> 16) sum = (sum & 0xffff) + (sum >> 16);
  return (~sum) & 0xffff;
}

function _parseIpPacket(buf) {
  if (!(buf instanceof Uint8Array) || buf.length < 20) return null;
  const version = (buf[0] >> 4) & 0xf;
  if (version !== 4) return null;
  const ihl = (buf[0] & 0xf) * 4;
  const totalLen = (buf[2] << 8) | buf[3];
  if (ihl < 20 || totalLen < ihl || totalLen > buf.length) return null;
  const protocol = buf[9];
  const srcIp = [buf[12], buf[13], buf[14], buf[15]];
  const dstIp = [buf[16], buf[17], buf[18], buf[19]];
  const id = (buf[4] << 8) | buf[5];
  const payload = buf.slice(ihl, totalLen);
  return { protocol, srcIp, dstIp, id, payload };
}

function _buildIpPacket(protocol, srcIp, dstIp, payload, id = 0) {
  const totalLen = 20 + payload.length;
  const pkt = new Uint8Array(totalLen);
  pkt[0] = 0x45;
  pkt[1] = 0;
  pkt[2] = (totalLen >> 8) & 0xff;
  pkt[3] = totalLen & 0xff;
  pkt[4] = (id >> 8) & 0xff;
  pkt[5] = id & 0xff;
  pkt[6] = 0x40;
  pkt[7] = 0;
  pkt[8] = 64;
  pkt[9] = protocol;
  pkt[12] = srcIp[0]; pkt[13] = srcIp[1]; pkt[14] = srcIp[2]; pkt[15] = srcIp[3];
  pkt[16] = dstIp[0]; pkt[17] = dstIp[1]; pkt[18] = dstIp[2]; pkt[19] = dstIp[3];
  const cksum = _checksum(pkt.slice(0, 20));
  pkt[10] = (cksum >> 8) & 0xff;
  pkt[11] = cksum & 0xff;
  pkt.set(payload, 20);
  return pkt;
}

function _parseUdp(payload) {
  if (payload.length < 8) return null;
  const srcPort = (payload[0] << 8) | payload[1];
  const dstPort = (payload[2] << 8) | payload[3];
  const length = (payload[4] << 8) | payload[5];
  if (length < 8 || length > payload.length) return null;
  const data = payload.slice(8, length);
  return { srcPort, dstPort, data };
}

function _buildUdp(srcPort, dstPort, data) {
  const len = 8 + data.length;
  const udp = new Uint8Array(len);
  udp[0] = (srcPort >> 8) & 0xff;
  udp[1] = srcPort & 0xff;
  udp[2] = (dstPort >> 8) & 0xff;
  udp[3] = dstPort & 0xff;
  udp[4] = (len >> 8) & 0xff;
  udp[5] = len & 0xff;
  udp[6] = 0;
  udp[7] = 0;
  udp.set(data, 8);
  return udp;
}

function _handleIcmpEcho(ipPkt) {
  const p = ipPkt.payload;
  if (p.length < 8 || p[0] !== 8) return null;
  const reply = new Uint8Array(p);
  reply[0] = 0;
  reply[2] = 0;
  reply[3] = 0;
  const cksum = _checksum(reply);
  reply[2] = (cksum >> 8) & 0xff;
  reply[3] = cksum & 0xff;
  return _buildIpPacket(1, ipPkt.dstIp, ipPkt.srcIp, reply, ipPkt.id);
}

async function _handleDnsUdp(ipPkt) {
  const udp = _parseUdp(ipPkt.payload);
  if (!udp || udp.dstPort !== 53 || udp.data.length === 0) return null;
  try {
    const resp = await fetch('https://cloudflare-dns.com/dns-query', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/dns-message',
        'Accept': 'application/dns-message',
      },
      body: udp.data,
    });
    if (!resp.ok) return null;
    const dns = new Uint8Array(await resp.arrayBuffer());
    const udpReply = _buildUdp(53, udp.srcPort, dns);
    return _buildIpPacket(17, ipPkt.dstIp, ipPkt.srcIp, udpReply, ipPkt.id);
  } catch (_) {
    return null;
  }
}

function _linuxTunnelWs(request) {
  const upgrade = request.headers.get('Upgrade');
  if (!upgrade || upgrade.toLowerCase() !== 'websocket') {
    return json({
      error: 'Upgrade required',
      hint: 'Use WebSocket at wss://relay.traits.build/linux/tunnel',
    }, 426);
  }

  const pair = new WebSocketPair();
  const client = pair[0];
  const server = pair[1];
  server.accept();

  server.addEventListener('message', (evt) => {
    const data = evt.data;
    // Optional text ping for diagnostics.
    if (typeof data === 'string') {
      if (data === 'ping') server.send('pong');
      return;
    }

    let bytes;
    if (data instanceof ArrayBuffer) bytes = new Uint8Array(data);
    else if (data && data.buffer instanceof ArrayBuffer) bytes = new Uint8Array(data.buffer, data.byteOffset || 0, data.byteLength || data.length || 0);
    else return;

    const ipPkt = _parseIpPacket(bytes);
    if (!ipPkt) return;

    if (ipPkt.protocol === 1) {
      const reply = _handleIcmpEcho(ipPkt);
      if (reply) server.send(reply);
      return;
    }

    if (ipPkt.protocol === 17) {
      _handleDnsUdp(ipPkt).then((reply) => {
        if (reply) server.send(reply);
      });
    }
  });

  return new Response(null, { status: 101, webSocket: client });
}

// ── Pairing code generation ───────────────────────────────────────────────────

const CODE_CHARS = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // unambiguous chars

function generateCode() {
  const buf = new Uint8Array(4);
  crypto.getRandomValues(buf);
  return Array.from(buf, (v) => CODE_CHARS[v % CODE_CHARS.length]).join("");
}

function normalizeCode(code) {
  if (!code) return null;
  const normalized = String(code).trim().toUpperCase();
  return /^[A-Z0-9]{4}$/.test(normalized) ? normalized : null;
}

// ── Durable Object: RelaySession ──────────────────────────────────────────────
//
// One instance per pairing code (created via idFromName(code)).
// All relay coordination happens in-memory — no KV writes needed.
//
// In-memory state:
//   pendingRequest  — a request the Mac hasn't picked up yet (phone arrived first)
//   pollResolve     — the Mac's waiting resolve() (poller arrived first)
//   resultResolvers — Map<id, resolve> for open phone /relay/call Promises

export class RelaySession {
  constructor(state, env) {
    this.created = Date.now();
    this.lastPollAt = null;     // timestamp of last /poll from Mac
    this.pendingRequest = null; // { id, path, args }
    this.pollResolve = null;    // fn(request) — Mac's waiting resolver
    this.resultResolvers = new Map(); // id → fn(result)
  }

  async fetch(request) {
    const url = new URL(request.url);

    switch (url.pathname) {
      case "/register": return this._register();
      case "/poll":    return this._poll();
      case "/call":    return this._call(request);
      case "/respond": return this._respond(request);
      case "/status":  return this._status();
      default:         return new Response("not found", { status: 404 });
    }
  }

  _register() {
    this.created = Date.now();
    this.lastPollAt = null;
    this.pendingRequest = null;
    this.pollResolve = null;
    this.resultResolvers.clear();
    return json({ ok: true });
  }

  // Mac long-polls here. Resolves immediately if a request is already waiting,
  // otherwise suspends for up to 29s then returns 204 (Mac should re-poll).
  _poll() {
    this.lastPollAt = Date.now(); // track liveness for _status()
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.pollResolve = null;
        resolve(new Response(null, { status: 204, headers: cors() }));
      }, 29_000);

      const deliver = (req) => {
        clearTimeout(timer);
        this.pollResolve = null;
        resolve(json(req));
      };

      if (this.pendingRequest) {
        // A call was already queued before Mac reconnected — deliver immediately.
        const req = this.pendingRequest;
        this.pendingRequest = null;
        deliver(req);
      } else {
        this.pollResolve = deliver;
      }
    });
  }

  // Phone calls a trait via relay. Suspends until Mac responds or 60s timeout.
  async _call(request) {
    const body = await request.json();
    const id = crypto.randomUUID();
    const req = { id, path: body.path, args: body.args ?? [] };

    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.resultResolvers.delete(id);
        resolve(json({ error: "Relay timeout (60s)", result: null }, 504));
      }, 60_000);

      this.resultResolvers.set(id, (result) => {
        clearTimeout(timer);
        resolve(json(result));
      });

      // Wake the Mac if it's polling, otherwise queue the request.
      if (this.pollResolve) {
        this.pollResolve(req);
      } else {
        this.pendingRequest = req;
      }
    });
  }

  // Mac sends back the result for a previous request.
  async _respond(request) {
    const body = await request.json();
    const resolve = this.resultResolvers.get(body.id);
    if (!resolve) {
      return json({ error: "No pending request with that id" }, 404);
    }
    this.resultResolvers.delete(body.id);
    resolve(body); // body contains { id, result, error? }
    return json({ ok: true });
  }

  _status() {
    // Mac is considered connected if it's currently in a poll OR polled within the
    // last 35s (29s poll timeout + 6s grace for reconnect).
    const macConnected =
      this.pollResolve !== null ||
      (this.lastPollAt !== null && Date.now() - this.lastPollAt < 35_000);
    return json({
      active: macConnected,
      age_seconds: Math.floor((Date.now() - this.created) / 1000),
    });
  }
}

// ── Main Worker ───────────────────────────────────────────────────────────────

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (url.pathname === '/linux/tunnel') {
      return _linuxTunnelWs(request);
    }

    // CORS preflight
    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: cors() });
    }

    if (url.pathname === "/health") {
      return new Response("ok", { headers: cors() });
    }

    // POST /relay/register
    if (url.pathname === "/relay/register" && request.method === "POST") {
      let preferred = null;
      try {
        const text = await request.text();
        if (text) {
          const body = JSON.parse(text);
          preferred = normalizeCode(body.code);
        }
      } catch (_) {
      }
      const code = preferred || generateCode();
      const stub = env.RELAY.get(env.RELAY.idFromName(code));
      await stub.fetch(new Request("http://do/register", { method: "POST" }));
      return json({ code });
    }

    // GET /relay/poll?code=XXXX
    if (url.pathname === "/relay/poll" && request.method === "GET") {
      const code = url.searchParams.get("code");
      if (!code) return json({ error: "missing code" }, 400);
      return env.RELAY.get(env.RELAY.idFromName(code)).fetch(
        new Request("http://do/poll")
      );
    }

    // POST /relay/connect  { code } → { token, code }  (issues signed token)
    if (url.pathname === "/relay/connect" && request.method === "POST") {
      if (!env.RELAY_SECRET) return json({ error: "Token signing not configured on relay" }, 503);
      const body = await request.json().catch(() => ({}));
      const code = normalizeCode(body.code);
      if (!code) return json({ error: "invalid code" }, 400);
      // Verify Mac is actually polling before issuing a token
      const stub = env.RELAY.get(env.RELAY.idFromName(code));
      const statusData = await stub.fetch(new Request("http://do/status")).then(r => r.json());
      if (!statusData.active) return json({ error: "No helper connected with this code" }, 404);
      const token = await signToken(code, new URL(request.url).origin, env.RELAY_SECRET);
      return json({ ok: true, token, code });
    }

    // POST /relay/call  { code|token, path, args }
    if (url.pathname === "/relay/call" && request.method === "POST") {
      const body = await request.json();
      let code = normalizeCode(body.code);
      // Accept signed token in place of code
      if (!code && body.token && env.RELAY_SECRET) {
        const payload = await verifyToken(body.token, env.RELAY_SECRET);
        if (!payload) return json({ error: "Invalid or expired relay token" }, 401);
        code = payload.code;
      }
      if (!code) return json({ error: "missing code or token" }, 400);
      return env.RELAY.get(env.RELAY.idFromName(code)).fetch(
        new Request("http://do/call", {
          method: "POST",
          body: JSON.stringify({ ...body, code }),
          headers: { "Content-Type": "application/json" },
        })
      );
    }

    // POST /relay/respond  { code, id, result }
    if (url.pathname === "/relay/respond" && request.method === "POST") {
      const body = await request.json();
      if (!body.code) return json({ error: "missing code" }, 400);
      return env.RELAY.get(env.RELAY.idFromName(body.code)).fetch(
        new Request("http://do/respond", {
          method: "POST",
          body: JSON.stringify(body),
          headers: { "Content-Type": "application/json" },
        })
      );
    }

    // GET /relay/status?code=XXXX  or  ?token=XXX
    if (url.pathname === "/relay/status" && request.method === "GET") {
      let code = url.searchParams.get("code");
      // Accept signed token in place of code
      const token = url.searchParams.get("token");
      if (!code && token && env.RELAY_SECRET) {
        const payload = await verifyToken(token, env.RELAY_SECRET);
        if (!payload) return json({ error: "Invalid or expired relay token" }, 401);
        code = payload.code;
      }
      if (!code) return json({ error: "missing code or token" }, 400);
      const stub = env.RELAY.get(env.RELAY.idFromName(code));
      const res  = await stub.fetch(new Request("http://do/status"));
      const data = await res.json();
      return json({ ...data, code }); // always include resolved code in response
    }

    return json({ error: "not found" }, 404);
  },
};
