# Bootstrap: double-entry bookkeeping engine — Design

## Context

We are building greenfield. The host system already has a related
project (`/home/paul/code/openpanel`) that uses OpenSpec + DDD
layering. We copy the OpenSpec workflow but adopt a single-crate,
flat-module layout because openaccounting is one bounded context (a
bookkeeping engine) and does not need a Cargo workspace of separate
crates at v0.1. Layer rules are enforced by review, not by separate
crates.

The starting point is the existing vito repo on GitHub
(`https://github.com/9-8-7-6/vito`, MIT). We reuse vito's *skeleton*
(Axum 0.8, axum-login, sqlx migrations, docker-compose, tower
middleware) and rewrite the *domain* (ledgers / accounts / transactions
/ postings) for proper double-entry. The original vito schema is
asset-tracking (stocks, crypto, real estate, single-entry between
assets), which is not what bookkeeping needs.

The single most important design decision:

> The `Σ debits = Σ credits` invariant is enforced in **three**
> places: in the application code (handler-level precheck), in the
> database (a Postgres trigger on `postings`), and in the report
> computations (which only ever read, never write). Three places —
> because the DB trigger is the safety net, the code precheck gives a
> friendly error message, and the reports prove the invariant is
> actually maintained.

## Goals / Non-Goals

**Goals**

- Single Rust binary that runs the web app, serves a responsive
  server-rendered UI, and is the only thing the operator ships.
- True double-entry bookkeeping that produces a balance sheet
  (A = L + E) by construction, not by hand.
- Document upload (receipts / invoices / payment slips) attached to
  transactions; server-side MIME + size allow-list.
- Five classical reports: trial balance, balance sheet, income
  statement, cash flow, general ledger.
- Visualizations as server-rendered SVG (zero JS chart library).
- Fluid responsive layout that works on phone and desktop.
- Reproducible: `cargo build` is the entire build, plus a Postgres
  connection string.

**Non-Goals (v0.1)**

- Multi-tenant permissioning beyond per-user ownership.
- Bank feeds (OFX, Plaid, etc.).
- Invoicing / AR / AP workflows.
- Native mobile apps.
- Multi-currency conversion at the report level (each ledger has a
  single base currency; cross-currency transactions are out of scope).
- Audit log beyond the implicit audit provided by created_at /
  updated_at timestamps.

## Decisions

### D1. Single-crate layout (no Cargo workspace)

- **Why:** v0.1 is one bounded context. A workspace adds friction
  (separate `Cargo.toml` files, longer build, more boilerplate) without
  buying us anything yet.
- **Alternative considered:** workspace with `oa-core`, `oa-domain`,
  `oa-app`, `oa-web` like openpanel. Rejected: overkill for one
  product. We can split later if we add more contexts.
- **Mitigation:** the directory layout (`src/domain/`, `src/handlers/`,
  `src/reports/`, `src/charts/`, `src/storage/`, `src/auth/`,
  `src/templates/`) makes the layering obvious to humans. AGENTS.md
  codifies the dependency rules.

### D2. PostgreSQL, not SQLite

- **Why:** the user asked for Postgres after seeing the vito
  reference. Postgres gives us real triggers (`CREATE TRIGGER`) for the
  balance invariant, partial indexes, and row-level constraints. It
  also gives us a story for multi-user / multi-ledger scaling later.
- **Alternative considered:** SQLite + a Rust-side balance validator.
  Rejected because the DB trigger is the strongest possible safety net
  — the user cannot bypass it by hitting the DB directly.

### D3. Askama for templates, not Tera / Handlebars / MiniJinja

- **Why:** Askama is pure-Rust, type-checked at compile time (you
  can't pass a wrong field name), Jinja-like, and integrates with Axum
  via `askama_axum`. The compile-time check is what makes server-side
  HTML safe to refactor.
- **Alternative considered:** Tera (Mature, popular, but a separate
  library with its own types). MiniJinja (newer). Askama wins on the
  compile-time guarantee.

### D4. Server-rendered SVG, no JS chart library

- **Why:** the spec says "web version, responsive, no JS build
  pipeline". A chart library (Chart.js, ECharts, D3) is a 200–500 KB
  JS payload. Two SVG chart helpers (line + donut) cover the
  dashboard's needs in ~150 lines of Rust and zero bytes of JS.
- **Alternative considered:** lazy-load Chart.js only on dashboard.
  Rejected because we still need to bundle and ship a JS file. Server
  SVG just works.

### D5. HTMX + Tailwind via Play CDN, no build step

- **Why:** zero build, zero `node_modules`, zero `package.json`.
  HTMX minified is vendored at `static/htmx.min.js` so the page works
  offline. Tailwind via Play CDN gives full utility coverage in
  development. For production we document switching to the standalone
  Tailwind CLI to remove the CDN dependency.
- **Alternative considered:** SPA (React / Vue / Svelte). Rejected
  because the user said "web version, fluid layout" — server-rendered
  HTML is the simplest expression of that and produces a smaller,
  faster, more accessible page.

### D6. Local filesystem for documents, swappable interface

- **Why:** v0.1 is single-host. Filesystem storage is simple, fast,
  and the right tool. We define a `Storage` trait-like surface
  (`FilesystemStore`) so swapping to S3 later is a contained refactor.
- **Alternative considered:** Postgres `bytea`. Rejected: bloats the
  DB, no thumbnail generation, no streaming. S3. Rejected: needs
  another service to run.

### D7. Default chart of accounts seeded on ledger creation

- **Why:** a new user should be able to record their first
  transaction in under a minute. A sensible default (Cash, Bank, AR,
  AP, Credit Card, Owner's Equity, Opening Balances, Sales Revenue,
  Other Income, Office Supplies, Travel & Meals, Software & SaaS,
  Marketing, Professional Services, Rent, Utilities, Other Expense)
  gets them to "first transaction" in three clicks.
- **Alternative considered:** empty chart, force the user to
  configure. Rejected as hostile to non-accountants.

## Risks / Trade-offs

- **R1: SQLx compile-time query checking is not active by default.**
  → Mitigation: queries are written as strings with `$1, $2, …`
  placeholders. We add a `make sqlx-prepare` target in a follow-up
  change that uses `cargo sqlx prepare` to generate a
  `.sqlx/` cache, and `SQLX_OFFLINE=true` in CI to fail the build on
  drift.
- **R2: The `askama_axum` `IntoResponse` impl may not yet be stable
  for nested templates.** → Mitigation: each template explicitly
  implements `IntoResponse` via `askama_axum::IntoResponse` on the
  page struct. Handlers return `Result<impl IntoResponse, AppError>`.
- **R3: Tailwind Play CDN warns in production.** → Mitigation: the
  README documents the swap to the standalone CLI build for
  production. Not in v0.1 scope.
- **R4: Money math via `rust_decimal`.** No risk in practice, but
  `f64` conversions are needed for SVG charts. → Mitigation: convert
  to `f64` only at the very last moment, in the chart renderer, and
  never store the converted value.
- **R5: Argon2 with the `default` parameters is intentionally
  slow.** → Mitigation: 50–100 ms hash time is acceptable for a
  personal / startup deployment with < 100 users.

## Migration Plan

Greenfield. There is no existing data.

For an existing user with bookkeeping data in another tool, v0.1
ships CSV export and a future change will add CSV / OFX import. The
schema is straightforward enough that a one-time `INSERT`s script is
trivial to write.

## Open Questions

- **Q1:** Should we add a workspace switch (multiple ledgers visible
  in the nav) for users with many ledgers? Defer to v0.2.
- **Q2:** Should the trial balance be the *unadjusted* (pre-closing)
  or the *adjusted* (post-closing) trial balance? v0.1 ships
  unadjusted (no closing entries). Closing entries can be modelled
  later as a transaction from each income/expense account to Owner's
  Equity / Retained Earnings.

## File-by-File Plan

```
openaccounting/
├── Cargo.toml                        # single crate
├── docker-compose.yml                # postgres + app
├── Dockerfile                        # multi-stage rust:1.82-slim
├── .env.example                      # documented env vars
├── .gitignore
├── LICENSE                           # MIT
├── NOTICE                            # third-party attributions
├── README.md                         # overview, vito credit, first-principles
├── Agents.md                         # spec-first contract for AI agents
├── migrations/
│   └── 0001_init.sql                 # schema + balance trigger
├── src/
│   ├── main.rs                       # composition root
│   ├── config.rs                     # env loading
│   ├── error.rs                      # AppError + IntoResponse
│   ├── auth/
│   │   ├── mod.rs                    # User, Backend, create_user
│   │   ├── password.rs               # Argon2id hash + verify
│   │   └── handlers.rs               # /login /register /logout
│   ├── db/
│   │   ├── mod.rs
│   │   └── pool.rs                   # PgPool + migrate
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── ledger.rs                 # Ledger + default chart
│   │   ├── account.rs                # Account, AccountType
│   │   ├── transaction.rs            # Transaction, NewTransaction, TxnLineInput
│   │   ├── posting.rs                # Posting, Direction
│   │   └── document.rs               # Document
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── ledgers.rs                # list / new / show
│   │   ├── accounts.rs               # list / new + balance map
│   │   ├── transactions.rs           # list / new / show
│   │   ├── documents.rs              # upload / list / download
│   │   ├── reports.rs                # 5 reports + CSV export
│   │   └── dashboard.rs              # KPIs + 2 charts + recent
│   ├── reports/
│   │   ├── mod.rs
│   │   ├── trial_balance.rs          # Σ debits == Σ credits
│   │   ├── balance_sheet.rs          # A = L + E
│   │   ├── income_statement.rs       # income − expense
│   │   ├── cash_flow.rs              # cash-account flows
│   │   └── general_ledger.rs         # postings with running balance
│   ├── charts/
│   │   ├── mod.rs
│   │   ├── line.rs                   # SVG line chart
│   │   └── donut.rs                  # SVG donut chart
│   ├── storage/
│   │   ├── mod.rs
│   │   └── filesystem.rs             # FilesystemStore
│   └── templates/
│       ├── mod.rs                    # template structs
│       ├── auth.rs                   # LoginPage, RegisterPage
│       ├── ledgers.rs                # LedgerList / New / Show
│       ├── accounts.rs               # AccountList / New
│       ├── transactions.rs           # TransactionList / New / Show
│       ├── documents.rs              # DocumentList
│       ├── reports.rs                # 5 report pages
│       ├── dashboard.rs              # DashboardPage
│       ├── charts.rs                 # Chart partials
│       └── common.rs
├── templates/                        # Askama .html files
│   ├── base.html
│   ├── error.html
│   ├── index.html
│   ├── dashboard.html
│   ├── auth/
│   │   ├── login.html
│   │   └── register.html
│   ├── ledgers/
│   │   ├── list.html
│   │   ├── new.html
│   │   └── show.html
│   ├── accounts/
│   │   ├── list.html
│   │   └── new.html
│   ├── transactions/
│   │   ├── list.html
│   │   ├── new.html
│   │   └── show.html
│   ├── documents/
│   │   └── list.html
│   ├── reports/
│   │   ├── index.html
│   │   ├── trial_balance.html
│   │   ├── balance_sheet.html
│   │   ├── income_statement.html
│   │   ├── cash_flow.html
│   │   └── general_ledger.html
│   └── partials/
│       ├── _nav.html
│       └── charts/
│           ├── line.html
│           └── donut.html
└── static/
    ├── htmx.min.js                   # vendored (BSD-0)
    └── css/                          # placeholder; tailwind via CDN
```

## Testing Strategy

Per AGENTS.md §2.4 and §4, every task under `## 2. Implementation`
is preceded by a matching task under `## 1. Testing`.

| Type | What it tests | Where |
|---|---|---|
| Unit | `Domain::AccountType::normal_direction` for each variant; `Direction` round-trips; chart math (nice_max, donut slice path). | `src/<area>.rs::tests` |
| Property | `prop_posting_balance`: for any random `(debits, credits)` partition, the validator accepts iff `Σ == Σ`. | `src/domain/posting.rs::prop` |
| Integration | HTTP routes against a `TestDb`: create user → login → create ledger → seed → create transaction (balanced and unbalanced) → upload document → run report → CSV export. | `tests/http/` |
| DB | The trigger rejects unbalanced inserts; the trigger accepts balanced inserts. | `tests/db/trigger_balance.rs` |
| HTTP smoke | `curl` against a running local server; check that 200s and 4xxs come back as expected. | `tests/smoke.sh` |
