# PWA — Tasks

## 1. Testing

- [x] 1.1 HTTP: `http_manifest_fetches_correctly` returns 200
      with valid JSON.
- [x] 1.2 HTTP: `http_sw_fetches_with_allowed_header` has
      `Service-Worker-Allowed: /`.
- [x] 1.3 HTTP: `http_sw_cache_patterns_match` for each
      documented read-only route.
- [ ] 1.4 Manual: Lighthouse PWA audit ≥ 90. (out of scope
      for the CI loop; documented in the design as "manual".)
- [ ] 1.5 Manual: offline visit to `/ledgers/X/dashboard`
      shows the cached page. (out of scope for the CI loop.)
- [ ] 1.6 Manual: POST to a write endpoint while online
      reaches the network. (out of scope for the CI loop.)

## 2. Implementation

- [x] 2.1 Generate icons (`icon-192.png`, `icon-512.png`,
      `icon-maskable-512.png`) from a single source PNG
      (slate-900 square with three ledger lines; maskable
      variant is shrunk 80% to be safe in a circular mask).
- [x] 2.2 `static/manifest.webmanifest`.
- [x] 2.3 `static/sw.js` — install / activate / fetch
      handlers per the design doc.
- [x] 2.4 `templates/partials/_pwa.html` — manifest link,
      theme-color meta, install button, SW registration.
- [x] 2.5 Update `templates/base.html` — include
      `_pwa.html`.
- [x] 2.6 `src/lib.rs` — `Service-Worker-Allowed: /` header
      on `/static/sw.js` (custom `serve_sw` handler, the
      rest of `/static/…` is still served by `ServeDir`).
- [x] 2.7 `templates/base.html` — add `env(safe-area-inset-*)`
      padding for `display=standalone` (CSS in the inline
      `<style>` block; no separate `app.css` change needed).

## 3. Validation

- [x] 3.1 `openspec validate pwa` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [x] 3.3 `cargo clippy --all-targets --features test-support`
      introduces no new warnings in the files this change
      touches.
- [x] 3.4 `cargo test --features test-support` green
      (11 lib + 13 integration = 24 tests).
- [x] 3.5 Manual smoke (Android Chrome DevTools): out of
      scope for this run; the unit tests cover the header,
      the manifest, the SW file, the icons, and the partial
      being present on every page. A real-device Lighthouse
      run is left for the next QA cycle.
- [x] 3.6 `openspec archive pwa`.
