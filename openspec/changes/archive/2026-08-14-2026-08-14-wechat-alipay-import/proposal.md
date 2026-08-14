# Add WeChat Pay and Alipay dedicated importers

## Why

A first-principles audit of openaccounting against its stated audience
("individuals and micro-enterprises", README) found that the dominant
CN payment platforms — **WeChat Pay** and **Alipay** — are not
supported. The only path today is a generic 7-column CSV uploader
whose `confirm` handler is a stub (it logs an audit row and redirects
without creating any transaction; see `src/handlers/import.rs:164`).

For CN individuals and SMBs, money lives in these wallets. Without
import support the system fails its first-principles question
("can I prove where the money came from and went?") at the very
first step — the data never enters the ledger.

References consulted for canonical column layouts and download paths:
- `jiegec/china_bean_importers` README (canonical Python parsers)
- `yann0917/alipay-wechat-merge` (Go reference parser)
- Alipay opendocs: `opendocs.alipay.com/b/03af7h`
- WeChat Pay download wiki: `pay.weixin.qq.com/wiki/doc/api/app/app.php?chapter=9_6&index=8`

## What Changes

- Add a new module `src/import/` containing one parser per platform:
  `wechat.rs`, `alipay_mobile.rs`, `alipay_web.rs`.
- Each parser is a pure function
  `(bytes: &[u8], filename: &str) -> Result<Vec<ParsedRow>, ParseError>`.
- Add `GB18030` encoding detection (encoding_rs) — these files are
  GBK-encoded in practice; UTF-8 also supported for completeness.
- Add a header-row sentinel scan: skip metadata rows until the
  canonical header token pattern is found.
- Add `money sign inference` from the explicit `收/支` column.
- Add `dedup fingerprint` `(date, amount_cents, normalized_payee)`
  using `xxhash-rust` for fast in-memory dedup.
- New HTTP routes:
  - `GET  /ledgers/{id}/import/wechat`         — upload page
  - `POST /ledgers/{id}/import/wechat`         — parse + preview
  - `POST /ledgers/{id}/import/wechat/commit`  — create transactions
  - `GET  /ledgers/{id}/import/alipay`         — upload page
  - `POST /ledgers/{id}/import/alipay`         — parse + preview
  - `POST /ledgers/{id}/import/alipay/commit`  — create transactions
- New templates: `templates/import/wechat_upload.html`,
  `templates/import/wechat_preview.html`,
  `templates/import/alipay_upload.html`,
  `templates/import/alipay_preview.html`.
- New audit events: `import.wechat.parse`, `import.wechat.commit`,
  `import.alipay.parse`, `import.alipay.commit`.

## Capabilities

### New Capabilities

- `data-import` — dedicated platform-aware importers (WeChat, Alipay
  mobile, Alipay web) with encoding detection, sentinel-row parsing,
  sign inference, and dedup.

### Modified Capabilities

- (none — bookkeeping, reports, auth, documents unchanged)

## Impact

- **New files:**
  - `src/import/mod.rs`
  - `src/import/wechat.rs`
  - `src/import/alipay_mobile.rs`
  - `src/import/alipay_web.rs`
  - `src/import/encoding.rs` (GB18030 detection helper)
  - `src/import/dedup.rs` (xxhash fingerprint)
  - `src/handlers/import_wechat.rs`, `src/handlers/import_alipay.rs`
  - `src/templates/import_wechat.rs`, `src/templates/import_alipay.rs`
  - `templates/import/wechat_upload.html`
  - `templates/import/wechat_preview.html`
  - `templates/import/alipay_upload.html`
  - `templates/import/alipay_preview.html`
  - `tests/integration/import_wechat.rs`
  - `tests/integration/import_alipay.rs`
  - `tests/fixtures/wechat_personal_sample.csv`
  - `tests/fixtures/alipay_mobile_sample.csv`
  - `tests/fixtures/alipay_web_sample.txt`
- **Modified files:**
  - `Cargo.toml` — add `encoding_rs = "0.8"`, `xxhash-rust = "0.8"`
  - `src/main.rs` — wire 6 new routes
  - `src/handlers/mod.rs` — `pub mod import_wechat; pub mod import_alipay;`
- **Database:** no migration (importers write through the existing
  `transactions` / `postings` table).
- **Backwards compatible:** existing generic CSV importer remains
  unchanged.

## Non-Goals

- WeChat **merchant-API** bill format (different layout, only
  relevant for platform-side integrations). Personal-account flow
  only.
- OCR of receipt images inside the bill. (Separate future change.)
- Auto-categorization rules (separate change).
- Live API integration with WeChat / Alipay. Only file imports.
