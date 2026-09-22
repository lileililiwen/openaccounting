# Changelog

All notable changes to OpenAccounting will be documented in
this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Accessibility and mobile-promise capability (`ux-a11y`): dated
  WCAG 2.2 AA audit at `docs/wcag-audit-2026-09-21.md` with 4 P1
  findings all remediated at archive; HTMX focus management plus
  aria-live announcements via `static/js/a11y.js` reading
  `data-htmx-focus` and `data-htmx-announce` markers; chart
  accessibility wrapping every SVG in `<figure role="img">` with an
  `aria-label` summary and a visually-hidden `<table>` data summary;
  locale coverage gate (`scripts/check_locale_coverage.py`)
  publishing `docs/locale-coverage.json` and failing above 5 % missing
  keys for the six day-1 locales; explicit mobile promise linted
  between `README.md` and `mobile/README.md`
  (`scripts/check_mobile_promise.py`); CI gate
  `.github/workflows/a11y.yml` runs the three docs-lint scripts.
- Agent infrastructure: canonical `AGENTS.md` plus
  `.ai-rules/{workflow,completion,architecture}.md`,
  `scripts/check-openspec-change-names.mjs`, and `.agentignore`; the
  legacy `Agents.md` is retained as a compatibility shim.

### Changed
- Retired the `mobile/` Capacitor shell; the project ships a single
  Rust binary with an installable PWA (manifest, service worker, and
  install button) as the supported mobile story. The
  `.github/workflows/mobile-build.yml` job is a documented no-op
  pointing at `mobile/README.md` and the WCAG audit finding A11.
- Three `tax_transactions` integration tests updated to strip the
  `?posted=1` a11y focus marker from the redirect URL before parsing
  the path-tail UUID.

### Migration notes
- No new migrations. The 8 previously archived packages each shipped
  their own reversible migration; the highest number currently in
  the tree is 0060 (compliance-exports). `sqlx migrate run --source
  migrations` is a no-op on a fully-applied checkout.
- To upgrade a checkout, run `sqlx migrate run --source migrations`
  (renders a no-op when already current). Every migration ships a
  DOWN block verified by the `migrations-reversible` CI job; revert
  with `scripts/migrate-down.sh` only after taking a backup per
  `docs/backup-restore.md`.

## [0.1.0] - 2026-08-13

### Added
- Initial release with double-entry bookkeeping engine.
- Chart of accounts, balanced transactions, document uploads.
- Reports: trial balance, balance sheet, income statement,
  cash flow, general ledger, AR/AP aging.
- Authentication with Argon2 password hashing.
- Session management with PostgreSQL-backed sessions.
- Responsive web UI with HTMX + Tailwind CSS.
- Data import from OFX, QIF, MT940, CSV, WeChat, Alipay.
- Data export to JSON, beancount, hledger formats.
- Document storage with filesystem backend.
- Audit trail with append-only hash chain.
- Multi-currency support with FX gain/loss tracking.
- Period-close and closing entries.
- Reversible database migrations.
