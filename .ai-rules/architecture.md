# Architecture conventions

OpenAccounting is a single Rust binary with server-rendered HTML. Keep these
boundaries explicit when adding modules or cross-cutting behavior:

- `src/domain/` owns accounting concepts and invariants; it must not depend on
  Axum transport or Askama templates.
- `src/reports/` and `src/charts/` consume domain data and exact Decimal
  calculations; chart conversion to `f64` is limited to SVG rendering.
- `src/handlers/` owns HTTP extraction, authorization, responses, and route
  composition; it may call domain, reports, auth, and storage layers.
- `src/auth/` owns password hashing, sessions, CSRF, rate limits, and OIDC.
- `src/storage/` owns document persistence and backend selection.
- `src/main.rs`/`src/lib.rs` are composition roots for configuration, pools,
  storage, workers, and router wiring.
- `migrations/` are the durable PostgreSQL contract. New migrations need a
  reversible `-- !DOWN` section or an explicit documented exception.

Every write path must preserve balanced postings and ledger authorization.
New external providers require a capability boundary, failure behavior,
secrets handling, tests, and a documented fallback or explicit limitation.
