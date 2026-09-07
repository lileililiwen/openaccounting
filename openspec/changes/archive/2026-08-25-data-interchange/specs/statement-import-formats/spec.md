# statement-import-formats Specification (delta)

## ADDED Requirements

### Requirement: Supported Formats

The reconciliation import (`POST /reconcile/{account_id}/import`) SHALL
accept, in addition to CSV: OFX v1 (SGML) and v2 (XML) including
`.qfx`, QIF, CAMT.052/053 (ISO 20022 XML), and MT940/MT942 text.
Format SHALL be detected by content sniffing (root element / headers),
not file extension. Each parsed line maps to the existing
`bank_statement_lines` shape: date, amount, currency (if present),
payee/name, reference/memo, external id.

#### Scenario: OFX file lands in the same pipeline

- **WHEN** a user uploads a `.qfx` export for the reconciled account
- **THEN** parsed lines appear on the reconciliation page exactly as
  CSV imports do, and dedupe against existing lines applies.

#### Scenario: Sniffing beats lying extensions

- **WHEN** a CAMT.053 file is renamed `statement.csv`
- **THEN** it still parses as CAMT and imports.

### Requirement: Duplicate Detection Across Formats

A statement line is a duplicate when `(account_id, external_id)`
matches an imported line, or — where the format lacks stable ids
(QIF) — when `(date, amount, normalized payee)` matches within the
same import batch or existing rows. Duplicates SHALL be flagged, not
silently dropped; the user confirms per row.

#### Scenario: Re-downloaded OFX is idempotent

- **WHEN** the same OFX file is imported twice
- **THEN** the second run flags every line as duplicate and creates none.

### Requirement: Currency Handling

Lines carrying a currency different from the account's currency SHALL
be rejected with a row-level error naming both codes (FX conversion
belongs to the `multi-currency-fx` capability, not the importer).

#### Scenario: Mismatched currency fails loudly

- **WHEN** a USD-account receives a EUR-denominated CAMT statement
- **THEN** those rows error with "EUR vs account USD" and valid rows
  may still be confirmed.

---
