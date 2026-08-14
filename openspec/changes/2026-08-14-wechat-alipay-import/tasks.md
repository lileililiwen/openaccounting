# WeChat/Alipay Importers — Tasks

> Tests come first per `Agents.md §2.4`.

## 1. Testing

- [ ] 1.1 Unit: `encoding::decode` returns UTF-8 path for a UTF-8
      fixture and GB18030 path for a GBK fixture, and
      `UnsupportedEncoding` for a random byte sequence with
      invalid GB18030 characters.
- [ ] 1.2 Unit: `wechat::parse` on
      `tests/fixtures/wechat_personal_sample.csv` returns 2 rows
      with correct direction / amount / payee.
- [ ] 1.3 Unit: `wechat::parse` finds the header regardless of how
      many metadata rows precede it (parameterised: 0, 1, 16, 30).
- [ ] 1.4 Unit: `alipay_mobile::parse` on the mobile sample returns
      1 row by default and 2 rows when `include_other_infer=true`.
- [ ] 1.5 Unit: `alipay_web::parse` on the web sample returns rows
      and does not surface `付款账户`.
- [ ] 1.6 Unit: `dedup::normalize_payee` strips the agreed prefixes
      and lowercases.
- [ ] 1.7 Property: `dedup::fingerprint` is order-independent for
      1000 random tuples.
- [ ] 1.8 Property: `dedup::normalize_payee` is idempotent for 1000
      random strings.
- [ ] 1.9 Integration: `http_wechat_commit_creates_transactions` —
      5-row POST → 5 transactions visible in GET.
- [ ] 1.10 Integration: `http_wechat_commit_rolls_back_on_bad_row`
      — 0 transactions committed, HTTP 422.
- [ ] 1.11 Integration: `http_alipay_mobile_infers_credit_on_refund`
      — toggle on, row count = 2.
- [ ] 1.12 Integration: `http_alipay_web_handles_short_columns` —
      assert no `付款账户` field in rendered HTML.

## 2. Implementation

- [ ] 2.1 Add `encoding_rs = "0.8"` and `xxhash-rust = { version =
      "0.8", features = ["xxh3"] }` to `Cargo.toml`.
- [ ] 2.2 Create `src/import/mod.rs` with re-exports + the
      extended `ParsedRow` struct.
- [ ] 2.3 Create `src/import/encoding.rs` with `decode()` per the
      design doc.
- [ ] 2.4 Create `src/import/dedup.rs` with `fingerprint()` and
      `normalize_payee()`.
- [ ] 2.5 Create `src/import/wechat.rs` with the header sentinel
      scan + 11-column parser.
- [ ] 2.6 Create `src/import/alipay_mobile.rs` with the
      `收款 / 退款` inference logic.
- [ ] 2.7 Create `src/import/alipay_web.rs` with the short-column
      parser.
- [ ] 2.8 Create `src/handlers/import_wechat.rs` with `upload_page`,
      `preview`, `commit` handlers.
- [ ] 2.9 Create `src/handlers/import_alipay.rs` with the same
      three handlers for the Alipay routes.
- [ ] 2.10 Update `src/main.rs` to wire the 6 new routes.
- [ ] 2.11 Add `templates/import/wechat_upload.html`,
      `wechat_preview.html`, `alipay_upload.html`,
      `alipay_preview.html` (server-rendered, HTMX-friendly).
- [ ] 2.12 Add nav links from `templates/partials/_nav.html`:
      "Import → WeChat Pay" / "Import → Alipay".
- [ ] 2.13 Add fixture files under `tests/fixtures/` (one per
      platform, ≥3 data rows each).
- [ ] 2.14 Add `pub mod import_wechat; pub mod import_alipay;` to
      `src/handlers/mod.rs`.
- [ ] 2.15 Auto-detect redirect: in
      `src/handlers/import.rs::upload`, sniff the first line and
      303 to the platform-specific preview.

## 3. Validation

- [ ] 3.1 `openspec validate wechat-alipay-import` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green — all boxes under `## 1. Testing`
      pass.
- [ ] 3.5 Manual smoke: upload the WeChat fixture via the UI,
      confirm preview renders 2 rows, commit, assert 2 transactions
      appear in the list.
- [ ] 3.6 Manual smoke: upload the Alipay mobile fixture with the
      `include_other` toggle on, confirm 2 rows committed.
- [ ] 3.7 `openspec archive wechat-alipay-import` after all boxes
      are checked.
