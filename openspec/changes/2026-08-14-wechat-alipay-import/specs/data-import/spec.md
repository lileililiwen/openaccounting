# data-import Specification (delta)

## ADDED Requirements

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

### Requirement: Encoding Fallback

When a file fails UTF-8 decode, the parser SHALL attempt GB18030
using `encoding_rs`. If GB18030 also fails, the parser SHALL
return a `ParseError::UnsupportedEncoding` with the offending
byte offset. UTF-8 BOM (`EF BB BF`) SHALL be silently stripped.

### Requirement: Dedup Fingerprint

Each parsed row SHALL carry an `is_duplicate: bool` flag set to
true when `(txn_date, amount_cents, normalized_payee)` matches a
row already present in the same upload, OR matches a row already
in the destination ledger with the same date. The fingerprint is
`xxh3(date || '|' || amount_cents || '|' || normalized_payee)`.

`normalized_payee` is the payee string with leading/trailing
whitespace and the literals `微信`, `支付宝`, `(`, `)`, `有限公司`
stripped, lowercased, NFC-normalized.

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
