# WCAG 2.2 AA audit — OpenAccounting

| Field            | Value                                                       |
| ---------------- | ----------------------------------------------------------- |
| Audit date       | 2026-09-21                                                  |
| Standard         | WCAG 2.2 AA (`https://www.w3.org/TR/WCAG22/`)              |
| Scope            | Rendered server HTML for all auth and ledger routes, plus HTMX partial swaps and chart containers. |
| Auditor          | OpenSpec change `ux-a11y-mobile` (BFS→DFS→BFS) + `axe-core` 4.10 on rendered fixtures. |
| P1 findings      | 4 open at start → 0 open at archive (all remediated in this change). |
| P2 findings      | 5 — informational, tracked but not release-blocking.         |
| P3 findings      | 3 — future work.                                            |
| Test support     | `tests/integration/ux_a11y.rs`, `tests/integration/chart_a11y.rs`, `tests/integration/locale_coverage.rs`, `tests/integration/mobile_promise.rs`. |
| Release claim    | Conformance **gated** by `scripts/check_a11y_audit.py` — README may not claim AA while a P1 is open. |

## Methodology

1. `axe-core` 4.10 was driven against rendered HTML for
   `/login`, `/register`, `/ledgers`, `/ledgers/{id}/dashboard`,
   `/ledgers/{id}/transactions/new`, `/ledgers/{id}/reports/balance-sheet`,
   and the reconciliation session page (`/ledgers/{id}/rec_session`).
2. HTMX partial swaps were exercised in `tests/integration/ux_a11y.rs`
   by issuing a POST and asserting the response carries the
   `data-htmx-focus` and `data-htmx-announce` markers defined
   below. The JavaScript handler in `static/js/a11y.js` reads
   these markers after `htmx:afterSwap` and moves focus to the
   named heading and announces the text via the `aria-live`
   region.
3. Charts were audited by inspecting the rendered SVG for
   `role="img"` + `aria-label` + a visually-hidden data table.
   See `tests/integration/chart_a11y.rs` for the assertion
   driver.
4. Locale coverage was audited by running
   `scripts/check_locale_coverage.py` against the live tree
   and against fixture catalogs with deliberately missing
   keys; the fixture case must fail the gate.

## P1 findings (release-blocking)

| ID  | Finding                                                                   | Fix shipped in this change                                                                                                                          |
| --- | ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1  | HTMX partial swaps do not move focus and do not announce completion.      | `static/js/a11y.js` listens for `htmx:afterSwap`; reads `data-htmx-focus` / `data-htmx-announce` markers; focuses the named element; announces text. |
| A2  | `render_line` and `render_donut` emit raw SVG with no accessible alternative. | Both helpers now wrap their output in a `<figure>` carrying `role="img"` + `aria-label` + a visually-hidden `<table>` data summary.                 |
| A3  | No CI gate on missing translations; locales can drift silently.          | `scripts/check_locale_coverage.py` (above) compares every locale against `static/locales/en.json` and fails above 5 % missing keys for the six day-1 locales.       |
| A4  | README non-goals vs `mobile/README.md` could diverge — no enforcement.    | `scripts/check_mobile_promise.py` requires both files to carry the same promise; the change retires `mobile/` and points the mobile README at PWA.  |

Each P1 is covered by a passing integration test named after
its ID (`ux_a11y::a1_focus_and_announce`,
`chart_a11y::render_line_is_accessible`,
`locale_coverage::gate_fails_above_threshold`,
`mobile_promise::docs_agree`).

## P2 findings (informational, tracked)

- **A5** Color-only emphasis on the cash-runway widget.
  Currently red/amber/emerald is the only signal. Status text
  ("safe", "watch", "tight") is rendered separately but is
  visually small. **Plan**: bump the textual signal to the
  same font weight as the metric. Owner: dashboard widget.
- **A6** `templates/rec_session/page.html` session-status
  pill is a single span with color; add a leading icon and
  full word ("Open"/"Closed").
- **A7** Modal dialogs do not trap focus while open.
  Acceptable for v0.1 since modals are rare (creation forms);
  revisit when adding the dashboard builder modal.
- **A8** Form validation errors render inline with red text
  only. **Plan**: pair each error with an inline `<svg>` and
  `aria-live="assertive"`. Owner: forms.
- **A9** The skip-link is rendered visually on focus only
  but not in the `tab order`. Tested manually and works
  because focus-on-load moves it; consider an `autofocus`
  attribute.

## P3 findings (future work)

- **A10** Charts ship a hidden `<table>` but the table
  headers do not repeat on long series. Acceptable for
  day-1 series sizes; revisit if report widgets ever show
  more than 24 buckets.
- **A11** `mobile/` Capacitor shell exists in the tree but
  no native binaries are produced. The change retires
  the shell (A4); `mobile/README.md` now points at PWA
  install only.
- **A12** No high-contrast (`forced-colors`) media-query
  overrides for chart fills. Add CSS custom-property
  fall-backs when the report widget gets a dark theme
  pass.

## Release gating

`scripts/check_a11y_audit.py` enforces:

1. No P1 finding id `A1..A4` may appear with `status: open`
   while the README claims AA conformance.
2. The locale coverage artifact (`docs/locale-coverage.json`)
   must exist and pass its threshold.
3. The mobile promise check (A4) must pass.

The CI workflow `.github/workflows/a11y.yml` (added in this
change) runs all three. The mobile promise is also re-checked
inside `tests/integration/docs_consistency.rs::mobile_promise`
so a docs-lint regression trips the test suite.

## Re-test plan

Re-run this audit after every change that touches:

- `templates/**.html`
- `static/js/htmx-csrf.js`, `static/js/a11y.js`
- `src/charts/`
- `static/locales/*.json`
- `static/sw.js` (mobile offline behaviour)

The integration tests in `tests/integration/` cover the
deterministic paths; manual screen-reader pass is the
non-automated half and is owned by the human reviewer of
this report.