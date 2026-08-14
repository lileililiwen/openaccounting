// OpenAccounting service worker.
//
// Precache: the static assets the app needs to render at all.
// Runtime cache: read-only GETs against report / dashboard /
// accounts routes, using stale-while-revalidate so a slow
// network never blocks the UI.
//
// Anything that is a write (POST) or a write-adjacent
// endpoint (login, register, import, reimbursements) is
// always passed through to the network. Auth pages and
// write endpoints MUST NOT be cached, otherwise stale
// credentials or stale CSRF tokens would surface.

const PRECACHE = "oa-precache-v1";
const RUNTIME = "oa-runtime-v1";

const PRECACHE_URLS = [
  "/",
  "/static/css/app.css",
  "/static/htmx.min.js",
  "/static/icons/icon-192.png",
  "/static/icons/icon-512.png",
  "/static/icons/icon-maskable-512.png",
  "/static/manifest.webmanifest",
];

const RUNTIME_CACHE_PATTERNS = [
  /^\/ledgers\/[^/]+\/dashboard\/?$/,
  /^\/ledgers\/[^/]+\/reports\/(balance-sheet|trial-balance|income-statement|cash-flow|general-ledger)\/?$/,
  /^\/ledgers\/[^/]+\/accounts\/?$/,
];

const NEVER_CACHE_PATTERNS = [
  /^\/login/,
  /^\/register/,
  /^\/logout/,
  /^\/import/,
  /\/import\//,
  /\/reimbursements\/.*\/(submit|approve|reject|pay)/,
  /\/transactions\/new/,
  /\/accounts\/new/,
  /^\/admin\//,
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(PRECACHE).then((cache) => cache.addAll(PRECACHE_URLS))
  );
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(
        keys
          .filter((k) => k !== PRECACHE && k !== RUNTIME)
          .map((k) => caches.delete(k))
      );
      await self.clients.claim();
    })()
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") {
    return; // never cache POST/PUT/DELETE
  }
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) {
    return; // ignore cross-origin
  }
  if (NEVER_CACHE_PATTERNS.some((p) => p.test(url.pathname))) {
    return; // always network
  }
  if (RUNTIME_CACHE_PATTERNS.some((p) => p.test(url.pathname))) {
    event.respondWith(staleWhileRevalidate(req));
    return;
  }
  // For everything else (HTML navigations, other static
  // assets), pass through to the network. The browser's
  // normal cache handles static assets.
});

async function staleWhileRevalidate(request) {
  const cache = await caches.open(RUNTIME);
  const cached = await cache.match(request);
  const networkFetch = fetch(request)
    .then((response) => {
      if (response && response.status === 200) {
        cache.put(request, response.clone());
      }
      return response;
    })
    .catch(() => cached);
  return cached || networkFetch;
}
