# pwa Specification

## Purpose

Define the installable, offline-capable shell that lets the
openaccounting web app behave like a native app on phones.

## Requirements

### Requirement: Web App Manifest

The binary SHALL serve a `static/manifest.webmanifest` at the
URL `/static/manifest.webmanifest` containing:

- `name`: "OpenAccounting"
- `short_name`: "OpenAcct"
- `start_url`: "/"
- `display`: "standalone"
- `theme_color`: "#0f172a" (slate-900)
- `background_color`: "#f8fafc" (slate-50)
- `icons`: 192x192, 512x512, and 512x512 maskable PNGs.

#### Scenario: Manifest validates

- **WHEN** the manifest is fetched and validated against the
  W3C web-app-manifest schema
- **THEN** validation passes; no required fields are missing.

### Requirement: Service Worker

The binary SHALL serve a service worker at `/static/sw.js` that
performs the following on `install`:

1. Precache `/`, `/static/css/app.css`, `/static/htmx.min.js`,
   and `/static/icons/icon-192.png`.

And on `fetch` for read-only `GET` requests:

1. If the URL matches a runtime-cache pattern
   (`/ledgers/*/dashboard`,
    `/ledgers/*/reports/balance-sheet`,
    `/ledgers/*/reports/trial-balance`,
    `/ledgers/*/reports/income-statement`,
    `/ledgers/*/reports/cash-flow`,
    `/ledgers/*/accounts`), return the cached response if
   fresh, otherwise fetch from the network, update the cache,
   and return the response.
2. POST requests, `/login`, `/register`, `/import/*`,
   `/reimbursements/*/submit`, `/approve`, `/reject`, `/pay`
   are NEVER cached — they always go to the network.

#### Scenario: Read-only route is served from cache when offline

- **WHEN** the service worker is active and the user visits
  `/ledgers/X/dashboard` while offline
- **THEN** the cached response from the last online visit is
  rendered; no network request is attempted.

#### Scenario: Write route always reaches the server

- **WHEN** the user POSTs `/ledgers/X/reimbursements/Y/approve`
  while online
- **THEN** the request is sent to the network; the service
  worker does not intercept it.

### Requirement: Install Prompt

The base template (`templates/base.html`) SHALL include a
partial `templates/partials/_pwa.html` that:

1. Registers `/static/sw.js` on every page load.
2. Listens for the `beforeinstallprompt` event and shows a
   small "Install OpenAccounting" button in the nav bar.

The button is shown only when the browser signals the event is
available; otherwise it is hidden.

#### Scenario: User installs the PWA on Android Chrome

- **WHEN** the user visits `/` on Android Chrome and the
  `beforeinstallprompt` event fires
- **THEN** the "Install" button appears in the nav bar;
  clicking it triggers the native install dialog.

### Requirement: Service-Worker-Allowed Header

The server SHALL respond to `GET /static/sw.js` with the header
`Service-Worker-Allowed: /` so the worker can intercept requests
outside its own scope.

#### Scenario: Header is present

- **WHEN** the browser fetches `/static/sw.js`
- **THEN** the response includes `Service-Worker-Allowed: /`.

### Requirement: No-JS Fallback

The service worker MUST NOT be required for the app to function.
Every server-rendered route MUST work without JavaScript. The
worker only ADDS offline / install behavior.
