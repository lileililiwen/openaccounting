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

### Changed
- Production mode now rejects known fallback credentials and
  placeholder secrets.
- docker-compose.yml APP_SECRET uses environment variable
  substitution with documented fallback.

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
