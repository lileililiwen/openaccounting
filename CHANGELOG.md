# Changelog

All notable changes to OpenAccounting will be documented in
this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Accounting assurance: canonical synthetic ledger fixtures with
  exact expected report outputs for trial balance, income
  statement, balance sheet, and cash flow.
- Import/export round-trip tests for JSON, beancount, and
  hledger formats.
- Treatment records for tax, FX, invoices/AR/AP, amortization,
  inventory, closing, reversals, and audit chain workflows.
- Compliance disclaimers in README and treatment record docs.
- Security and operations baseline: SECURITY.md, CONTRIBUTING.md,
  CODEOWNERS, CHANGELOG.md.
- Configuration tests for placeholder secrets, missing APP_ENV,
  insecure cookies, and invalid database settings.
- Dependency vulnerability and license scanning in CI.
- Backup/restore drill documentation and fixtures.
- Production configuration documentation.
- Release readiness: ROADMAP.md with delivery order and v1.0 exit
  gates, GOVERNANCE.md with merge rules and a 5-business-day triage
  SLA, issue/PR templates, operator docs (data-model ERD, admin
  runbook, sizing guide, API reference pointer), and
  docs-consistency CI checks (TBD markers, stale identity, doc
  paths, roadmap entries, ERD freshness).
- Operational hardening: operative security contact with disclosure
  process, threat model, and log-redaction policy; S3 backup target
  with retention enforcement; nightly restore-verification job;
  stated RTO/RPO with WAL-archive PITR procedure; opt-in OTel
  tracing with redacted spans; per-route rate-limit middleware
  (429 + Retry-After on auth, API, webhook, import routes);
  release SBOM (CycloneDX) plus signatures with a bundle-completeness
  gate.
- Professional close controls: per-ledger hard-close watermark with
  409 rejection of closed-date writes, audited reopen/override flow
  with mandatory reason, maker-checker approval queue for
  over-threshold journals (maker ≠ checker), `accountant` and
  `auditor` ledger roles (auditors read plus export only), and
  gapless per-ledger-per-year invoice numbering with void reasons
  and a gap report distinguishing voids from true gaps.

### Changed
- Production mode now rejects known fallback credentials and
  placeholder secrets.
- docker-compose.yml APP_SECRET uses environment variable
  substitution with documented fallback.
- The CI stale-identity check now runs via
  `scripts/check_identity.py` instead of an inline grep, fixing a
  self-match on its own pattern.

### Migration notes
- Migration `0058_pro_close_controls`: adds `closed_periods.closed_through`,
  `reopen_events`, `journal_approvals`, `invoice_sequences`,
  `ledgers.approval_threshold`, `invoices.void_reason`, widens
  `transactions.kind` with `pending` and ledger roles with
  `accountant`/`auditor`. Reversible (DOWN verified: revert + re-apply clean).
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
