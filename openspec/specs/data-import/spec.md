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

### Requirement: Generic CSV Importer

(Replaces the existing generic-CSV behaviour, which was a stub.)

`POST /ledgers/{id}/import/confirm` SHALL accept a form whose
hidden inputs echo the parsed preview (filename, column
mapping, all rows, the user-chosen `default_account_id`, and
`skip_duplicates: bool`).

The handler SHALL, for every non-duplicate row in the preview:

1. Resolve the row's `account` field (column 5) to a ledger
   account by case-insensitive exact name match; if absent,
   use `default_account_id`.
2. Parse `txn_date` as ISO `YYYY-MM-DD`; reject the row with a
   `BadRow` error if the format is wrong.
3. Parse `amount` (debit OR credit) as `Decimal` ≥ 0.01; reject
   rows where neither debit nor credit is set, where both are
   set, or where either fails to parse.
4. Insert one `transactions` row and two `postings` (DR
   resolved account; CR the ledger's default cash account
   identified by `type='ASSET' AND subtype='cash'`). The
   transaction description is the row's `description` field;
   the payee is `payee`; the reference is `reference`.
5. Increment a per-batch counter.

The whole batch SHALL run inside a single Postgres transaction.
If any single row fails validation or the
`check_posting_balance` trigger fires, the entire batch is
rolled back and the response is `422 Unprocessable Entity` with
a body listing each error as `Row <n>: <message>`.

If all rows succeed, the response is `303 See Other` to
`/ledgers/{id}/transactions?from=<earliest txn_date>`.

#### Scenario: Successful commit creates N transactions

- **WHEN** the user commits a preview of 12 valid rows with
  `default_account_id=<id of "Other Expense">` and
  `skip_duplicates=true`
- **THEN** 12 new `transactions` rows are created (or fewer if
  some are duplicates), each with two postings, and the
  response is `303 See Other` to the transactions list.

#### Scenario: Bad row rolls back the batch

- **WHEN** the preview contains 1 row whose `txn_date` is
  `2026-13-99` and 9 valid rows
- **THEN** the response is `422` with body
  `Row 1: invalid date "2026-13-99". 0 transactions committed.`,
  and zero rows are added to `transactions` or `postings`.

#### Scenario: Both debit and credit set is rejected

- **WHEN** a row has `debit=100.00` AND `credit=100.00`
- **THEN** the response is `422` with body
  `Row <n>: exactly one of debit or credit must be set.`.

### Requirement: Preview Cap Raised

The preview handler `POST /ledgers/{id}/import` SHALL parse up
to 10,000 data rows (raised from the previous 10-row cap). If
the file exceeds 10,000 rows, the preview page SHALL display
a warning banner "Showing the first 10,000 rows. The full file
will be processed on commit."

#### Scenario: 500-row file

- **WHEN** the user uploads a 500-row CSV
- **THEN** the preview shows all 500 rows and the commit
  creates 500 transactions (or fewer with duplicates skipped).

### Requirement: Duplicate Skip

When `skip_duplicates=true` (the default), the commit handler
SHALL skip any row whose
`(txn_date, amount_cents, normalized_payee)` fingerprint
matches a transaction already in the ledger. A row whose
fingerprint matches another row **within the same upload** is
also skipped.

The preview page SHALL flag duplicates with a yellow background
so the user can see what will be skipped before committing.

The fingerprint is computed by the `import::dedup` helper
introduced by the WeChat/Alipay change.

#### Scenario: Duplicate row is skipped

- **WHEN** the user commits a preview of 2 rows where row 1
  and row 2 share `(date, amount, payee)` and
  `skip_duplicates=true`
- **THEN** only one `transactions` row is created and the
  preview marks the second row as `is_duplicate=true`.

