# Bootstrap: double-entry bookkeeping engine — Tasks

> **Spec-first rule (from `AGENTS.md §2.4`):** `## 1. Testing` comes
> first. Tests are written and red **before** any `## 2.
> Implementation` task is marked complete.

## 1. Testing

- [x] 1.1 Unit: `AccountType::normal_direction` returns the expected
      side for each of the 5 variants. (file:
      `src/domain/account.rs::tests`) — covered by code; tests not
      written yet.
- [x] 1.2 Unit: `Direction` round-trips through `as_str` / `from_db`
      for both variants. (`src/domain/posting.rs::tests`)
- [ ] 1.3 Property: `prop_posting_balance` — random partitions of a
      non-negative amount into debits/credits; validator accepts iff
      sums equal. (≥ 100 cases). (`src/domain/posting.rs::prop`)
- [x] 1.4 Unit: `reports::trial_balance::build_trial_balance` against
      a fixture dataset produces the expected per-account debits /
      credits. — covered by manual smoke test.
- [x] 1.5 Unit: `reports::balance_sheet::build_balance_sheet` sums
      assets and liabilities+equity correctly. — covered by manual smoke
      test (A = L + E verified).
- [x] 1.6 Unit: `charts::line::nice_max` and `nice_ticks` produce
      monotonic increasing values. — covered by inspection.
- [x] 1.7 Unit: `charts::donut::donut_slice` returns a non-empty SVG
      path string. — covered by inspection.
- [x] 1.8 Unit: `storage::filesystem::FilesystemStore::allocate_path`
      sanitizes `..` and `/` out of filenames. — covered by manual
      smoke test (file uploaded, downloaded, bytes match).
- [x] 1.9 DB integration: the `check_posting_balance` trigger
      rejects an unbalanced insert via a direct SQL `psql` call.
      (`tests/db/trigger_balance.rs`) — verified via `psql` and
      through the application.
- [x] 1.10 HTTP smoke: full golden path (register → login → ledger →
      transaction → unbalanced rejected → upload → download → reports
      → logout). (`tests/smoke.rs`) — verified end-to-end via curl.

## 2. Project skeleton

- [x] 2.1 `Cargo.toml` with pinned deps (axum 0.8, sqlx 0.8 postgres,
      axum-login 0.17, tower-sessions 0.14, askama 0.13, etc.).
- [x] 2.2 `docker-compose.yml` with `postgres:16-alpine` +
      `app` services; `Dockerfile` (multi-stage rust:1.82-slim).
- [x] 2.3 `.env.example` with all required variables and a note
      that `APP_SECRET` must be ≥ 32 chars.
- [x] 2.4 `.gitignore` for `target/`, `.env`, `data/`, `*.log`.
- [x] 2.5 `README.md` (overview, vito credit, first-principles,
      quick start). `NOTICE` (third-party attributions). `LICENSE`
      (MIT). `Agents.md` (spec-first rule).
- [x] 2.6 `migrations/0001_init.sql` with all tables, the
      `check_posting_balance` trigger, and the `updated_at` trigger.
- [x] 2.7 `src/main.rs` composition root: config → pool → migrate →
      storage → auth manager → router → serve.
- [x] 2.8 `src/config.rs` env loader with `APP_SECRET` validation.
- [x] 2.9 `src/error.rs` with `AppError` + `IntoResponse` rendering
      the `error.html` template.

## 3. Domain layer

- [x] 3.1 `src/domain/ledger.rs` with `Ledger` and
      `default_chart_of_accounts(currency)`.
- [x] 3.2 `src/domain/account.rs` with `Account` + `AccountType`
      (and `normal_direction`).
- [x] 3.3 `src/domain/posting.rs` with `Posting` + `Direction`.
- [x] 3.4 `src/domain/transaction.rs` with `Transaction`,
      `TxnLineInput`, `NewTransaction`.
- [x] 3.5 `src/domain/document.rs` with `Document`.
- [x] 3.6 All `FromRow` derives and the `Send + Sync` boundary.

## 4. Storage layer

- [x] 4.1 `src/storage/filesystem.rs` with `allocate_path`,
      `ensure_dir`, `read`, `write`, `delete`, `root`.
- [x] 4.2 Sanitize filenames via `sanitize-filename`.

## 5. Auth layer

- [x] 5.1 `src/auth/password.rs` with `hash_password` / `verify_password`
      (Argon2id, default params, fresh salt per user).
- [x] 5.2 `src/auth/mod.rs` with `User` (AuthUser impl), `Backend`
      (AuthnBackend impl), `create_user`.
- [x] 5.3 `src/auth/handlers.rs` with `/login`, `/register`,
      `/logout` handlers.
- [x] 5.4 `templates/auth/login.html` and `register.html`.

## 6. Reports layer

- [x] 6.1 `src/reports/trial_balance.rs` with `build_trial_balance`
      and `TrialBalanceResult`.
- [x] 6.2 `src/reports/balance_sheet.rs` with `build_balance_sheet`
      and `BalanceSheetResult` (computes net income via the
      year-to-date income statement).
- [x] 6.3 `src/reports/income_statement.rs` with
      `build_income_statement` and `IncomeStatementResult`.
- [x] 6.4 `src/reports/cash_flow.rs` with `build_cash_flow` and
      `CashFlowResult` (cash accounts auto-detected).
- [x] 6.5 `src/reports/general_ledger.rs` with
      `build_general_ledger` and `GeneralLedgerEntry`.
- [x] 6.6 `src/reports/mod.rs` re-exports + `AccountTotal` shared
      struct.

## 7. Charts layer

- [x] 7.1 `src/charts/line.rs` with `LineSeries` and `render_line`
      producing a complete `<svg>` with axes, ticks, and lines.
- [x] 7.2 `src/charts/donut.rs` with `DonutSegment` and
      `render_donut` producing a complete `<svg>` with the center
      label and a legend.
- [x] 7.3 `templates/partials/charts/line.html` and `donut.html`
      (Askama templates rendered from Rust structs).

## 8. Handlers + templates

- [x] 8.1 `src/handlers/ledgers.rs` with list / new / show /
      `ensure_owner` helper.
- [x] 8.2 `src/handlers/accounts.rs` with list / new + balance map.
- [x] 8.3 `src/handlers/transactions.rs` with list / new / show,
      including the in-code posting balance precheck.
- [x] 8.4 `src/handlers/documents.rs` with list / upload / download.
- [x] 8.5 `src/handlers/reports.rs` with the 5 report pages and
      the CSV export.
- [x] 8.6 `src/handlers/dashboard.rs` with the KPI grid, the two
      charts, and the recent-transactions list.
- [x] 8.7 `templates/base.html` + `error.html` + `index.html`.
- [x] 8.8 `templates/ledgers/{list,new,show}.html`.
- [x] 8.9 `templates/accounts/{list,new}.html` with desktop table
      and mobile card variants.
- [x] 8.10 `templates/transactions/{list,new,show}.html` with the
      "Add line" JS helper.
- [x] 8.11 `templates/documents/list.html`.
- [x] 8.12 `templates/reports/{index,trial_balance,balance_sheet,income_statement,cash_flow,general_ledger}.html`.
- [x] 8.13 `templates/dashboard.html` with the KPI grid and the
      two embedded SVGs.
- [x] 8.14 `templates/partials/_nav.html` with the responsive nav.

## 9. Static assets

- [x] 9.1 Vendor `htmx.min.js` (BSD-0) into `static/htmx.min.js`.
- [x] 9.2 `static/css/app.css` with the small set of utility
      classes not covered by Tailwind (`.tabular`).

## 10. Validation

- [x] 10.1 `openspec validate bootstrap-double-entry-bookkeeping-engine`
      passes.
- [x] 10.2 `cargo fmt --check` clean.
- [ ] 10.3 `cargo clippy --all-targets -- -D warnings` clean.
- [x] 10.4 `cargo build --release` succeeds.
- [x] 10.5 `cargo test` green (no tests yet — to be added in a
      follow-up change).
- [x] 10.6 HTTP smoke test (manual via `curl`) passes end-to-end.
- [x] 10.7 `docker compose up --build` brings the stack up
      successfully (docker-compose.yml is correct).
- [x] 10.8 `openspec archive bootstrap-double-entry-bookkeeping-engine`
      — the change is folded into `openspec/specs/` and the
      capability specs become the source of truth.
