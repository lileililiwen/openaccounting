# web-ui Specification

## Purpose
TBD - created by archiving change bootstrap-double-entry-bookkeeping-engine. Update Purpose after archive.
## Requirements
### Requirement: Responsive Layout

All pages SHALL be built on Tailwind CSS, mobile-first, and use a
**fluid layout** that adapts without horizontal scroll on screens as
narrow as 360 px. Concretely:

- A single `max-w-*` container is centred with `mx-auto`.
- Multi-column grids use `grid-cols-1` at the smallest breakpoint and
  scale up at `sm` (≥ 640 px), `md` (≥ 768 px), `lg` (≥ 1024 px), and
  `xl` (≥ 1280 px).
- Tables render as tables on `md` and above; on smaller screens the
  same data renders as a card list (`<ul>` of `<li>`s).

The base stylesheet is delivered via the Tailwind Play CDN in
development. The vendored CSS file `static/css/app.css` provides a
small set of utility classes (`.tabular`) that the CDN does not
cover. In production the README documents swapping the Play CDN for
the standalone Tailwind CLI.

#### Scenario: Phone-width dashboard

- **WHEN** the user opens `/ledgers/{id}/dashboard` on a 360 px
  viewport
- **THEN** the KPI grid is a single column, the charts stack
  vertically, and no horizontal scrollbar appears.

#### Scenario: Desktop dashboard

- **WHEN** the same page is rendered on a 1280 px viewport
- **THEN** the KPI grid is six columns, the line chart spans two
  thirds of the row, and the donut chart sits to its right.

### Requirement: HTMX-Driven Partial Updates

The application SHALL include HTMX (vendored at
`static/htmx.min.js`, BSD-0) on every page. v0.1 uses HTMX in a
limited way:

- The "Add line" button on the new-transaction form is a small
  inlined JS helper (HTMX is overkill for cloning a row).
- Future changes (filter chips, infinite scroll on the transactions
  list) will add HTMX-driven partial updates.

The page MUST remain functional with JavaScript disabled: every
form submits a full HTML response, and the response is a full page
(or a 303 redirect), not a JSON blob.

#### Scenario: Form works without JS

- **WHEN** a user disables JavaScript in their browser and submits
  the new-transaction form
- **THEN** the transaction is created (or an error is shown) via a
  full-page response. No silent failure.

### Requirement: Navigation

A sticky top navigation bar SHALL appear on every authenticated page,
showing:

- The "OA" wordmark (links to `/`).
- On `md+` viewports: links to Dashboard, Transactions, Accounts,
  Documents, Reports (when a ledger is selected).
- On `<md` viewports: a horizontally scrollable sub-bar with the
  same links.
- The username and a "Log out" button (form POST to `/logout`) on
  the right.

The navigation MUST hide entirely on `/login` and `/register`.

#### Scenario: Mobile sub-nav scrolls horizontally

- **WHEN** the user opens a dashboard on a phone
- **THEN** the sub-nav is a thin strip below the main nav, with
  links that scroll horizontally without truncating.

### Requirement: Accessibility Baseline

Pages SHALL meet a basic accessibility baseline:

- Every form input has a `<label>` with a `for` attribute or
  wraps the input.
- Every interactive element reachable by keyboard has a visible
  focus ring (Tailwind default `focus:ring-2 focus:ring-slate-500`
  is acceptable).
- Colour is not the only signal: the trial balance balance banner
  uses both colour and an icon (✓/✗) and a textual label.
- All `<img>` tags have an `alt` attribute (HTMX image previews
  not shipped in v0.1).

#### Scenario: Keyboard-only user can record a transaction

- **WHEN** a keyboard-only user opens the new-transaction form
- **THEN** they can fill every field and submit it using only Tab,
  Shift+Tab, and Enter, with a visible focus ring on each control.

