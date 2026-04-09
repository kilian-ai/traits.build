// SPDX-License-Identifier: CC0-1.0
// Cross-Origin Isolation Service Worker
//
// Enables SharedArrayBuffer and Atomics by setting COOP + COEP headers on all
// same-origin responses. This is required for Linux/WASM which uses
// WebAssembly.Memory({ shared: true }) backed by SharedArrayBuffer.
//
// Uses COEP: credentialless (not require-corp) so CDN resources (xterm.js,
// fonts) still load without needing explicit CORP headers.
//
// Adapted from https://github.com/gzuidhof/coi-serviceworker (MIT)

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', e => e.waitUntil(self.clients.claim()));

async function handleFetch(e) {
    if (e.request.cache === 'only-if-cached' && e.request.mode !== 'same-origin') return;

    let r;
    try {
        r = await fetch(e.request);
    } catch (_) {
        return;
    }

    if (!r) return;

    const headers = new Headers(r.headers);
    headers.set('Cross-Origin-Opener-Policy', 'same-origin');
    headers.set('Cross-Origin-Embedder-Policy', 'credentialless');
    headers.set('Cross-Origin-Resource-Policy', 'same-origin');

    return new Response(r.body, {
        status: r.status,
        statusText: r.statusText,
        headers,
    });
}

self.addEventListener('fetch', e => {
    // Only intercept same-origin requests (we add COEP: credentialless so
    // cross-origin resources are fetched without credentials anyway).
    if (e.request.url.startsWith(self.location.origin)) {
        e.respondWith(handleFetch(e));
    }
});
