# CSV Import Column-Mapping Wizard

## Why

`src/handlers/import.rs` exists but I haven't read its full flow.
A typical user has a bank CSV with arbitrary columns; the import requires
the user to know which column is the amount and which is the date. A
column-mapping step is industry-standard.

## What Changes

- Three-step wizard: upload → map columns → preview → commit.
- The map step allows drag-and-drop between detected columns and the
  required ledger fields (date, amount, description, payee, account).
- The preview step shows the first 5 rows transformed.
- Saved mappings per (filename-pattern, format) so repeat imports are
  one-click.

## Capabilities

### New Capabilities

- `csv-import-wizard`: Step-by-step CSV importer.

## Impact

**New files:**
- `src/handlers/import_wizard.rs`.
- `templates/import/_wizard.html` (3 steps).
- `tests/http/csv_wizard.rs`.
