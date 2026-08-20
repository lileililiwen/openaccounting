---
name: tax-on-transactions
description: Attach tax rates to transaction postings so tax is posted to the tax account and the tax report shows real figures.
type: change
---

# Tax on Transactions

## Why

Tax rates exist (a `tax_rates` table, a rate CRUD page, and a tax report)
but nothing ever writes to `posting_taxes` — the join table between
postings and tax rates is only ever read by the report query. As a
result the tax report is always empty and there is no way to record
sales/purchase tax against a transaction. For a startup or individual
running daily books, VAT / sales-tax tracking is a core requirement:
every purchase with VAT and every invoice with output tax needs the tax
amount posted to the tax liability account so the tax report is
meaningful.

## What Changes

- The Advanced (multi-leg) transaction editor gains an optional **tax
  rate** picker on each posting line, fed from the ledger's active tax
  rates.
- When a line has a tax rate, saving the transaction additionally posts
  a **tax leg** to the tax rate's account on the same debit/credit side
  as the taxed line, and records the linkage in `posting_taxes` (base
  amount + tax amount).
- The balancing-line editor accounts for tax: the auto-balancing line
  absorbs the gross (net + tax) so the entry still satisfies
  Σ debits = Σ credits.
- The tax report and CSV export surface the newly recorded data.

## Capabilities

- `transaction-tax-lines`: attach a tax rate to a posting line and have
  the tax leg posted automatically.

## Non-Goals

- Tax on the two-leg "Simple" entry mode (future work).
- Automatic tax extraction from bank/CSV importers (future work).
- Tax-inclusive entry (entered amount already includes tax) (future work).
