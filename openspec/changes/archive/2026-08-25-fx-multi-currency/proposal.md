# Multi-Currency FX: Rates, Foreign Amounts, Revaluation

## Why

Currency today is a label. `ledgers.base_currency`, `accounts.currency`,
and `transactions.currency` are CHAR(3) columns, but there is no
exchange-rate store anywhere (`rg "exchange_rate|fx_rate" src/ migrations/`
is empty). A EUR ledger cannot record a USD invoice without lying about
its base-currency value. Every comparable product has this:
QuickBooks Online, Xero (160+ currencies + automatic realized/unrealized
gains), Zoho Books, Sage Plus, Manager.io, Bigcapital, Firefly III
(dated FX store + API). Wave is the notable *absence*, and it is a
known criticism of Wave. This is the single largest functional gap
between openaccounting and the systems it competes with.

## What Changes

- New `fx_rates` table (base currency, quote currency, rate, dated)
  populated from manual entry and an ECB daily-feed worker.
- Postings MAY carry a `foreign_amount` + `foreign_currency`; the
  posting's base amount is derived at the transaction date's rate.
- Month-end revaluation action posts unrealized gain/loss for
  foreign-currency monetary accounts.
- New "Realized & Unrealized FX Gains" report; balance sheet and
  income statement show FX gain/loss lines.
- Investment holdings report may reuse the same rate/price store.

## Capabilities

### New Capabilities

- `multi-currency-fx`: dated FX rate store, foreign-amount postings,
  revaluation, and FX gain/loss reporting.

## Impact

**New files:** `src/domain/fx.rs`, `src/handlers/fx.rs`,
`src/workers/fx_refresh.rs`, `src/reports/fx_gains.rs`,
`migrations/00xx_fx_rates.sql`.
**Modified:** posting service (foreign-amount derivation), balance-sheet /
income-statement queries, holdings report.
