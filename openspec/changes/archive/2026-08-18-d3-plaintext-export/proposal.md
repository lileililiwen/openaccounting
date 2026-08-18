# Plain-Text Accounting (PTA) Export and Import

## Why

The Beancount export is mentioned in O1. This change promotes it to
a peer of the official system: export AND import, plus an hledger
flavor, plus a CLI subcommand for scripted round-trips. Users who use
Beancount / Fava / hledger / Paisa can treat OpenAccounting as a UI
over their existing plain-text books.

## What Changes

- Export: same as O1, but as a subcommand of the binary:
  `openaccounting export <ledger> --format=beancount > books.bean`.
- Import: `openaccounting import <ledger> --format=beancount` parses
  and inserts.
- hledger flavor: same export with `date,desc,account1,amount1,…` CSV.
- Round-trip test: 100 random ledgers → Beancount → re-import → equal.

## Capabilities

### New Capabilities

- `pta-export-import`: Plain-text export and import.

## Impact

**New files:**
- `src/bin/cmd_export.rs`, `src/bin/cmd_import.rs`.
- `src/import/beancount.rs`, `src/import/hledger.rs`.
- `tests/integration/pta_round_trip.rs`.
