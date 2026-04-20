/**
 * Apptron Publishing Service Worker
 *
 * Serves files published from the Wanix guest /public directory.
 * The IDE page reads files from the VFS and caches them here;
 * subsequent navigation to those paths serves from the Cache API.
 */

const CACHE = 'apptron-publish-v1';

/* Paths that must always pass through to the network (SPA, static assets, API). */
const RESERVED = new Set([
  '/', '/index.html', '/publish-sw.js', '/CNAME', '/favicon.ico',
  '/404.html', '/README.md',
]);
const RESERVED_PREFIXES = [
  '/static/', '/traits/', '/relay/', '/health', '/docs/', '/_', '/.',
];

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET') return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  const path = url.pathname;
  if (RESERVED.has(path)) return;
  if (RESERVED_PREFIXES.some((p) => path.startsWith(p))) return;

  event.respondWith(
    caches.open(CACHE).then(async (cache) => {
      let cached = await cache.match(request);
      if (cached) return cached;

      // Directory-like path: try appending index.html
      if (path.endsWith('/')) {
        cached = await cache.match(new URL(path + 'index.html', url.origin));
        if (cached) return cached;
      }

      // Not published — fall through to network.
      return fetch(request);
    }),
  );
});

self.addEventListener('message', (event) => {
  const { data } = event;
  if (!data?.type) return;

  switch (data.type) {
    case 'publish-file': {
      const { path, body, contentType } = data;
      const resp = new Response(body, {
        headers: { 'Content-Type': contentType || 'application/octet-stream' },
      });
      event.waitUntil(
        caches.open(CACHE).then((c) =>
          c.put(new Request(new URL(path, self.location.origin)), resp),
        ),
      );
      break;
    }

    case 'unpublish-file': {
      const req = new Request(new URL(data.path, self.location.origin));
      event.waitUntil(caches.open(CACHE).then((c) => c.delete(req)));
      break;
    }

    case 'clear-published':
      event.waitUntil(caches.delete(CACHE));
      break;

    case 'list-published':
      event.waitUntil(
        caches
          .open(CACHE)
          .then((c) => c.keys())
          .then((keys) => {
            const paths = keys.map((r) => new URL(r.url).pathname);
            event.ports?.[0]?.postMessage({ paths });
          }),
      );
      break;
  }
});
