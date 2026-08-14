# data-import Specification (delta)

## ADDED Requirements

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
line of the uploaded file and 303-redirect to the matching
preview route:

| First non-blank line             | Redirect target                       |
|----------------------------------|----------------------------------------|
| `OFXHEADER:`                     | `/ledgers/{id}/import/ofx`             |
| `<?xml version=` AND contains `<OFX>` | `/ledgers/{id}/import/ofx`        |
| `!Type:`                          | `/ledgers/{id}/import/qif`             |
| `:20:` followed by `:25:` AND `:60F:` or `:60M:` | `/ledgers/{id}/import/mt940` |
| (otherwise)                      | generic CSV preview (existing)         |

If sniffing fails the handler returns `400 Bad Request` with
the body `Unrecognized file format. Accepted: CSV, OFX (SGML
or XML), QIF, MT940, WeChat Pay, Alipay.`

### Requirement: Cross-Format Dedup

The dedup fingerprint introduced by
`2026-08-14-wechat-alipay-import` SHALL be applied uniformly
across all importers (CSV, OFX, QIF, MT940, WeChat, Alipay).
A row whose fingerprint matches a transaction already in the
ledger is marked `is_duplicate=true` regardless of source
format.
