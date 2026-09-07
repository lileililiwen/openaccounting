# multi-currency-fx Specification

## Purpose
TBD - created by archiving change fx-multi-currency. Update Purpose after archive.
## Requirements
### Requirement: FX Rate Store

The system SHALL persist dated exchange rates in an `fx_rates`
table with columns `(base_currency, quote_currency, rate, rate_date,
source)` where `rate` is the amount of `quote_currency` per one unit
of `base_currency`, stored as DECIMAL(24,12). Rates SHALL be unique
per `(base_currency, quote_currency, rate_date)`. The lookup for a
transaction on date D SHALL return the most recent rate with
`rate_date <= D`; if none exists within 90 days, posting in that
currency pair MUST be rejected with a user-facing error naming the
missing pair and date.

#### Scenario: Rate lookup picks latest on-or-before

- **WHEN** rates exist for USD→EUR on 2026-08-20 (0.91) and
  2026-08-24 (0.92), and a transaction is dated 2026-08-23
- **THEN** the applied rate is 0.91 and its `rate_date` is recorded
  on the derived amounts.

#### Scenario: Missing pair blocks posting

- **WHEN** a user records a JPY posting in a USD ledger and no
  JPY→USD rate exists within the lookback window
- **THEN** the save fails with an error naming "JPY → USD" and the
  transaction date, and no postings are written.

### Requirement: Rate Sources

Rates SHALL be maintainable from two sources: manual entry via
`POST /ledgers/{id}/fx-rates` (admin or ledger owner), and a daily
ECB reference feed fetched by a background worker when
`FX_ECB_ENABLED=true`. A manually entered rate for a given day SHALL
take precedence over the feed. The worker MUST be idempotent: re-running
a day does not duplicate rows.

#### Scenario: Manual overrides feed

- **WHEN** the ECB worker stored EUR→USD 1.08 for 2026-08-21 and an
  owner then enters 1.09 for the same pair/day
- **THEN** subsequent lookups return 1.09 and the row's source is
  `manual`.

### Requirement: Foreign Amounts on Postings

A posting MAY specify `foreign_amount` and `foreign_currency`
(different from the ledger base currency). The posting service SHALL
derive the base-currency amount as `foreign_amount × rate(pair,
transaction_date)` rounded half-up to the ledger's currency exponent.
The double-entry invariant continues to hold on **base** amounts;
the DB balance trigger MUST NOT be relaxed.

#### Scenario: Foreign invoice converts at transaction-date rate

- **WHEN** a USD-ledger records a 1000 EUR expense dated 2026-08-23
  with rate 0.91 EUR→USD available
- **THEN** the debit posting stores base amount 910.00 USD with
  `foreign_amount=1000, foreign_currency='EUR'`, and the balancing
  credit is 910.00 USD.

### Requirement: Revaluation

Ledger owners SHALL be able to run month-end revaluation from the
ledger page (`POST /ledgers/{id}/fx-revaluation` with a date). The
action SHALL compute unrealized gain/loss for every non-zero balance
on foreign-currency monetary accounts (asset/liability subtypes),
post one balanced revaluation transaction to an auto-created
"FX Unrealized Gain/Loss" income/expense pair, and refuse to run
twice for the same `(account, month)`.

#### Scenario: Depreciating foreign bank balance

- **WHEN** a EUR account holds 1000 EUR worth 910 USD at month start
  and 890 USD at month end, and revaluation runs for that month
- **THEN** a balanced transaction posts 20.00 USD to FX loss and the
  same revaluation cannot be posted again for that month.

### Requirement: FX Gains Report

The reports index SHALL include "Realized & Unrealized FX Gains"
showing, per currency pair and period: realized gains (from
settlements/disposals of foreign balances), unrealized gains (from
the latest revaluation), and totals. The report SHALL be exportable
via the existing `export.csv` mechanism.

#### Scenario: Report separates realized from unrealized

- **WHEN** a ledger has both settled foreign invoices and a posted
  month-end revaluation
- **THEN** the report shows realized and unrealized sections with
  non-overlapping amounts.

