/**
 * traits-build-tunnel — TCP port tunnel via WebSocket pairs
 *
 * Forks the FsBridge pattern from relay/src/index.js into a dedicated
 * worker so the running relay.traits.build is never touched.
 *
 * Routes:
 *   GET  /health
 *   POST /port/register   { code?, ports:[22,21,22000,8384] } → { code, token }
 *   WS   /port/guest?code=XXXX&port=22     ← guest (websocat → local port)
 *   WS   /port/client?code=XXXX&port=22    ← external client
 *   GET  /port/status?code=XXXX            → { registered_ports, guest_ports, client_ports, age_s }
 *   POST /port/unregister { code }
 *   GET  /port/debug?code=XXXX
 *
 * Each code maps to one PortSession Durable Object.
 * Each PortSession holds one guest WS + one client WS per registered port.
 * Raw bytes are spliced bidirectionally — zero protocol awareness.
 *
 * Deploying:
 *   cd relay-tunnel && npm install && npx wrangler deploy
 */

// ── HMAC-SHA256 signed tokens ─────────────────────────────────────────────────
// Requires TUNNEL_SECRET worker secret (optional, skipped if absent).
// Set via: npx wrangler secret put TUNNEL_SECRET

const TOKEN_TTL_SECS = 86400 * 30;

async function _getHmacKey(secret) {
  return crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign', 'verify'],
  );
}

async function signToken(code, secret) {
  const payload = { code, iat: Math.floor(Date.now() / 1000), exp: Math.floor(Date.now() / 1000) + TOKEN_TTL_SECS };
  const payloadBytes = new TextEncoder().encode(JSON.stringify(payload));
  const key = await _getHmacKey(secret);
  const sig = await crypto.subtle.sign('HMAC', key, payloadBytes);
  return btoa(JSON.stringify(payload)) + '.' + btoa(String.fromCharCode(...new Uint8Array(sig)));
}

async function verifyToken(token, secret) {
  try {
    const dot = token.lastIndexOf('.');
    if (dot === -1) return null;
    const payloadB64 = token.slice(0, dot);
    const sigB64 = token.slice(dot + 1);
    const payload = JSON.parse(atob(payloadB64));
    if (!payload.exp || Date.now() / 1000 > payload.exp) return null;
    const key = await _getHmacKey(secret);
    const sigBytes = Uint8Array.from(atob(sigB64), c => c.charCodeAt(0));
    const dataBytes = new TextEncoder().encode(JSON.stringify(payload));
    const valid = await crypto.subtle.verify('HMAC', key, sigBytes, dataBytes);
    return valid ? payload : null;
  } catch (_) { return null; }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function cors() {
  return {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET,POST,OPTIONS',
    'Access-Control-Allow-Headers': '*',
  };
}

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'Content-Type': 'application/json', ...cors() },
  });
}

const CODE_CHARS = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
function generateCode() {
  let c = '';
  for (let i = 0; i < 4; i++) c += CODE_CHARS[Math.floor(Math.random() * CODE_CHARS.length)];
  return c;
}

function normalizeCode(s) {
  if (!s) return null;
  const c = String(s).toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 4);
  return c.length === 4 ? c : null;
}

function parsePort(s) {
  const n = parseInt(s, 10);
  return (n > 0 && n < 65536) ? n : null;
}

function _concat(arrays) {
  let total = 0;
  for (const a of arrays) total += a.byteLength;
  const out = new Uint8Array(total);
  let off = 0;
  for (const a of arrays) { out.set(a, off); off += a.byteLength; }
  return out;
}

// ── PortSession Durable Object ────────────────────────────────────────────────
// One DO instance per pairing code.
// Holds: registeredPorts, guestWs[port], clientWs[port]

export class PortSession {
  constructor(state, _env) {
    this.state = state;
    this.created = Date.now();
    this.registeredPorts = new Set();
    this.guestWs = new Map();   // port → WebSocket
    this.clientWs = new Map();  // port → WebSocket
    this.guestAt = new Map();   // port → timestamp
    this.clientAt = new Map();  // port → timestamp
    // Buffer guest→client data that arrives before client connects
    // (e.g. SSH banner sent by sshd on TCP accept, dropped without buffering)
    this.guestBuffer = new Map(); // port → Array<data>
    this.BUFFER_MAX = 256;        // cap per port
    this.lastActivity = Date.now();
    this.IDLE_TTL_MS = 10 * 60 * 1000;
    // Per-port HTTP proxy call state: port → { chunks:[], resolve, resetIdle }
    // Used to route webSocketMessage bytes into an active _httpProxy call.
    this.proxyCalls = new Map();
    // Per-port "used guest" tracker: each guest WS wraps exactly one
    // tcp:127.0.0.1:PORT connection on the guest side. Once the first
    // client has paired, sshd/ftpd has already streamed its banner into
    // that pipe and is mid-protocol. Any subsequent client picking up the
    // same guest sees an empty buffer and protocol garbage ("Bad packet
    // length", "socket unexpectedly closed", SFTP upload failures). On
    // every NEW client attach we close the existing guest if the port has
    // already served a client — the guest-side respawn loop reopens a
    // fresh tcp accept and re-emits a clean banner before pairing.
    this.usedPorts = new Set();
    // Restore registered ports + rehydrate hibernated WebSockets.
    this.state.blockConcurrencyWhile(async () => {
      const ports = await this.state.storage.get('registeredPorts');
      if (Array.isArray(ports)) this.registeredPorts = new Set(ports);
      const created = await this.state.storage.get('created');
      if (typeof created === 'number') this.created = created;
      // Rehydrate WS refs from Hibernation API — survives DO eviction.
      for (const ws of this.state.getWebSockets('guest')) {
        const { port } = this._parseTags(ws);
        if (port != null) { this.guestWs.set(port, ws); this.guestAt.set(port, Date.now()); }
      }
      for (const ws of this.state.getWebSockets('client')) {
        const { port } = this._parseTags(ws);
        if (port != null) { this.clientWs.set(port, ws); this.clientAt.set(port, Date.now()); }
      }
    });
  }

  _parseTags(ws) {
    let role = null, port = null;
    for (const t of this.state.getTags(ws)) {
      if (t === 'guest' || t === 'client') role = t;
      else if (t.startsWith('port:')) port = parseInt(t.slice(5), 10);
    }
    return { role, port };
  }

  _cloneWsPayload(message) {
    if (message instanceof ArrayBuffer) {
      return message.slice(0);
    }
    if (typeof message === 'string') {
      return message;
    }
    if (message && message.byteLength != null) {
      const u8 = new Uint8Array(message.buffer || message, message.byteOffset || 0, message.byteLength);
      return u8.slice();
    }
    return message;
  }

  async fetch(request) {
    this.lastActivity = Date.now();
    const url = new URL(request.url);
    if (url.pathname.startsWith('/http')) return this._httpProxy(request, url);
    switch (url.pathname) {
      case '/register':  return this._register(request);
      case '/guest':     return this._accept(request, 'guest', url);
      case '/client':    return this._accept(request, 'client', url);
      case '/status':    return this._status();
      case '/unregister':return this._unregister();
      case '/debug':     return this._debug();
      default:           return new Response('not found', { status: 404 });
    }
  }

  async _register(request) {
    let ports = [];
    try {
      const body = await request.json();
      ports = (body.ports || []).map(Number).filter(p => p > 0 && p < 65536);
    } catch (_) {}
    if (!ports.length) return json({ error: 'ports required (non-empty array of port numbers)' }, 400);
    for (const p of ports) this.registeredPorts.add(p);
    await this.state.storage.put('registeredPorts', [...this.registeredPorts]);
    await this.state.storage.put('created', this.created);
    return json({ ok: true, registered_ports: [...this.registeredPorts] });
  }

  _accept(request, role, url) {
    const port = parsePort(url.searchParams.get('port'));
    if (!port) return new Response('missing or invalid port', { status: 400 });
    if (this.registeredPorts.size > 0 && !this.registeredPorts.has(port)) {
      return new Response(`port ${port} not registered for this code`, { status: 403 });
    }

    if (request.headers.get('Upgrade') !== 'websocket') {
      return new Response('WebSocket upgrade required', { status: 426 });
    }

    // Boot any existing WS on the same role+port (1:1 pairing)
    const existing = role === 'guest' ? this.guestWs.get(port) : this.clientWs.get(port);
    if (existing) {
      try { existing.close(1000, 'replaced'); } catch (_) {}
      if (role === 'guest') {
        this.guestWs.delete(port);
        this.guestAt.delete(port);
        // Drop any buffered bytes from the replaced guest's tcp:PORT accept.
        // Those bytes belong to a now-dead sshd/ftpd/etc. accept; mixing them
        // with the new bridge's fresh banner produces SSH KEX corruption.
        this.guestBuffer.delete(port);
      }
      else                  { this.clientWs.delete(port); this.clientAt.delete(port); }
    }

    // Force-fresh guest on every NEW client attach for already-used ports
    // (see usedPorts comment in constructor). Skip on the very first client
    // to avoid kicking the only live bridge before it has done its job.
    if (role === 'client' && this.usedPorts.has(port)) {
      const stale = this.guestWs.get(port);
      if (stale) {
        try { stale.close(1000, 'guest-recycle-fresh'); } catch (_) {}
        this.guestWs.delete(port);
        this.guestAt.delete(port);
        this.guestBuffer.delete(port);
      }
    }

    const pair = new WebSocketPair();
    const server = pair[1];
    // Hibernation API: Cloudflare manages WS lifecycle across DO eviction.
    // Tag with role + port so we can rehydrate refs after wake.
    this.state.acceptWebSocket(server, [role, `port:${port}`]);

    if (role === 'guest') {
      this.guestWs.set(port, server);
      this.guestAt.set(port, Date.now());
    } else {
      this.clientWs.set(port, server);
      this.clientAt.set(port, Date.now());
      // Mark this port as having served a client. The next client attach
      // will recycle the guest above before pairing.
      this.usedPorts.add(port);
      // Flush any buffered guest→client data (e.g. SSH banner)
      const buf = this.guestBuffer.get(port);
      if (buf && buf.length) {
        const buffered = [...buf];
        this.guestBuffer.delete(port);
        setTimeout(() => {
          for (const d of buffered) {
            try { server.send(d); } catch (_) {}
          }
        }, 0);
      }
    }

    return new Response(null, { status: 101, webSocket: pair[0] });
  }

  // Hibernation API handlers — Cloudflare invokes these for any acceptWebSocket'd
  // WS, surviving DO eviction. All bytes flow through here.
  async webSocketMessage(ws, message) {
    this.lastActivity = Date.now();
    const { role, port } = this._parseTags(ws);
    if (port == null) return;

    // If an _httpProxy call is waiting on this port, deliver bytes to it.
    const call = this.proxyCalls.get(port);
    if (call && role === 'guest') {
      let u8;
      if (message instanceof ArrayBuffer)       u8 = new Uint8Array(message);
      else if (typeof message === 'string')     u8 = new TextEncoder().encode(message);
      else if (message && message.byteLength != null) u8 = new Uint8Array(message.buffer || message);
      if (u8) { call.chunks.push(u8); call.resetIdle(); }
      return;
    }

    // Normal relay: forward to paired peer
    const peer = role === 'guest' ? this.clientWs.get(port) : this.guestWs.get(port);
    if (!peer) {
      if (role === 'guest') {
        let buf = this.guestBuffer.get(port);
        if (!buf) { buf = []; this.guestBuffer.set(port, buf); }
        buf.push(this._cloneWsPayload(message));
        if (buf.length > this.BUFFER_MAX) buf.shift();
      }
      return;
    }
    try { peer.send(message); } catch (_) {}
  }

  async webSocketClose(ws, code, reason, _wasClean) {
    const { role, port } = this._parseTags(ws);
    if (port == null) return;
    // IMPORTANT: only clear map entries if the closing WS is still the current
    // one for this (role, port). When _accept replaces an existing WS via
    // close(1000,'replaced'), the OLD ws's close handler fires asynchronously
    // — by then the NEW ws is already registered in the map. Unconditionally
    // deleting would clobber the new entry, leaving guestWs/clientWs empty
    // and stranding the live connection (status shows guest:true but bytes
    // never flow). This also caused the SSH banner corruption we observed:
    // bridge thrash + buffer clears mid-handshake left the next client peer
    // staring at mid-stream KEX-INIT bytes.
    if (role === 'guest') {
      if (this.guestWs.get(port) === ws) {
        this.guestWs.delete(port);
        this.guestAt.delete(port);
        this.guestBuffer.delete(port);
        // Forget "used" marker — the next guest that pairs is fresh.
        this.usedPorts.delete(port);
      } else {
        // Replaced WS closing — leave the new entry intact.
        return;
      }
    } else if (role === 'client') {
      if (this.clientWs.get(port) === ws) {
        this.clientWs.delete(port);
        this.clientAt.delete(port);
      } else {
        return;
      }
    }
    const peer = role === 'guest' ? this.clientWs.get(port) : this.guestWs.get(port);
    if (peer) {
      try { peer.close(code || 1000, reason || 'peer disconnected'); } catch (_) {}
    }
    const call = this.proxyCalls.get(port);
    if (call && role === 'guest') call.settle('close');
  }

  async webSocketError(ws, _err) {
    return this.webSocketClose(ws, 1011, 'peer error', false);
  }

  _status() {
    const now = Date.now();
    const guestPorts = [...this.guestWs.keys()];
    const clientPorts = [...this.clientWs.keys()];
    return json({
      registered_ports: [...this.registeredPorts],
      guest_ports: guestPorts,
      client_ports: clientPorts,
      paired_ports: guestPorts.filter(p => this.clientWs.has(p)),
      age_s: Math.floor((now - this.created) / 1000),
      idle_s: Math.floor((now - this.lastActivity) / 1000),
    });
  }

  async _unregister() {
    for (const ws of this.guestWs.values()) { try { ws.close(1000, 'unregistered'); } catch (_) {} }
    for (const ws of this.clientWs.values()) { try { ws.close(1000, 'unregistered'); } catch (_) {} }
    this.guestWs.clear();
    this.clientWs.clear();
    this.registeredPorts.clear();
    await this.state.storage.deleteAll();
    return json({ ok: true });
  }

  // HTTP/1.1 proxy: sends an HTTP request over the guest WS (which is bridged
  // to tcp:127.0.0.1:PORT inside the guest), parses the response bytes, returns
  // a Response. Each call consumes/closes the guest WS (single-shot), so the
  // guest-side respawn loop in tunnel-up.sh must be active for multiple calls.
  //
  // Requires: X-Tunnel-Port header (no URL parsing needed — main worker sets it).
  // Uses: Connection: close so TCP terminates cleanly.
  async _httpProxy(request, url) {
    const port = parsePort(request.headers.get('X-Tunnel-Port'));
    if (!port) return new Response('missing port', { status: 400, headers: cors() });
    const guestWs = this.guestWs.get(port);
    if (!guestWs) {
      return new Response(`guest not connected on port ${port}`, { status: 503, headers: cors() });
    }
    if (this.clientWs.has(port)) {
      return new Response(`port ${port} busy with TCP client`, { status: 409, headers: cors() });
    }

    // Derive upstream path: main worker routes to "/http" + guest path.
    const stripped = url.pathname.replace(/^\/http/, '') || '/';
    const guestPath = stripped + (url.search || '');
    const method = request.method;
    const bodyBytes = (method === 'GET' || method === 'HEAD')
      ? null
      : new Uint8Array(await request.arrayBuffer());

    // Build HTTP/1.1 request bytes.
    const hdrs = [];
    hdrs.push(`${method} ${guestPath} HTTP/1.1`);
    hdrs.push(`Host: guest.tunnel.local`);
    hdrs.push(`Connection: close`);
    hdrs.push(`User-Agent: traits-tunnel-proxy/1`);
    for (const h of ['accept', 'accept-encoding', 'range', 'content-type', 'cache-control']) {
      const v = request.headers.get(h);
      if (v) hdrs.push(`${h}: ${v}`);
    }
    if (bodyBytes) hdrs.push(`Content-Length: ${bodyBytes.byteLength}`);
    const reqHead = new TextEncoder().encode(hdrs.join('\r\n') + '\r\n\r\n');
    const reqBytes = bodyBytes ? _concat([reqHead, bodyBytes]) : reqHead;

    // Collect response bytes with idle + close detection.
    const chunks = [];
    let settle;
    const done = new Promise(r => { settle = r; });
    let idleTimer;
    const IDLE_MS = 1500;
    const resetIdle = () => { clearTimeout(idleTimer); idleTimer = setTimeout(() => settle('idle'), IDLE_MS); };

    // Register the call so webSocketMessage routes bytes here.
    // (Only one _httpProxy per port at a time — client lock above prevents overlap.)
    this.proxyCalls.set(port, { chunks, resetIdle, settle });

    try {
      guestWs.send(reqBytes);
    } catch (err) {
      this.proxyCalls.delete(port);
      return new Response('failed to send to guest', { status: 502, headers: cors() });
    }
    resetIdle();

    // Overall timeout for the whole proxy call.
    const timeout = new Promise(r => setTimeout(() => r('timeout'), 12000));
    await Promise.race([done, timeout]);
    clearTimeout(idleTimer);
    this.proxyCalls.delete(port);

    // Parse HTTP/1.1 response bytes.
    const total = chunks.reduce((s, c) => s + c.byteLength, 0);
    if (!total) return new Response('no response from guest', { status: 504, headers: cors() });
    const buf = new Uint8Array(total);
    let off = 0;
    for (const c of chunks) { buf.set(c, off); off += c.byteLength; }

    // Find \r\n\r\n header terminator.
    let he = -1;
    for (let i = 0; i + 3 < buf.length; i++) {
      if (buf[i] === 13 && buf[i + 1] === 10 && buf[i + 2] === 13 && buf[i + 3] === 10) { he = i; break; }
    }
    if (he < 0) return new Response('bad http response from guest', { status: 502, headers: cors() });

    const headStr = new TextDecoder().decode(buf.slice(0, he));
    const body = buf.slice(he + 4);
    const lines = headStr.split('\r\n');
    const statusMatch = lines[0].match(/^HTTP\/\d\.\d\s+(\d+)\s*(.*)$/);
    const status = statusMatch ? parseInt(statusMatch[1], 10) : 502;

    const outHeaders = new Headers();
    for (let i = 1; i < lines.length; i++) {
      const m = lines[i].match(/^([^:]+):\s*(.*)$/);
      if (!m) continue;
      const k = m[1].toLowerCase();
      // Skip hop-by-hop + length fields (Response recomputes them).
      if (['connection', 'transfer-encoding', 'keep-alive', 'content-length', 'content-encoding'].includes(k)) continue;
      try { outHeaders.append(m[1], m[2]); } catch (_) {}
    }
    outHeaders.set('access-control-allow-origin', '*');
    outHeaders.set('access-control-expose-headers', '*');
    // HEAD has no body.
    return new Response(method === 'HEAD' ? null : body, { status, headers: outHeaders });
  }

  _debug() {
    const now = Date.now();
    const ports = {};
    for (const p of this.registeredPorts) {
      ports[p] = {
        guest: this.guestWs.has(p),
        client: this.clientWs.has(p),
        guest_age_s: this.guestAt.has(p) ? Math.floor((now - this.guestAt.get(p)) / 1000) : null,
        client_age_s: this.clientAt.has(p) ? Math.floor((now - this.clientAt.get(p)) / 1000) : null,
      };
    }
    return json({
      created: new Date(this.created).toISOString(),
      age_s: Math.floor((now - this.created) / 1000),
      idle_s: Math.floor((now - this.lastActivity) / 1000),
      ports,
    });
  }
}

// ── Main Worker ───────────────────────────────────────────────────────────────

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (request.method === 'OPTIONS') {
      return new Response(null, { status: 204, headers: cors() });
    }

    if (url.pathname === '/health') {
      return new Response('ok', { headers: cors() });
    }

    // POST /port/register  { code?, ports:[22, 21, 22000] }
    if (url.pathname === '/port/register' && request.method === 'POST') {
      let body = {};
      try { body = await request.json(); } catch (_) {}
      const code = normalizeCode(body.code) || generateCode();
      const ports = (body.ports || []).map(Number).filter(p => p > 0 && p < 65536);
      if (!ports.length) return json({ error: 'ports required' }, 400);

      const stub = env.PORT_SESSION.get(env.PORT_SESSION.idFromName(code));
      await stub.fetch(new Request('http://do/register', {
        method: 'POST',
        body: JSON.stringify({ ports }),
        headers: { 'Content-Type': 'application/json' },
      }));

      let token = null;
      if (env.TUNNEL_SECRET) {
        token = await signToken(code, env.TUNNEL_SECRET);
      }

      return json({ code, token, ports, relay: 'wss://tunnel.traits.build' });
    }

    // WS /port/guest?code=XXXX&port=22
    if (url.pathname === '/port/guest') {
      const code = normalizeCode(url.searchParams.get('code'));
      if (!code) return json({ error: 'missing code' }, 400);
      return env.PORT_SESSION.get(env.PORT_SESSION.idFromName(code)).fetch(
        new Request(`http://do/guest?port=${url.searchParams.get('port') || ''}`, request),
      );
    }

    // WS /port/client?code=XXXX&port=22
    if (url.pathname === '/port/client') {
      const code = normalizeCode(url.searchParams.get('code'));
      // Accept signed token in place of code
      if (!code && request.headers.get('upgrade') === 'websocket' && url.searchParams.get('token') && env.TUNNEL_SECRET) {
        const payload = await verifyToken(url.searchParams.get('token'), env.TUNNEL_SECRET);
        if (!payload) return json({ error: 'Invalid or expired token' }, 401);
        return env.PORT_SESSION.get(env.PORT_SESSION.idFromName(payload.code)).fetch(
          new Request(`http://do/client?port=${url.searchParams.get('port') || ''}`, request),
        );
      }
      if (!code) return json({ error: 'missing code' }, 400);
      return env.PORT_SESSION.get(env.PORT_SESSION.idFromName(code)).fetch(
        new Request(`http://do/client?port=${url.searchParams.get('port') || ''}`, request),
      );
    }

    // GET/HEAD/POST /port/http/CODE/PORT/path  → HTTP-over-WS proxy to guest
    if (url.pathname.startsWith('/port/http/')) {
      const rest = url.pathname.slice('/port/http/'.length);
      // Format: CODE/PORT[/path...]
      const slash1 = rest.indexOf('/');
      if (slash1 < 0) return json({ error: 'expected /port/http/CODE/PORT/path' }, 400);
      const code = normalizeCode(rest.slice(0, slash1));
      if (!code) return json({ error: 'invalid code' }, 400);
      const afterCode = rest.slice(slash1 + 1);
      const slash2 = afterCode.indexOf('/');
      const portStr = slash2 < 0 ? afterCode : afterCode.slice(0, slash2);
      const port = parsePort(portStr);
      if (!port) return json({ error: 'invalid port' }, 400);
      const guestPath = slash2 < 0 ? '/' : '/' + afterCode.slice(slash2 + 1);

      // Forward to DO with pathname "/http" + guest path as search/suffix.
      const headers = new Headers(request.headers);
      headers.set('X-Tunnel-Port', String(port));
      const innerUrl = 'http://do/http' + guestPath + (url.search || '');
      return env.PORT_SESSION.get(env.PORT_SESSION.idFromName(code)).fetch(
        new Request(innerUrl, {
          method: request.method,
          headers,
          body: (request.method === 'GET' || request.method === 'HEAD') ? null : request.body,
        }),
      );
    }


    if (url.pathname === '/port/status' && request.method === 'GET') {
      let code = normalizeCode(url.searchParams.get('code'));
      if (!code && url.searchParams.get('token') && env.TUNNEL_SECRET) {
        const payload = await verifyToken(url.searchParams.get('token'), env.TUNNEL_SECRET);
        if (!payload) return json({ error: 'Invalid or expired token' }, 401);
        code = payload.code;
      }
      if (!code) return json({ error: 'missing code' }, 400);
      const res = await env.PORT_SESSION.get(env.PORT_SESSION.idFromName(code)).fetch(
        new Request('http://do/status'),
      );
      const data = await res.json();
      return json({ ...data, code });
    }

    // POST /port/unregister  { code }
    if (url.pathname === '/port/unregister' && request.method === 'POST') {
      const body = await request.json().catch(() => ({}));
      const code = normalizeCode(body.code);
      if (!code) return json({ error: 'missing code' }, 400);
      const res = await env.PORT_SESSION.get(env.PORT_SESSION.idFromName(code)).fetch(
        new Request('http://do/unregister', { method: 'POST' }),
      );
      return json(await res.json());
    }

    // GET /port/debug?code=XXXX
    if (url.pathname === '/port/debug' && request.method === 'GET') {
      const code = normalizeCode(url.searchParams.get('code'));
      if (!code) return json({ error: 'missing code' }, 400);
      const res = await env.PORT_SESSION.get(env.PORT_SESSION.idFromName(code)).fetch(
        new Request('http://do/debug'),
      );
      return json({ code, ...(await res.json()) });
    }

    return json({ error: 'not found' }, 404);
  },
};
