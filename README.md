# OpenAccounting

**Open-source double-entry bookkeeping for individuals and startups.**

Web-based, responsive, document-aware. Single binary, PostgreSQL backend, HTMX + Tailwind UI.

## Project status and contributor docs

[![Triage SLA](https://img.shields.io/badge/triage-ack%20in%205%20business%20days-blue)](GOVERNANCE.md#triage-sla)

OpenAccounting is v0.1-alpha. The application and core accounting workflows
are implemented, while the active hardening and expansion queue remains
OpenSpec-tracked work. Start with [`AGENTS.md`](AGENTS.md),
[`HANDOFF.md`](HANDOFF.md), and [`ROADMAP.md`](ROADMAP.md) before changing
the repository. Contribution process and triage expectations are in
[`GOVERNANCE.md`](GOVERNANCE.md). Operational and accounting treatment guides are in `docs/`.

---

## Acknowledgements — Built Upon [vito](https://github.com/9-8-7-6/vito)

This project is built upon the excellent work of **[vito](https://github.com/9-8-7-6/vito)** by
[9-8-7-6](https://github.com/9-8-7-6), licensed under the MIT License.

vito provided the original Axum-based project skeleton, authentication scaffolding
(`axum-login` + Argon2), `sqlx` migration setup, Docker layout, and tower middleware
patterns. OpenAccounting is a domain-focused rewrite that keeps that skeleton and
replaces the asset-tracking core with a true **double-entry bookkeeping** engine,
adds a server-rendered web UI (HTMX + Tailwind), document uploads, reports, and
data visualization.

We are grateful to the vito authors and contributors. Their MIT-licensed work made
this project possible.

> vito is asset-tracking focused (single-entry, stocks/crypto/real estate, PostgreSQL).
> OpenAccounting keeps vito's project structure but rewrites the core schema to
> support proper double-entry (Chart of Accounts → Transactions → balanced Postings).

---

## First-Principles Analysis

A bookkeeping system, stripped to its essence, must answer three questions:

1. **How much money do I have, and where?** — *Balances by account, at a point in time.*
2. **Where did the money come from, and where did it go?** — *Categorized flows over a period.*
3. **Can I prove it?** — *Source documents attached to each event.*

From these questions, the irreducible concepts fall out:

| Concept | Meaning | Why it is essential |
|---|---|---|
| **Ledger** | A self-contained set of books (one per individual, business, or project) | Keeps books separate; enables multi-entity |
| **Account** | A category that holds or measures money (asset / liability / equity / income / expense) | The chart of accounts — every cent lives somewhere |
| **Transaction** | A dated business event with a description and payee | The atomic unit of record |
| **Posting** | A debit or credit leg on one account, within one transaction | The double-entry invariant: postings per transaction sum to zero |
| **Document** | A file (receipt, invoice, payment slip) linked to a transaction | The proof — and the user's primary evidence |
| **Period** | A date range used for reporting | Without a period, totals are meaningless |

**The double-entry invariant** — `Σ debits == Σ credits` for every transaction — is not
a stylistic choice. It is the only way to make `Assets = Liabilities + Equity`
provably true at any point in time. Cash-basis single-entry cannot produce a
balance sheet; double-entry can, automatically.

---

## Features

Status legend: ✅ shipped · 🧪 experimental · 🔌 provider-dependent · 📦 optional

Core bookkeeping
- ✅ **Double-entry bookkeeping** — every transaction is balanced; cannot be saved otherwise.
- ✅ **Chart of Accounts** — five account types (Asset, Liability, Equity, Income, Expense); default chart seeded. Edit/rename accounts and archive unused ones (they stay in reports); record **opening balances** when starting the books.
- ✅ **Transactions** — dated, described, with 2+ postings. Multi-leg splits supported.
- ✅ **Document upload** — attach receipts, invoices, payment slips (images + PDFs) to any transaction.
- ✅ **Reports** — every rendered report is discoverable from the report index (trial balance, balance sheet, income statement, cash flow, general ledger, AR/AP aging, cash-flow forecast, budget vs actual, tax summary, amortization), and closed fiscal periods show a "FY… is closed — these figures are final." banner.
- ✅ **Visualizations** (server-rendered SVG, zero JS chart library) — income vs. expense over time, expense breakdown by category, account balance trends.
- ✅ **Responsive web UI** — mobile-first, fluid layout, tables become cards on small screens.
- ✅ **Multi-currency** — per-ledger base currency, per-transaction currency, ECB rate refresh.
- ✅ **Single binary**, no JS build step, no Node.js required.

Advanced accounting
- ✅ **Invoices** — multi-line invoices with computed totals, a printable per-invoice detail page with mark-paid / void actions, and a computed overdue flag for unpaid past-due invoices.
- ✅ **Tax on transactions** — define tax rates per ledger (sales/purchase), attach a rate to any posting line, and the tax leg is posted to the rate's account automatically. The tax report shows net / tax / gross per rate.
- ✅ **AR / AP aging** — outstanding receivables and payables bucketed by age.
- ✅ **Amortization schedules** — recurring journal entries with skip / forecast.
- ✅ **Cash-basis toggle** — each ledger is accrual or cash; the income-statement and cash-flow reports accept `?basis=…`; the cash variant only counts postings whose peer leg is a cash / bank account. The default is accrual, which preserves the double-entry A = L + E invariant.
- ✅ **Closing entries** — fiscal-year close transfers net income to retained earnings and locks the period.
- ✅ **Investment lots** — cost-basis tracking for securities with realized-gains reports.
- ✅ **Multi-entity consolidation** — inter-ledger transfers and consolidated reports.
- ✅ **Inventory** — purchases, adjustments, valuation.
- ✅ **Fixed assets** — depreciation schedules and disposal.
- ✅ **Budgets** — budget vs. actual reporting.
- 🧪 **Factur-X e-invoicing** — XML export of invoices for EU compliance; not a certified access point.

Data import / export
- ✅ **CSV export** — transactions and reports.
- ✅ **Plain-text accounting (PTA)** — Beancount export/import plus a hledger-style CSV, both as `openaccounting export` / `openaccounting import` subcommands for scripted round-trips (see below).
- ✅ **CSV import wizard** — auto-detects columns, previews, lets you map, then commits.
- ✅ **Bank statement imports** — WeChat Pay and Alipay CSV formats.
- 🔌 **Bank feeds** — Plaid link + sync; HMAC-SHA256 verified webhook. Plaid availability is provider-dependent.

Onboarding & UX
- ✅ **Onboarding setup checklist** — a data-driven five-step guide (opening balances, first transaction, a document, a bank feed, an invite) shown on the dashboard and at `/ledgers/{id}/setup`, so a new user knows exactly what to do first.

API & integrations
- ✅ **REST API** — personal API tokens (manageable from the account page) for the `/api/v1/*` endpoints.
- 🔌 **OIDC SSO** — single sign-on for self-hosted deployments; configured per environment.
- ✅ **Outgoing webhooks** — subscriptions with HMAC-SHA256 payload signing, secret rotation, and replay.
- 🧪 **OCR feedback** — document OCR with a feedback loop to improve extracted fields.

Operations
- ✅ **Health and metrics endpoints** — `/healthz`, `/readyz`, `/metrics` (Prometheus).
- ✅ **Scheduled backups** — configurable cron schedule with retention.
- ✅ **Audit chain** — append-only hash chain with verification.
- ✅ **Reproducible builds** — pinned toolchain, cosign-signed release artifacts.
- 📦 **S3 document storage** — opt-in `storage-s3` Cargo feature; default is filesystem.
- 📦 **SQLite backend** — opt-in `db-sqlite` Cargo feature; default is PostgreSQL.

## Non-Goals (for v1)

- Native mobile apps (iOS / Android) — install the responsive
  web UI as a PWA instead. The PWA install path is documented
  in [`mobile/README.md`](mobile/README.md) and enforced by
  `scripts/check_mobile_promise.py` so the two READMEs cannot
  diverge. Offline scope is the cached read-only routes only —
  there is no offline write queue.
- Multi-tenant SaaS hosting — single-tenant per deployment.
- Jurisdiction-specific tax rule packs (US 1099, EU VAT MOSS, etc.) — the tax feature records rates and postings; it does not pick rules by jurisdiction.
- Bank reconciliation auto-matching beyond the line-level match helper.
- Formal SOC 2 / ISO 27001 certification.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Browser (HTMX + Tailwind; vendored htmx.min.js +        │
│          hand-written app.css in static/)               │
│   - Server-rendered HTML, partial updates via HTMX      │
└────────────────────────┬────────────────────────────────┘
                         │ HTTP
┌────────────────────────▼────────────────────────────────┐
│ Axum application (single Rust binary)                   │
│   - src/lib.rs: composition root                        │
│       build_router(state, config) -> axum::Router       │
│       run() -> anyhow::Result<()>                        │
│   - src/main.rs: thin shell calling run()               │
│   - tower-sessions (Postgres-backed)                    │
│   - axum-login + Argon2                                 │
│   - askama templates                                    │
│   - Hand-written SVG charts                             │
└────────────────────────┬────────────────────────────────┘
                         │ sqlx
┌────────────────────────▼────────────────────────────────┐
│ PostgreSQL                                              │
│   - ledgers, accounts, transactions, postings,          │
│     documents, tags, audit_chain                        │
│ Document storage                                        │
│   - Filesystem: ./data/documents/{transaction_id}/...   │
│   - or S3: storage-s3 Cargo feature                     │
└─────────────────────────────────────────────────────────┘
```

### Tech stack

Pinned in `Cargo.toml` and `rust-toolchain.toml` (channel `1.95.0`):

- **Backend:** Rust 1.95, Axum 0.8.1, sqlx 0.8.3, axum-login 0.17, Argon2 0.5.3
- **Sessions:** `tower-sessions` 0.14 (Postgres-backed, no Redis)
- **Templates:** Askama 0.13 (pure Rust, type-safe, Jinja-like)
- **Money:** `rust_decimal` 1.36 (exact decimal arithmetic)
- **Frontend:** Server-rendered HTML + HTMX (vendored) + hand-written CSS on top of Tailwind
- **DB:** PostgreSQL (SQLite optional via `db-sqlite` feature)
- **Charts:** Hand-written SVG, server-rendered

---

## Plain-text CLI

Export a ledger to Beancount (or hledger-style CSV) on stdout:

```
openaccounting export --ledger=<ledger-uuid> --format=beancount > books.bean
openaccounting export --ledger=<ledger-uuid> --format=hledger-csv > books.csv
```

Import a file back from stdin. Already-imported transactions are
detected by `(date, description, payee, amount)` and skipped, so
re-imports are a no-op. `--dry-run` prints the planned diff
without writing anything:

```
openaccounting import --ledger=<ledger-uuid> --format=beancount < books.bean
openaccounting import --ledger=<ledger-uuid> --format=hledger-csv --dry-run < books.csv
```

Both subcommands read `DATABASE_URL` from the environment or
`.env` (the same connection settings the server uses).

---

## Quick Start

### Prerequisites

- Rust 1.95+ (pinned via `rust-toolchain.toml`; `rustup` will pick it up automatically)
- PostgreSQL 14+ (or `docker compose up -d postgres`)

### Run

```bash
# 1. Start Postgres
docker compose up -d postgres

# 2. Copy env and edit
cp .env.example .env

# 3. Run (migrations are applied automatically on startup)
cargo run --release

# 4. Open http://localhost:3000
# First visit: register a user, create a ledger, start posting.
```

### Default Chart of Accounts

On first ledger creation, the following accounts are seeded:

- **Assets:** Cash on Hand, Bank Account, Accounts Receivable
- **Liabilities:** Accounts Payable, Credit Card
- **Equity:** Owner's Equity, Opening Balances
- **Income:** Sales Revenue, Other Income
- **Expenses:** Office Supplies, Travel & Meals, Software & SaaS, Marketing, Professional Services, Rent, Utilities, Other

---

## Domain Schema

```
ledgers (id, name, base_currency, owner_id, …)
  └── accounts (id, ledger_id, name, type, currency, …)
       └── postings (id, transaction_id, account_id, amount, direction[DEBIT|CREDIT], …)
            └── transactions (id, ledger_id, date, description, payee, …)
                 └── documents (id, transaction_id, filename, mime, size, path, …)
                 └── tags (id, transaction_id, name)
```

**Invariant enforced in code and in DB (trigger):**
`SUM(postings.amount where direction='DEBIT') = SUM(postings.amount where direction='CREDIT')`
for every transaction.

---

## Project Layout

```
openaccounting/
├── Cargo.toml
├── docker-compose.yml
├── Dockerfile
├── .env.example
├── migrations/
│   └── … (sqlx migrations, applied on startup)
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── error.rs
│   ├── db/
│   ├── auth/             # login, sessions, password hashing
│   ├── domain/           # ledger, account, transaction, posting, document
│   ├── handlers/         # axum route handlers (one per resource)
│   ├── reports/          # trial balance, P&L, balance sheet, cash flow
│   ├── charts/           # server-rendered SVG chart helpers
│   ├── storage/          # filesystem document store
│   └── templates/        # askama template structs
├── templates/            # *.html (jinja-like)
│   ├── base.html
│   ├── partials/
│   └── …
└── static/
    ├── htmx.min.js       # vendored
    └── css/app.css       # small hand-written CSS on top of Tailwind
```

---

## Cash basis

Read the [cash-basis guide](docs/cash-basis.md) to understand the
read-time cash filter, the DeferredRevenue / PrepaidExpense
convention, and one-click recognition.

## SQLite backend

Read the [SQLite guide](docs/sqlite-backend.md) to enable the
optional `db-sqlite` Cargo feature and run with
`DATABASE_URL=sqlite:///path/to/db.sqlite`.

## WASM demo

A read-only in-browser demo ships at
[`static/demo/index.html`](static/demo/index.html). See
[`crates/wasm-demo/README.md`](crates/wasm-demo/README.md) for
build instructions. The full server is unchanged.

## Verifying releases

Release binaries are [reproducible and cosign-signed](docs/release-verification.md).
To verify a downloaded binary:

```sh
sha256sum openaccounting
cosign verify-blob \
  --bundle openaccounting.bundle \
  --certificate-identity-regexp 'https://github.com/lileililiwen/openaccounting/.github/workflows/release.yml@refs/tags/<TAG>' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  openaccounting
```

## Migrations

Every migration in `migrations/` MUST be reversible: each file
ends with a `-- !DOWN` block that undoes the UP section in order.
`scripts/migrate-down.sh` reverts and re-applies every migration
against a fresh database; the CI job `.github/workflows/ci.yml`
runs this script in the `migrations-reversible` step. New
migrations that drop data (e.g. seed scripts) MUST carry a
`-- Reversible: no` header and explain why in the design doc.

`sqlx migrate add --reversible <name>` is the canonical command
for new migrations.

## Accounting Compliance Disclaimer

OpenAccounting is open-source bookkeeping software. It is NOT
a substitute for professional accounting, tax, or legal advice.

**Tax compliance:** Passing automated tests does NOT establish
jurisdiction-specific tax compliance. Tax rules vary by
jurisdiction and change frequently. Consult a qualified tax
professional for your jurisdiction.

**Accounting standards:** The software implements generic
double-entry bookkeeping. Revenue recognition, inventory
valuation, depreciation, and other treatments follow simplified
rules that may NOT conform to your jurisdiction's accounting
standards (e.g., GAAP, IFRS). Consult your auditor.

**Audit trail:** The append-only hash chain provides
tamper-evidence but is NOT a legally binding digital signature.
It does not replace formal audit controls.

**Financial reports:** Reports are informational only. They
should NOT be used as the sole basis for financial decisions
without professional review.

For detailed treatment records for each advanced workflow
(tax, FX, invoices, amortization, inventory, closing,
reversals, audit chain), see
`docs/accounting-treatment/`.

## License

MIT — see [LICENSE](LICENSE).

This project includes code patterns adapted from
**[vito](https://github.com/9-8-7-6/vito)** © 9-8-7-6, also MIT-licensed.
See [NOTICE](NOTICE) for details.
