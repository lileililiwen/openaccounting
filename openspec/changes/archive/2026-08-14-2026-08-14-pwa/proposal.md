# Add Progressive Web App (PWA) Support

## Why

openaccounting is a web app with no native mobile shell. Field
users (the README's "individuals and micro-enterprises") often
need to check balances, scan a receipt, or approve a claim from
a phone with patchy network. A PWA — installable, with offline
read-only cache — is the lowest-cost path to that UX without
shipping separate iOS / Android binaries.

ezBookkeeping ships a PWA; Firefly III does not. Closing this
gap makes the product viable on phones immediately, while the
larger `mobile-shells` change handles native-app parity later.

## What Changes

- New `static/manifest.webmanifest` declaring the app metadata
  (name, short_name, start_url, display=standalone,
  theme_color, background_color, icons).
- New `static/sw.js` service worker implementing:
  - **Precache** of `/`, `/static/*`, base CSS, the
    vendored `htmx.min.js`.
  - **Runtime cache** for read-only GET responses
    (`/ledgers/{id}/dashboard`,
     `/ledgers/{id}/reports/balance-sheet`, …) using
    stale-while-revalidate.
  - **Never cache** POSTs, auth pages, or `/import/*`,
    `/reimbursements/*` write endpoints.
- New `templates/partials/_pwa.html` included in `base.html`
  that registers the manifest + service worker and shows a
  small "Install" prompt.
- New `static/icons/` set (192x192, 512x512, maskable 512x512)
  in PNG.
- `Content-Security-Policy` updated to allow the service
  worker's `worker-src 'self'`.

## Capabilities

### New Capabilities

- `pwa` — installable, offline read-only progressive web app.

## Impact

- **New files:**
  - `static/manifest.webmanifest`
  - `static/sw.js`
  - `static/icons/icon-{192,512,maskable-512}.png`
  - `templates/partials/_pwa.html`
  - `tests/http/pwa.rs`
- **Modified files:**
  - `templates/base.html` — include `_pwa.html`.
  - `static/css/app.css` — add safe-area / display=standalone
    padding.
  - `src/main.rs` — set proper CSP and Service-Worker-Allowed
    header.

## Non-Goals

- Offline write support (offline POSTs queue and replay
  via Background Sync — separate, larger change).
- Push notifications (separate change).
- Native app packaging (separate `mobile-shells` change).
