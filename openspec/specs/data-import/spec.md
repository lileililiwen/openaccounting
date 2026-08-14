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

### Requirement: WeChat Pay Bill Parser

The system SHALL accept WeChat Pay "personal-account" bill exports
(`我 → 服务 → 钱包 → 账单 → 常见问题 → 下载账单 → 用于个人对账`).
The uploaded file is a UTF-8 CSV wrapped in a password-protected ZIP;
the user uploads the extracted CSV (ZIP handling out of scope for
this change).

The parser SHALL:

- Read the file as UTF-8; reject if a non-UTF-8 byte sequence is
  found (use GB18030 fallback when the file is mislabelled — see
  `Encoding Fallback` requirement below).
- Skip metadata rows until it finds the header row whose first two
  fields are exactly `交易时间` and `交易类型`.
- Parse the 11 canonical columns:
  `交易时间, 交易类型, 交易对方, 商品, 收/支, 金额(元), 支付方式,
  当前状态, 交易单号, 商户单号, 备注`.
- Drop rows whose `当前状态` is not `支付成功` or `收款成功` (the
  user can toggle this in the preview page).
- Derive direction from `收/支`: `收入` → CREDIT, `支出` → DEBIT,
  `/` or empty → row dropped.
- Map a row to `ParsedRow { date, description, debit, credit,
  account, payee, reference, is_duplicate }`. `payee` = the
  `交易对方` field, with the literal `发给` prefix stripped.
- Return the rows sorted by `交易时间` ascending.

#### Scenario: A typical WeChat personal bill parses correctly

- **WHEN** the user uploads a UTF-8 CSV with one metadata banner, one
  blank line, the canonical 11-column header, and three rows
  (one 微信红包 -10.00 / 支出, one 退款 +5.00 / 收入, one
  transaction with `当前状态=已退款`)
- **THEN** the parser returns two `ParsedRow` values (the refunded
  row is dropped), the first is `{debit: "10.00", credit: "",
  payee: "Alice"}` and the second `{debit: "", credit: "5.00",
  payee: "Bob"}`.

#### Scenario: Header sentinel detection

- **WHEN** the CSV has 16 metadata rows before the canonical header
- **THEN** the parser skips all 16 metadata rows and starts reading
  data from row 17, regardless of how many metadata rows exist
  (no hard-coded `skip 16`).

#### Scenario: Encoding fallback to GB18030

- **WHEN** the file is a mislabelled GBK/GB18030 file containing
  identical CJK characters but invalid UTF-8 byte sequences
- **THEN** the parser transparently re-reads with `encoding_rs::GB18030`
  and produces the same `ParsedRow` sequence as the UTF-8 path.

### Requirement: Alipay Mobile Bill Parser

The system SHALL accept Alipay mobile "交易流水证明" exports
(`我的 → 账单 → ⋯ → 开具交易流水证明 → 用于个人对账 → 申请`).

The file is GBK-encoded CSV with a 16-column header:

`交易号, 商家订单号, 交易创建时间, 付款时间, 最近修改时间, 交易来源地,
类型, 交易对方, 商品名称, 金额（元）, 收/支, 交易状态, 服务费（元）,
成功退款（元）, 备注, 资金状态`.

The parser SHALL:

- Decode as GB18030 (UTF-8 files from the same path are also
  accepted).
- Detect the sentinel row whose first two fields are exactly
  `交易号` and `商家订单号`. The block may be preceded by other
  text (e.g. "---...支付宝（中国）网络技术有限公司...---"); skip
  until the sentinel.
- Treat `收/支` = `收入` as CREDIT, `支出` as DEBIT, `其他` /
  `不计收支` as drop-by-default with a toggle to keep and infer
  from `备注` (`退款` → CREDIT, `余额宝-单次转入` → DEBIT).
- Drop rows where `资金状态` ≠ `已收入` or `已支出` unless the user
  toggles "include pending" in the preview.

#### Scenario: Alipay mobile with refund row

- **WHEN** the CSV has the 16-column header and 2 data rows: one
  `支出 / 100.00` for merchant M, one `其他 / 100.00` with
  `备注=买家退款`
- **THEN** with default settings the parser returns one
  `ParsedRow { debit: "100.00", credit: "", payee: "M" }`.
  With the "include 其他 + infer" toggle enabled, it returns both,
  the second as `{debit: "", credit: "100.00"}`.

### Requirement: Alipay Web Bill Parser

The system SHALL accept Alipay web "下载Txt格式账单" exports. The
file is a `.txt` (not CSV) with 4 leading header rows and a
shorter-column layout:

`交易时间, 交易分类, 交易对方, 对方账号, 商品说明, 收/支, 金额,
收付款方式, 交易状态, 交易订单号, 商家订单号, 备注`.

The parser SHALL:

- Decode as GB18030.
- Skip the 4 header rows.
- Apply the same sign-inference and drop rules as the mobile
  variant.
- Note: the web variant does **not** include `付款账户`; the parser
  SHALL NOT pretend to surface that field (mark as unknown).

#### Scenario: Web export with a banner parses

- **WHEN** the user uploads a `.txt` file whose first 4 lines are
  banner/account/date rows followed by the 12-column header and
  three data rows (one 支出, one 收入, one 支出)
- **THEN** the parser skips the banner, returns three rows, and
  the rendered preview contains no `付款账户` field.

### Requirement: Encoding Fallback

When a file fails UTF-8 decode, the parser SHALL attempt GB18030
using `encoding_rs`. If GB18030 also fails, the parser SHALL
return a `ParseError::UnsupportedEncoding` with the offending
byte offset. UTF-8 BOM (`EF BB BF`) SHALL be silently stripped.

#### Scenario: GBK file decodes via GB18030

- **WHEN** a bill is a GBK-encoded byte sequence containing the
  CJK string `交易时间` (invalid UTF-8)
- **THEN** the parser decodes it as GB18030 and yields the same
  rows as the UTF-8 path.

#### Scenario: Random bytes are rejected

- **WHEN** the byte buffer is invalid in both UTF-8 and GB18030
- **THEN** the parser returns `ParseError::UnsupportedEncoding`.

### Requirement: Dedup Fingerprint

Each parsed row SHALL carry an `is_duplicate: bool` flag set to
true when `(txn_date, amount_cents, normalized_payee)` matches a
row already present in the same upload, OR matches a row already
in the destination ledger with the same date. The fingerprint is
`xxh3(date || '|' || amount_cents || '|' || normalized_payee)`.

`normalized_payee` is the payee string with leading/trailing
whitespace and the literals `微信`, `支付宝`, `(`, `)`, `有限公司`
stripped, lowercased, NFC-normalized.

#### Scenario: A re-uploaded row is flagged as a duplicate

- **WHEN** a bill is imported into a ledger that already contains
  a transaction on the same date, for the same amount, to the
  same normalized payee
- **THEN** the preview marks that row with `is_duplicate=true`
  and renders it with a yellow background.

The preview UI SHALL display duplicate rows with a yellow
background and exclude them from the commit unless the user
explicitly un-checks "skip duplicates".

### Requirement: Commit Endpoint

`POST /ledgers/{id}/import/wechat/commit` and
`POST /ledgers/{id}/import/alipay/commit` SHALL accept a form
with the parsed preview rows + the user's chosen default expense
account (or per-row account override) + the duplicate-skip flag.

For each non-duplicate row the handler SHALL create one
`transactions` row plus two `postings` (DR user-chosen expense
account; CR the ledger's default cash account identified by
`type='ASSET' AND subtype='cash'`). The double-entry invariant
trigger in `migrations/0001_init.sql` SHALL apply unchanged.

The commit SHALL be transactional: if any single row violates the
trigger, the entire batch is rolled back and the handler returns
`422 Unprocessable Entity` with the row index and message.

#### Scenario: Successful commit

- **WHEN** the user commits a 5-row WeChat preview with
  `default_expense_account_id=<id of "Office Supplies">` and
  `skip_duplicates=true`
- **THEN** 4 new `transactions` rows are created, each with two
  postings, and the response is `303 See Other` to the
  transactions list.

#### Scenario: Trigger violation rolls back the whole batch

- **WHEN** the user commits a 5-row batch but row 3 is a manually
  corrupted row with `amount = 0.00`
- **THEN** the response is `422` with body `Row 3 (line 19):
  amount must be > 0`; zero rows are committed; the audit log has
  only one `import.wechat.commit.failed` entry, no
  `import.wechat.commit.success`.

