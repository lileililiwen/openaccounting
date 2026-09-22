# OpenAccounting Mobile

> **Status (2026-09-21):** Retired. The Capacitor shell that
> previously lived under `mobile/` has been removed. See the
> OpenSpec change `ux-a11y-mobile` for the audit trail and
> `docs/wcag-audit-2026-09-21.md` finding A11.

OpenAccounting is a single-binary Rust web application with a
responsive HTML UI. The mobile story is the installable PWA
that ships in the binary itself — see
[`/docs/admin-runbook.md`](../docs/admin-runbook.md) for the
PWA manifest, service worker scope, and install-button partial.

## Why no native shell

- The single-binary strategy forbids a parallel native build
  pipeline (`mobile/README.md` history: `2026-08-14` change).
- The Capacitor shell pointed at `app.example.com` but never
  shipped a receipt-capture flow, so it added maintenance for
  no user value (`u13-ux-a11y-mobile` design decision).
- Browser-native PWA install already covers the "Add to Home
  Screen" path on iOS and Android without a binary download.

## Install the PWA

The binary serves:

- `/static/manifest.webmanifest` — install metadata.
- `/static/sw.js` — service worker, precaches the shell, caches
  read-only routes (`/ledgers/{id}/dashboard` and the report
  routes) stale-while-revalidate.
- `templates/partials/_pwa.html` — install button that surfaces
  on `beforeinstallprompt`.

## Promise (single source of truth)

> Native mobile apps (iOS / Android) are **not** shipped.
> Install the responsive web UI as a PWA on phones and tablets.
> The offline scope is the cached read-only routes only — there
> is no offline write queue.

The same statement appears verbatim in the project root
`README.md` under "Non-Goals (for v1)" so the two files agree
and the docs-lint gate (`scripts/check_mobile_promise.py`)
passes.