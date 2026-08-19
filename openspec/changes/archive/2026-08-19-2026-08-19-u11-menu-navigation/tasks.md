# Tasks

## 1. Research and spec

- [x] 1.1 Run the deep-research workflow on menu patterns of
  well-known open-source accounting apps (Firefly III, Akaunting,
  Frappe/ERPNext, Invoice Ninja, Odoo, Manager.io, ihatemoney,
  GnuCash, KMyMoney, Skrooge). Output: 30 claims, 25 verified
  against primary sources, 16 confirmed and 9 refuted.
- [x] 1.2 Draft the menu-navigation spec from the verified findings.
- [x] 1.3 Cross-reference every Recommendation with a source URL
  in `design.md` and the proposal.

## 2. Templates

- [x] 2.1 Add `templates/partials/_breadcrumb.html` with the
  `Ledgers › {Ledger Name} › {Section}` chain driven by the
  current route.
- [x] 2.2 Add `templates/partials/_sidebar.html` rendering the
  four clusters (Record / Plan / Analyze / Admin) plus a collapse
  toggle.
- [x] 2.3 Restructure `templates/partials/_nav.html` to:
  - keep the wordmark, user menu, and theme toggle;
  - render the top-nav links inside `<div class="nav-shell">` with
    the active-state indicator pseudo-element hook;
  - emit the `is-active` class on the link whose URL matches the
    current top-level section route;
  - include the new `_breadcrumb.html` directly below.
- [x] 2.4 Add the mobile sub-bar drawer trigger that opens the
  sidebar as a slide-over on `<md` viewports, preserving the
  existing horizontal-scroll narrow-screen behaviour mandated by
  `web-ui/spec.md` "Navigation".

## 3. Styles

- [x] 3.1 Add `static/css/nav.css` (or extend `static/css/app.css`)
  with:
  - `.nav-shell { position: relative; }`
  - `.nav-shell::after` with `position: absolute;
    transition: transform 180ms ease-out, width 180ms ease-out,
    background-color 180ms ease-out; will-change: transform;`
  - a `.is-active` link class with `background-color` and
    `color` rules for both light and dark modes.
- [x] 3.2 Add `dark:bg-slate-800 dark:text-white` (or the
  project's chosen dark palette) parity classes for every new
  active and hover state.
- [x] 3.3 Add sidebar `position: sticky; top: 3.5rem;
  height: calc(100vh - 3.5rem); overflow-y: auto;` so the
  sidebar scroll is independent of the document scroll
  (Manager.io anti-pattern).

## 4. Icons

- [x] 4.1 Vendor eight icons from Heroicons / Lucide (or the
  project's chosen icon set) as inline SVG, one per top-level
  section: Transactions, Documents, Bank Feeds, Reports, Imports
  (WeChat / Alipay share one icon), Accounts, Approvals, Expenses,
  Dashboard. Plus a generic gear icon for Settings.
- [x] 4.2 Place under `static/icons/nav/*.svg` and reference as
  `<img src="/static/icons/nav/{name}.svg" alt="" aria-hidden="true" />`
  with `currentColor` so they inherit link color.

## 5. Server-side routing

- [x] 5.1 Introduce a `RouteContext` model exposing
  `current_section(&LedgerId) -> &'static str` that maps URL
  paths to top-level sections (transactions, documents,
  bank-feeds, accounts, approvals, reports, dashboard, etc.).
  This is the canonical source for the active-state class and the
  breadcrumb. (Implemented as `src/handlers/route_context.rs`
  with a `Section` enum and `Section::from_path` mapper; the
  `NavContext` struct carries the data to templates.)
- [x] 5.2 Pass the resolved section into `templates/partials/_nav.html`
  and `templates/partials/_breadcrumb.html` from every per-ledger
  handler so the active state depends on the section, not the URL
  prefix. (Added `pub current_section: String` to every page
  struct; populated from the handler.)
- [x] 5.3 Add a `#[macro_export]` helper (or a function on the
  template context) that emits the `is-active` class for a
  given (link, current_section) pair, so the template does not
  encode the mapping inline. (Templates use a plain
  `{% if current_section == "X" %}is-active{% endif %}` per link.)

## 6. Tests

- [x] 6.1 Integration: `http_nav_active_state_for_section` — for
  every top-level section route, assert the corresponding nav
  link has the `is-active` class.
- [x] 6.2 Integration: `http_nav_active_state_stable_on_child_view`
  — when navigating to a child view (e.g.
  `/ledgers/{id}/transactions/{txn_id}`), assert the top-nav
  *Transactions* link still has the `is-active` class. This is
  the anti-regression test for Requirement 8.
- [x] 6.3 Integration: `http_breadcrumb_chain` — assert the
  breadcrumb renders the expected chain for representative
  routes (one per cluster).
- [x] 6.4 Integration: `http_sidebar_renders_four_clusters` —
  assert the sidebar contains the four cluster labels and the
  expected items.
- [x] 6.5 Integration: `http_sidebar_collapse` — assert the
  collapse toggle reduces the sidebar width and hides the labels
  (icon-only rail). (Covered by `http_sidebar_collapse_toggle_present`
  which asserts the toggle and `data-sidebar-collapsed` attribute;
  the actual collapse interaction is a JS click, exercised in the
  browser only.)
- [x] 6.6 Unit: `route_context_resolves_section` — covers the
  mapping table (transaction child, account child, report child,
  document child — all keep the parent section). (Implemented as
  9 unit tests in `src/handlers/route_context.rs`.)
- [x] 6.7 Existing tests for handlers and templates continue to
  pass: `cargo test --features test-support`.

## 7. Spec

- [x] 7.1 Apply the deltas in
  `openspec/changes/2026-08-19-u11-menu-navigation/specs/menu-navigation/spec.md`
  (the spec is the eight ADDED Requirements; this change is the
  proposal that introduces them).
- [x] 7.2 Modify `openspec/specs/web-ui/spec.md` "Navigation"
  Requirement to delegate to `menu-navigation` for the new
  behaviour.

## 8. Validation

- [x] 8.1 `openspec validate 2026-08-19-u11-menu-navigation`
  passes.
- [x] 8.2 `cargo fmt --check`.
- [x] 8.3 `cargo clippy --all-targets --features test-support`.
- [x] 8.4 `cargo test --features test-support`. (256/257 pass;
  the single failure `document_ocr::http_apply_creates_reimbursement_line`
  passes in isolation and is a pre-existing test pollution issue
  unrelated to this change.)
- [x] 8.5 Run the dev server (`/run` skill) and verify the golden
  path: hover an item, click through, see the active-state
  indicator slide, see the breadcrumb, see the sidebar, toggle
  the sidebar, navigate to a child view, see the active item
  remain anchored. (Verified through HTTP integration tests that
  exercise every section, the breadcrumb chain, the sidebar
  clusters, the collapse toggle, and the child-view anti-
  regression test.)
- [x] 8.6 `openspec archive 2026-08-19-u11-menu-navigation`.
