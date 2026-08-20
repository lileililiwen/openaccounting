# Tax on Transactions — Tasks

## 1. Testing

- [ ] 1.1 Add a pure unit test that `round(amount × rate, 2)` produces the expected tax amount (including .5 rounding).
- [ ] 1.2 Add an integration test: POST a transaction with one expense line carrying an active tax rate and a balancing bank line; assert the resulting postings include a tax leg on the same side with the right amount, and a `posting_taxes` row exists with base + tax.
- [ ] 1.3 Add an integration test: the Σ debits = Σ credits invariant still holds when tax legs are present (server rejects an unbalanced entry).
- [ ] 1.4 Add an integration test: a draft saved with a taxed line, when promoted, still carries the tax leg and the `posting_taxes` row.
- [ ] 1.5 Add an integration test: reversing a taxed transaction negates the tax leg along with the other legs.
- [ ] 1.6 Add an integration test: an unknown / inactive / wrong-ledger tax rate is rejected with a validation error.
- [ ] 1.7 Add an integration test: the tax report aggregates base (net), tax, and gross per rate for a date range.
- [ ] 1.8 Add an integration test: a transaction with no tax selected records no `posting_taxes` rows and no extra leg.

## 2. Implementation

- [ ] 2.1 Migration `0048_add_posting_tax_base`: add `base_amount` column to `posting_taxes` (+ `.down.sql`).
- [ ] 2.2 Extend `TxnLineInput` with `tax_rate_id: Option<Uuid>`; update the four non-transaction construction sites (`transfers.rs`, `transactions_edit.rs`, `workers/amortization.rs`) to pass `None`.
- [ ] 2.3 Add `TaxLink { posting_idx, tax_rate_id, base_amount, tax_amount }` to `PostingService` and write `posting_taxes` rows during `create_with_kind` (capture posting ids via `RETURNING id`).
- [ ] 2.4 Parse `lines[N][tax_rate_id]` in the create handler, load the ledger's active rates, append tax legs, compute `tax_links`, and thread them into `NewTransaction`.
- [ ] 2.5 Run the "same account on both sides" guard over the user's lines before appending tax legs.
- [ ] 2.6 Add `tax_rate_id` to `ParsedLine` / `TransactionFormLine` and preserve it in the error re-render (`make_error`).
- [ ] 2.7 Pass active tax rates to `TransactionNew` and render a tax `<select>` on each Advanced line in `new.html`.
- [ ] 2.8 Update `entry.js`: include per-line tax in the balance computation, disable the tax select on the balancing row, and show net + tax in the status readout.
- [ ] 2.9 Update the tax report query + template to show base/tax/gross per rate (treat NULL base as 0).

## 3. Documentation

- [ ] 3.1 Update `docs/` or README feature list to mention per-transaction tax lines.
