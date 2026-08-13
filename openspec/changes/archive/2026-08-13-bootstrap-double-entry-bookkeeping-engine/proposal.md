# Bootstrap: double-entry bookkeeping engine

## Why

There is no open-source, single-binary, server-rendered bookkeeping
system in Rust that supports proper double-entry accounting, document
attachments, and reports with visualizations in one package. Individuals
and small startups either run spreadsheets (no double-entry) or pay for
SaaS (closed source, monthly fees). This change ships the v0.1
foundation of **openaccounting** — built upon the
[vito](https://github.com/9-8-7-6/vito) Axum skeleton (MIT, credited
in `NOTICE`) with a rewritten domain, reports, document storage, and a
responsive HTMX + Tailwind UI.

## What Changes

- New Cargo project `openaccounting` (single binary, edition 2021).
- PostgreSQL schema (`migrations/0001_init.sql`): `users`, `ledgers`,
  `accounts`, `transactions`, `postings`, `documents`, `tags`,
  `transaction_tags` with a Postgres trigger enforcing the
  `Σ debits = Σ credits` invariant per transaction.
- Argon2id password hashing + `axum-login` + Postgres-backed
  `tower-sessions`. No Redis. Sessions last 30 days.
- Domain layer (`src/domain/`): `Ledger`, `Account`, `Transaction`,
  `Posting`, `Document`, `AccountType`, `Direction` — pure data with
  the double-entry invariant validated in code and in the DB.
- Reports module (`src/reports/`): trial balance, balance sheet, P&L,
  cash flow, general ledger. Date filters everywhere.
- Charts module (`src/charts/`): hand-written SVG (line + donut),
  server-rendered, zero JS chart library.
- Handlers (`src/handlers/`): ledgers, accounts, transactions,
  documents, reports, dashboard. All server-rendered HTML.
- Templates (`templates/`): Askama (pure Rust, type-safe).
  Server-rendered, fluid layout via Tailwind Play CDN, HTMX for partial
  updates. Tables become cards on small screens.
- Local-filesystem document store (`src/storage/`), one subdir per
  transaction; files sanitized; max 25 MB; allow-list of MIME types.
- CSV export for general ledger and trial balance.
- Docker Compose with Postgres + the app; Dockerfile for production.
- `README.md` + `NOTICE` + `AGENTS.md` (spec-first rule) +
  `LICENSE` (MIT).

**BREAKING**: none (this is a greenfield project).

## Capabilities

### New Capabilities

- `architecture` — layered architecture contract (domain / app / handlers)
  with dependency direction rules and tech-stack pinning.
- `bookkeeping` — the double-entry model: ledgers, chart of accounts,
  balanced transactions / postings, with the `Σ debits = Σ credits`
  invariant.
- `auth` — user model, Argon2id, sessions, register / login / logout,
  per-resource ownership checks.
- `documents` — upload, store, list, download of receipts / invoices /
  payment slips, with size and MIME allow-list.
- `reports` — trial balance, balance sheet, P&L, cash flow, general
  ledger; date filters; CSV export.
- `web-ui` — responsive server-rendered UI (HTMX + Tailwind), fluid
  layout, table-to-card adaptation on small screens, accessibility
  basics.

### Modified Capabilities

- None (greenfield).

## Impact

- **New files:** `Cargo.toml`, `migrations/0001_init.sql`, `src/**`,
  `templates/**`, `static/**`, `Dockerfile`, `docker-compose.yml`,
  `README.md`, `NOTICE`, `AGENTS.md`, `LICENSE`, `.env.example`.
- **Third-party reuse (credited):**
  - [vito](https://github.com/9-8-7-6/vito) (MIT) — Axum skeleton,
    axum-login, sqlx migrations layout.
  - [HTMX](https://htmx.org) (BSD-0) — vendored at `static/htmx.min.js`.
  - [Tailwind CSS](https://tailwindcss.com) (MIT) — Play CDN in dev.
- **Build / runtime:** `cargo build --release` produces a single
  static binary; `docker compose up` brings up Postgres + app.
- **Spec location:** `openspec/changes/2026-08-13-bootstrap-double-entry-bookkeeping-engine/`
  becomes the long-term source of truth for these capabilities after
  `openspec archive`.
