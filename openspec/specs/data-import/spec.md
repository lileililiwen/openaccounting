# data-import Specification

## Purpose
TBD - created by archiving change 2026-08-14-bank-statement-imports. Update Purpose after archive.
## Requirements
### Requirement: OFX Importer

The system SHALL accept OFX (Open Financial Exchange) files in
both QFX (SGML, header begins with `OFXHEADER:100`) and 2.x XML
variants. The file SHALL be sniffed from the first non-blank
line; SGML begins with `OFXHEADER:` while XML begins with `<?xml`.

For each `<STMTTRN>` (SGML) or `<bankTransaction>` (XML) entry
the parser SHALL extract:

- `DTPOSTED` → `txn_date` (UTC, YYYYMMDDHHMMSS or YYYYMMDD).
- `TRNAMT`  → `amount` (signed; negative = debit in OFX
  convention, mapped to Direction::Debit; positive = credit).
- `NAME`    → `payee`.
- `MEMO`    → description (falls back to NAME if empty).
- `FITID`   → `reference` (used by dedup fingerprint).

`STMTTRN` rows whose `TRNTYPE=DEP` or `TRNTYPE=DIRECTDEP` are
credit; `TRNTYPE=PAYMENT|CHECK|WITHDRAWAL|FEE|ATM|DEBIT` are
debit; everything else defaults to DEBIT and surfaces a soft
warning in the preview.

#### Scenario: QFX SGML parses correctly

- **WHEN** the user uploads a QFX file with one `<STMTTRN>`
  row: `DTPOSTED=20260814120000, TRNAMT=-42.50, NAME=STARBUCKS,
  MEMO=Latte, FITID=20260814001`
- **THEN** the parser returns one `ParsedRow { date:
  "2026-08-14", description: "Latte", debit: "42.50",
  credit: "", payee: "STARBUCKS", reference:
  "20260814001" }`.

#### Scenario: OFX 2.x XML parses correctly

- **WHEN** the user uploads an XML OFX 2.x file with the
  `<bankTransaction>` equivalent of the above
- **THEN** the same row is produced.

### Requirement: QIF Importer

The system SHALL accept QIF (Quicken Interchange Format) files.
Each non-header line is a transaction field; transactions are
separated by a line beginning with `^`.

Field mapping:

- `D` → `txn_date` (format `MM/DD/YYYY` or `MM/DD'YY`).
- `T` → `amount` (signed; negative = debit, positive = credit).
- `P` → `payee`.
- `M` → `memo / description`.
- `N` → `reference` (cheque number).
- `C` → cleared flag (informational; not stored).
- `L` → category (informational; surfaces as GL suggestion in
  preview).

The first non-blank line is the header (`!Type:Bank` etc.) and
MUST be skipped.

#### Scenario: QIF parses correctly

- **WHEN** the user uploads a QIF with one transaction:
  `D08/14/2026`, `T-12.34`, `PSupermarket`, `MWeekly shop`,
  `N123`, `^`
- **THEN** the parser returns `{date:"2026-08-14",
  description:"Weekly shop", debit:"12.34", credit:"",
  payee:"Supermarket", reference:"123"}`.

### Requirement: MT940 Importer

The system SHALL accept MT940 (SWIFT Customer Statement
Message) files. Each `:61:` line begins a transaction; the
following `:86:` line carries narrative.

Field mapping:

- `:61:` YYMMDD[MMDD] + D|C + amount + transaction type code
  + `N` + bank reference. (D = debit, C = credit.)
- `:86:` ?20 / ?21 / ?22 / ?23 / ?24 / ?25 / ?30 / ?31 /
  ?32 / ?33 sub-fields are concatenated (with `/` separators)
  to form the memo; the merchant is typically in `?32` or
  `?21`. The parser SHALL prefer the `?32` field as the payee
  if present, falling back to the full concatenated string.

The `:60F:` and `:62F:` lines (opening / closing balance) are
read but not converted to postings; they appear in the preview
as informational footer text.

#### Scenario: MT940 parses correctly

- **WHEN** the user uploads an MT940 file with `:61:
  2608140814D42,50N024NONREF//Settlement` and `:86: ?32STARBUCKS
  ?21Latte` immediately following
- **THEN** the parser returns `{date:"2026-08-14",
  description:"Settlement", debit:"42.50", credit:"",
  payee:"STARBUCKS", reference:"024NONREF"}`.

### Requirement: Format Sniff

`POST /ledgers/{id}/import` SHALL inspect the first non-blank
line of the uploaded file and dispatch to the matching
parser:

| First non-blank line             | Parser                                 |
|----------------------------------|----------------------------------------|
| `OFXHEADER:`                     | `import::ofx::parse`                    |
| `<?xml version=` AND contains `<OFX>` | `import::ofx::parse` (XML variant) |
| `!Type:`                          | `import::qif::parse`                    |
| `:20:` followed by `:25:` AND `:60F:` or `:60M:` | `import::mt940::parse`         |
| (otherwise)                      | generic CSV reader (existing)           |

The single preview page is rendered with a `format: <name>`
chip so the user knows which parser produced the rows. The
implementation MAY also 303-redirect to a per-format URL; the
spec is satisfied as long as the same preview payload is
returned.

#### Scenario: QFX file dispatches to OFX

- **WHEN** the user uploads a file whose first non-blank line
  is `OFXHEADER:100`
- **THEN** the handler renders the preview using
  `import::ofx::parse` and the `format` chip reads `ofx`.

#### Scenario: Unknown file format falls through to CSV

- **WHEN** the user uploads a file whose first non-blank line
  is `date,description,amount`
- **THEN** the existing CSV reader is used and the `format`
  chip reads `csv`.

### Requirement: Cross-Format Dedup

The dedup fingerprint introduced by
`2026-08-14-wechat-alipay-import` SHALL be applied uniformly
across all importers (CSV, OFX, QIF, MT940, WeChat, Alipay).
A row whose fingerprint matches a transaction already in the
ledger is marked `is_duplicate=true` regardless of source
format.

#### Scenario: Re-imported QIF row is marked duplicate

- **WHEN** the user commits a QIF file containing a row whose
  `(date, amount, payee)` already exists in the ledger
- **THEN** the row's `is_duplicate` flag is `true` and the
  commit step skips it (the row is not inserted twice).

