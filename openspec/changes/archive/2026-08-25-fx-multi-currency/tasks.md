# 1. Testing

- [x] 1.1 Unit: `fx::lookup(pair, date)` returns latest rate ≤ date; 90-day miss errors with pair + date in message.
- [x] 1.2 Unit: ECB CSV parser maps a fixture row to `(EUR, USD, 1.0891, 2026-08-21)` and is idempotent on re-parse.
- [x] 1.3 Unit: posting derivation rounds half-up (1000 EUR × 0.91 → 910.00; 0.915 → 915.00).
- [x] 1.4 Property: for 500 random foreign postings the DB balance trigger holds on base amounts.
- [x] 1.5 HTTP: `POST /ledgers/{id}/fx-rates` stores manual rate; manual beats feed row for same day.
- [x] 1.6 HTTP: revaluation posts balanced txn once; second call for same month returns 409.
- [x] 1.7 HTTP: FX gains report separates realized/unrealized; viewer role gets 403.
- [x] 1.8 Integration: worker backfills a 3-day gap without duplicates.

# 2. Implementation

- [x] 2.1 Migration `fx_rates` + nullable `foreign_amount`, `foreign_currency` on `postings`.
- [x] 2.2 `src/domain/fx.rs`: lookup, inversion, rounding.
- [x] 2.3 Posting service: accept optional foreign amount/currency, derive base leg.
- [x] 2.4 `src/handlers/fx.rs`: rates CRUD (owner-only), revaluation route.
- [x] 2.5 `src/workers/fx_refresh.rs`: ECB daily fetch, idempotent upsert.
- [x] 2.6 `src/reports/fx_gains.rs` + reports index entry + CSV export.

# 3. Validation

- [x] 3.1 `openspec validate fx-multi-currency`.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets --features test-support`.
- [x] 3.3 `cargo test --features test-support`.
