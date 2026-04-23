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

// ── PortSession Durable Object ────────────────────────────────────────────────
// One DO instance per pairing code.
// Holds: registeredPorts, guestWs[port], clientWs[port]

export class PortSession {
  constructor(state, _env) {
    this.created = Date.now();
    this.registeredPorts = new Set();
    this.guestWs = new Map();   // port → WebSocket
    this.clientWs = new Map();  // port → WebSocket
    this.guestAt = new Map();   // port → timestamp
    this.clientAt = new Map();  // port → timestamp
    // Eviction: idle TTL enforced lazily on each fetch
    this.lastActivity = Date.now();
    this.IDLE_TTL_MS = 10 * 60 * 1000; // 10 minutes
  }

  async fetch(request) {
    this.lastActivity = Date.now();
    const url = new URL(request.url);
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
    }

    const pair = new WebSocketPair();
    const server = pair[1];
    server.accept();

    if (role === 'guest') {
      this.guestWs.set(port, server);
      this.guestAt.set(port, Date.now());
    } else {
      this.clientWs.set(port, server);
      this.clientAt.set(port, Date.now());
    }

    const getOther = () => role === 'guest' ? this.clientWs.get(port) : this.guestWs.get(port);

    server.addEventListener('message', (ev) => {
      this.lastActivity = Date.now();
      const peer = getOther();
      if (!peer) return; // buffer: drop until other side joins
      try { peer.send(ev.data); } catch (_) {}
    });

    const teardown = (code, reason) => {
      if (role === 'guest') this.guestWs.delete(port);
      else this.clientWs.delete(port);
      const peer = getOther();
      if (peer) {
        try { peer.close(code || 1000, reason || 'peer disconnected'); } catch (_) {}
      }
    };
    server.addEventListener('close', (ev) => teardown(ev.code, ev.reason));
    server.addEventListener('error', () => teardown(1011, 'peer error'));

    return new Response(null, { status: 101, webSocket: pair[0] });
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

  _unregister() {
    for (const ws of this.guestWs.values()) { try { ws.close(1000, 'unregistered'); } catch (_) {} }
    for (const ws of this.clientWs.values()) { try { ws.close(1000, 'unregistered'); } catch (_) {} }
    this.guestWs.clear();
    this.clientWs.clear();
    this.registeredPorts.clear();
    return json({ ok: true });
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

    // GET /port/status?code=XXXX
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
