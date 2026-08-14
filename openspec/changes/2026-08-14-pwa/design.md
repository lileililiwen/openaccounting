# PWA — Design

## Manifest

```json
{
  "name": "OpenAccounting",
  "short_name": "OpenAcct",
  "start_url": "/",
  "display": "standalone",
  "theme_color": "#0f172a",
  "background_color": "#f8fafc",
  "icons": [
    { "src": "/static/icons/icon-192.png",      "sizes": "192x192", "type": "image/png" },
    { "src": "/static/icons/icon-512.png",      "sizes": "512x512", "type": "image/png" },
    { "src": "/static/icons/icon-maskable-512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" }
  ]
}
```

## Service worker

```js
// static/sw.js
const PRECACHE = ['/', '/static/css/app.css',
                   '/static/htmx.min.js',
                   '/static/icons/icon-192.png'];

const RUNTIME_CACHE_PATTERNS = [
  /^\/ledgers\/[^/]+\/dashboard\/?$/,
  /^\/ledgers\/[^/]+\/reports\/(balance-sheet|trial-balance|income-statement|cash-flow|general-ledger|cash-flow-forecast)\/?$/,
  /^\/ledgers\/[^/]+\/accounts\/?$/,
];

self.addEventListener('install', (e) => {
  e.waitUntil(caches.open('precache-v1').then(c => c.addAll(PRECACHE)));
  self.skipWaiting();
});

self.addEventListener('activate', (e) => {
  e.waitUntil(clients.claim());
});

self.addEventListener('fetch', (e) => {
  const req = e.request;
  if (req.method !== 'GET') return;          // never cache POST
  const url = new URL(req.url);
  if (RUNTIME_CACHE_PATTERNS.some(p => p.test(url.pathname))) {
    e.respondWith(staleWhileRevalidate(req));
  }
  // else: network passthrough
});

async function staleWhileRevalidate(req) {
  const cache = await caches.open('runtime-v1');
  const cached = await cache.match(req);
  const network = fetch(req).then(r => { cache.put(req, r.clone()); return r; })
                            .catch(() => cached);
  return cached || network;
}
```

## Install prompt

```html
<!-- templates/partials/_pwa.html -->
<link rel="manifest" href="/static/manifest.webmanifest">
<meta name="theme-color" content="#0f172a">
<script>
let deferredPrompt = null;
window.addEventListener('beforeinstallprompt', (e) => {
  e.preventDefault();
  deferredPrompt = e;
  document.getElementById('pwa-install-btn').hidden = false;
});
document.getElementById('pwa-install-btn').addEventListener('click',
  async () => {
    if (!deferredPrompt) return;
    deferredPrompt.prompt();
    await deferredPrompt.userChoice;
    deferredPrompt = null;
    document.getElementById('pwa-install-btn').hidden = true;
  });
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('/static/sw.js');
}
</script>
```

## Headers

`src/main.rs` adds `Service-Worker-Allowed: /` to the response
of `GET /static/sw.js` via a custom tower layer.

## Tests

### HTTP (no browser required)

- `http_manifest_fetches_correctly` — `GET
  /static/manifest.webmanifest` returns 200 with the expected
  JSON.
- `http_sw_fetches_with_allowed_header` — `GET /static/sw.js`
  has `Service-Worker-Allowed: /`.
- `http_sw_cache_patterns_match` — assert the pattern regex
  matches the documented routes.

### Visual (manual)

- Open Chrome DevTools → Application → Service Workers →
  confirm `sw.js` is activated.
- Lighthouse PWA audit passes with score ≥ 90.

## References

- W3C Web App Manifest spec
- MDN Service Worker API
- Workbox patterns (we deliberately don't pull in Workbox to
  avoid a build step)
