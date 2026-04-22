/**
 * Apptron IDE Publishing Service Worker (scoped)
 *
 * This worker is scoped to /static/apptron-ide/ and only intercepts
 * requests under /static/apptron-ide/public/.
 */

const CACHE = 'apptron-publish-v1';
const PUBLISHED_PREFIX = '/static/apptron-ide/public/';

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET') return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  const path = url.pathname;
  if (!path.startsWith(PUBLISHED_PREFIX)) return;

  event.respondWith(
    caches.open(CACHE).then(async (cache) => {
      let cached = await cache.match(request, { ignoreSearch: true });
      if (cached) return cached;

      if (path.endsWith('/')) {
        cached = await cache.match(new URL(path + 'index.html', url.origin), { ignoreSearch: true });
        if (cached) return cached;
      }

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
        headers: {
          'Content-Type': contentType || 'application/octet-stream',
          'Cache-Control': 'no-store, max-age=0, must-revalidate',
          Pragma: 'no-cache',
          Expires: '0',
          'X-Apptron-Published-At': String(Date.now()),
        },
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
