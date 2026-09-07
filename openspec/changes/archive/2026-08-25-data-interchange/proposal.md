# Data Interchange: Bank Statement Formats and Learned Payee Matching

## Why

Bank data enters openaccounting as CSV (wizard) or live feeds. Every
peer handles standard statement files: GnuCash imports OFX/QIF/MT940,
Firefly III's importer ingests CAMT.052/053, Actual Budget supports
OFX/QFX/QIF/CAMT.053, hledger reads them via converters. Users
switching from those tools — or whose banks only export files — have
no path in.

On matching, GnuCash's OFX matcher *learns* payee→contra-account from
prior confirmations; Firefly III re-runs rules retroactively. Our
`reconciliation_rules` are static regex rules; the transaction-entry
payee field has no history-driven suggestions, so every invoice from
the same vendor is categorized from scratch.

## What Changes

- Statement file import: OFX v1/v2 (`.ofx/.qfx`), QIF, CAMT.052/053,
  MT940 — parsed into the existing `bank_statement_lines` pipeline so
  reconciliation, rules, and dedupe apply unchanged.
- Payee learning: a `payee_aliases` store built from confirmed
  reconciliations and manual edits; powers autocomplete suggestions in
  transaction entry and auto-suggests account + category during
  statement import with confidence scores.
- Retroactive rule application: "apply rules to selected history"
  action on the transactions list.

## Capabilities

### New Capabilities

- `statement-import-formats`: OFX/QIF/CAMT/MT940 ingestion.
- `payee-learning`: learned aliases drive suggestions and auto-matching.

## Impact

**New files:** `src/import/statement/{ofx,qif,camt,mt940}.rs`,
`src/domain/payee_learning.rs`, `src/handlers/rules_apply.rs`.
**Modified:** reconciliation import route (file-type sniffing),
transaction entry template (suggestions endpoint), wizard preview.
