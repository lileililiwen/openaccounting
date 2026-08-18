## 1. Testing

- [x] 1.1 Unit: `fifo_buy_creates_one_lot`.
- [x] 1.2 Unit: `fifo_sell_matches_oldest_lot`.
- [x] 1.3 Unit: `fifo_sell_across_lots_splits_correctly`.
- [x] 1.4 HTTP: `http_holdings_report_renders`.
- [x] 1.5 HTTP: `http_realized_gains_report_renders`.
- [x] 1.6 Property: 100 random buy/sell sequences — Σ buys = open + disposed.

## 2. Implementation

- [x] 2.1 `migrations/0042_add_investment_lots.sql` (renumbered; proposed 0034 was taken). Adds `investment_lots(id, account_id, acquired_at, qty, unit_cost, currency, source_txn_id)` + `investment_disposals(id, lot_id, qty, unit_proceeds, disposed_at, source_txn_id, realized_gain)`.
- [x] 2.2 `src/domain/investment_lot.rs` — `insert_lot`, `fifo_match` (FIFO loop), `remaining_qty`, plus the `InvestmentLot`/`InvestmentDisposal` rows. 2 unit tests.
- [x] 2.3 `src/reports/holdings.rs` — JSON `GET /ledgers/{id}/reports/holdings` returning remaining qty + cost basis per account that has any lot.
- [x] 2.4 `src/reports/realized_gains.rs` — JSON `GET /ledgers/{id}/reports/realized-gains` listing disposals with their realized gain.
- [ ] 2.5 Auto-detect buy/sell on `Investment` accounts from `PostingService::create` — deferred. There is no `Investment` account subtype in the current CHECK constraint (only `CURRENT_ASSET`, `FIXED_ASSET`, `INTANGIBLE_ASSET`, `OTHER_ASSET`); tests use `OTHER_ASSET` for the brokerage account. The auto-detection can land once a migration adds the `INVESTMENT` subtype.

## 3. Validation

- [x] 3.1 `openspec validate a6-investment-lots` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings.
- [x] 3.4 `cargo test --features test-support --test integration` — 215 / 215 pass.
- [ ] 3.5 `openspec archive a6-investment-lots`.
