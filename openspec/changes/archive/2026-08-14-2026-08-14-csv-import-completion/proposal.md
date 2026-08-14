# Complete the Generic CSV Importer

## Why

The generic CSV importer at
`POST /ledgers/{id}/import/confirm` is currently a stub:

```rust
// src/handlers/import.rs:174
// For now, just redirect back with success message
// Full implementation would parse CSV, validate, and create transactions
let _ = audit::log(…).await;
Ok(Redirect::to(...).into_response())
```

It writes an audit row and redirects, but creates zero
transactions. The preview is hard-capped at 10 rows
(`src/handlers/import.rs:112`). The data-import capability is
therefore unusable for any user with more than 10 transactions
to import.

The WeChat/Alipay importer (separate change) builds on top of
the same `ParsedRow` struct, so completing the generic path
removes a shared gap and is a prerequisite for the platform
parsers.

## What Changes

- `confirm` handler actually parses the form-encoded rows and
  inserts one transaction per row (or one per N legs if a row
  has both a debit and a credit — split mode).
- Preview cap removed (or raised to 10,000).
- Add per-row validation with structured error reporting back to
  the preview page.
- Add atomicity: a single bad row rolls back the whole batch.
- Add a `default_account_id` form field on the confirm form so
  the user chooses which expense (or income) account the
  imported lines go to, instead of always defaulting to a
  guessed account.
- New audit event: `import.generic.commit.success`,
  `import.generic.commit.failed`.

## Capabilities

### Modified Capabilities

- `data-import` — wire `confirm` to real inserts, raise the
  preview cap, add structured errors, atomicity, and an
  account-selector.

## Impact

- **Modified files:**
  - `src/handlers/import.rs` — `confirm` handler
  - `src/templates/import.rs` — `ImportConfirm` Askama struct
  - `templates/import/preview.html` — show all rows; add
    per-row error column; add account selector
  - `tests/integration/csv_import.rs` (new)

## Non-Goals

- Platform-specific parsing (covered by the WeChat/Alipay
  change).
- Per-column-type auto-detection (date format, amount format)
  — assume well-formed CSV in the canonical 7-column shape.
- Re-import safety beyond `(date, amount_cents, payee)`
  fingerprint (same as platform importers).
