# PWA — Tasks

## 1. Testing

- [ ] 1.1 HTTP: `http_manifest_fetches_correctly` returns 200
      with valid JSON.
- [ ] 1.2 HTTP: `http_sw_fetches_with_allowed_header` has
      `Service-Worker-Allowed: /`.
- [ ] 1.3 HTTP: `http_sw_cache_patterns_match` for each
      documented read-only route.
- [ ] 1.4 Manual: Lighthouse PWA audit ≥ 90.
- [ ] 1.5 Manual: offline visit to `/ledgers/X/dashboard`
      shows the cached page.
- [ ] 1.6 Manual: POST to a write endpoint while online
      reaches the network.

## 2. Implementation

- [ ] 2.1 Generate icons (`icon-192.png`, `icon-512.png`,
      `icon-maskable-512.png`) from a single source PNG.
- [ ] 2.2 `static/manifest.webmanifest`.
- [ ] 2.3 `static/sw.js` — install / activate / fetch
      handlers per the design doc.
- [ ] 2.4 `templates/partials/_pwa.html` — manifest link,
      theme-color meta, install button, SW registration.
- [ ] 2.5 Update `templates/base.html` — include
      `_pwa.html`.
- [ ] 2.6 `src/main.rs` — `Service-Worker-Allowed: /` header
      on `/static/sw.js`.
- [ ] 2.7 Update `static/css/app.css` — add safe-area padding
      for `display=standalone`.

## 3. Validation

- [ ] 3.1 `openspec validate pwa` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke (Android Chrome DevTools):
      register SW, install PWA, kill network, visit
      `/ledgers/X/dashboard` — confirm cached response.
- [ ] 3.6 `openspec archive pwa`.
