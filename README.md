# OpenAccounting

**Open-source double-entry bookkeeping for individuals and startups.**

Web-based, responsive, document-aware. Single binary, PostgreSQL backend, HTMX + Tailwind UI.

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

- **Double-entry bookkeeping** — every transaction is balanced; cannot be saved otherwise.
- **Chart of Accounts** — five account types (Asset, Liability, Equity, Income, Expense); default chart seeded.
- **Transactions** — dated, described, with 2+ postings. Multi-leg splits supported.
- **Document upload** — attach receipts, invoices, payment slips (images + PDFs) to any transaction.
- **Reports**
  - General Ledger
  - Trial Balance
  - Balance Sheet (point-in-time)
  - Income Statement (period, **accrual or cash basis**)
  - Cash Flow (period)
- **Cash-basis toggle** — each ledger is created as either
  accrual or cash. The income-statement and cash-flow reports
  accept `?basis=…`; the cash variant only counts postings
  whose peer leg is a cash / bank account (i.e. revenue when
  received, expense when paid). The default is accrual, which
  preserves the double-entry A = L + E invariant.
- **Visualizations** (server-rendered SVG, zero JS chart library)
  - Income vs. expense over time
  - Expense breakdown by category
  - Account balance trends
- **Responsive web UI** — mobile-first, fluid layout, tables become cards on small screens.
- **Multi-currency** — per-ledger base currency, per-transaction currency.
- **CSV export** — transactions and reports.
- **Single binary**, no JS build step, no Node.js required.

## Non-Goals (for v1)

- Multi-user permissions / audit trail beyond per-user ownership
- Bank feeds / OFX import
- Invoicing / AR / AP workflows
- Mobile native apps

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Browser (HTMX + Tailwind via Play CDN in dev)           │
│   - Server-rendered HTML, partial updates via HTMX      │
└────────────────────────┬────────────────────────────────┘
                         │ HTTP
┌────────────────────────▼────────────────────────────────┐
│ Axum application (single Rust binary)                   │
│   - tower-sessions (Postgres-backed)                    │
│   - axum-login + Argon2                                 │
│   - askama templates                                    │
│   - Hand-written SVG charts                             │
└────────────────────────┬────────────────────────────────┘
                         │ sqlx
┌────────────────────────▼────────────────────────────────┐
│ PostgreSQL                                              │
│   - ledgers, accounts, transactions, postings,          │
│     documents, tags                                     │
│ Local filesystem                                        │
│   - ./data/documents/{transaction_id}/{filename}        │
└─────────────────────────────────────────────────────────┘
```

### Tech stack

- **Backend:** Rust, Axum 0.8, sqlx, axum-login, Argon2
- **Templates:** Askama (pure Rust, type-safe, Jinja-like)
- **Frontend:** Server-rendered HTML + HTMX (partials) + Tailwind Play CDN
- **DB:** PostgreSQL
- **Sessions:** `tower-sessions` backed by sqlx (no Redis)
- **Charts:** Hand-written SVG, server-rendered

---

## Quick Start

### Prerequisites

- Rust 1.78+
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

## License

MIT — see [LICENSE](LICENSE).

This project includes code patterns adapted from
**[vito](https://github.com/9-8-7-6/vito)** © 9-8-7-6, also MIT-licensed.
See [NOTICE](NOTICE) for details.
