# Add Bank-Statement Imports (OFX, QIF, MT940)

## Why

The current importer (`src/handlers/import.rs`) only accepts a
7-column CSV. Three canonical bank-statement formats —
**OFX** (Open Financial Exchange, used by US/UK/EU banks and
Quicken/Moneydance exports), **QIF** (Quicken Interchange
Format, legacy but ubiquitous), and **MT940** (SWIFT Customer
Statement Message, the EU standard) — are not supported. Banks
that do not offer a CSV export leave users stranded.

These formats are well-documented (OFX 2.x SGML/XML,
QIF Intuit spec, MT940 SWIFT standard) and the Firefly III
Data Importer (`github.com/firefly-iii/data-importer`) and
GnuCash AqBanking already prove they can be parsed in
production.

## What Changes

- New module `src/import/ofx.rs` (SGML + XML variants),
  `src/import/qif.rs`, `src/import/mt940.rs`.
- Auto-detection of file format by content sniff (header
  patterns, magic strings).
- New routes:
  - `GET  /ledgers/{id}/import/ofx`
  - `POST /ledgers/{id}/import/ofx/commit`
  - `GET  /ledgers/{id}/import/qif`
  - `POST /ledgers/{id}/import/qif/commit`
  - `GET  /ledgers/{id}/import/mt940`
  - `POST /ledgers/{id}/import/mt940/commit`
- Generic `/ledgers/{id}/import` auto-detects any of the three
  formats and 303-redirects to the platform-specific preview.
- Each importer extends the existing `ParsedRow` and dedup
  pipeline.
- New audit events: `import.ofx.commit.success` / `.failed`,
  same for `qif` and `mt940`.

## Capabilities

### Modified Capabilities

- `data-import` — three new format-specific parsers, sharing
  the dedup / sign-inference infrastructure.

## Impact

- **New files:**
  - `src/import/ofx.rs`
  - `src/import/qif.rs`
  - `src/import/mt940.rs`
  - `src/import/sniff.rs` (format detector)
  - `src/handlers/import_ofx.rs`, `import_qif.rs`, `import_mt940.rs`
  - `src/templates/import_{ofx,qif,mt940}.rs`
  - `templates/import/{ofx_upload,ofx_preview,qif_upload,
    qif_preview,mt940_upload,mt940_preview}.html`
  - `tests/integration/{import_ofx,import_qif,import_mt940}.rs`
  - `tests/fixtures/{ofx,qif,mt940}_sample.{ofx,qif,sta}`
- **Modified files:**
  - `src/main.rs` — 6 new routes.
  - `src/handlers/mod.rs` — `pub mod import_ofx; …`.
  - `src/handlers/import.rs::upload` — sniff + 303 redirect.

## Non-Goals

- CAMT.052 / CAMT.053 (EU bank XML standards) — separate
  future change.
- OFX 1.x (the legacy SGML with `<OFX>` tag, vs OFX 2.x XML).
  Both are supported via the same parser.
- Live bank connections (handled by the separate `bank-feeds`
  change).
