# WeChat/Alipay Importers — Design

## Module layout

```
src/import/
├── mod.rs                 // re-exports + ParsedRow
├── encoding.rs            // detect + decode (UTF-8 → GB18030 fallback)
├── dedup.rs               // xxh3 fingerprint + normalize_payee
├── wechat.rs              // parse(bytes, filename) -> Vec<ParsedRow>
├── alipay_mobile.rs       // parse(bytes) -> Vec<ParsedRow>
└── alipay_web.rs          // parse(bytes) -> Vec<ParsedRow>
```

All three `parse` functions share the same signature:

```rust
pub fn parse(bytes: &[u8], filename: &str) -> Result<Vec<ParsedRow>, ParseError>
```

`ParsedRow` is the existing struct in `src/handlers/import.rs:30`
extended with `pub fingerprint: u64` and `pub platform:
ImportPlatform { Wechat, AlipayMobile, AlipayWeb }`.

## Encoding detection

```rust
// src/import/encoding.rs
pub fn decode(bytes: &[u8]) -> Result<(Cow<str>, &'static str), ParseError> {
    // 1. strip UTF-8 BOM if present
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    // 2. try strict UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok((Cow::Borrowed(s), "UTF-8"));
    }
    // 3. fall back to GB18030 (covers GBK and GB2312)
    let (cow, _enc, had_errors) = encoding_rs::GB18030.decode(bytes);
    if had_errors {
        return Err(ParseError::UnsupportedEncoding);
    }
    Ok((Cow::Owned(cow.into_owned()), "GB18030"))
}
```

## Sentinel header scan

Each parser walks the rows until it finds the canonical header.
This avoids hard-coded skip counts that break when Alipay adds a
banner row.

```rust
fn find_header<'a>(
    rows: impl Iterator<Item = &'a str>,
    expected: &[&str],
) -> Option<usize> {
    rows.position(|row| {
        let cells: Vec<&str> = row.split(',').collect();
        cells.iter().take(expected.len()).eq(expected.iter().copied())
    })
}
```

For WeChat: `expected = ["交易时间", "交易类型"]`.
For Alipay mobile: `expected = ["交易号", "商家订单号"]`.
For Alipay web: `expected = ["交易时间", "交易分类"]`.

## Sign inference

```rust
fn direction(收支: &str, note: &str, include_other: bool) -> Option<Direction> {
    match 收支 {
        "收入" => Some(Direction::Credit),
        "支出" => Some(Direction::Debit),
        "/" | "" => None,
        "其他" | "不计收支" if include_other && note.contains("退款") =>
            Some(Direction::Credit),
        "其他" | "不计收支" if include_other && note.contains("余额宝-单次转入") =>
            Some(Direction::Debit),
        _ => None,
    }
}
```

## Dedup fingerprint

```rust
fn fingerprint(date: NaiveDate, amount_cents: i64, payee: &str) -> u64 {
    let normalized = normalize_payee(payee);
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.write(date.to_string().as_bytes());
    h.write(b"|");
    h.write(amount_cents.to_le_bytes().as_slice());
    h.write(b"|");
    h.write(normalized.as_bytes());
    h.digest()
}

fn normalize_payee(s: &str) -> String {
    s.trim()
        .replace("微信", "")
        .replace("支付宝", "")
        .replace("(", "")
        .replace(")", "")
        .replace("有限公司", "")
        .to_lowercase()
        .nfc() // unicode-normalization
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
```

Existing-ledger dedup runs the same fingerprint against the
ledger's last-90-days transactions at preview time.

## Routes

```rust
.route("/ledgers/{id}/import/wechat",          get(handlers::import_wechat::upload_page))
.route("/ledgers/{id}/import/wechat",          post(handlers::import_wechat::preview))
.route("/ledgers/{id}/import/wechat/commit",   post(handlers::import_wechat::commit))
.route("/ledgers/{id}/import/alipay",          get(handlers::import_alipay::upload_page))
.route("/ledgers/{id}/import/alipay",          post(handlers::import_alipay::preview))
.route("/ledgers/{id}/import/alipay/commit",   post(handlers::import_alipay::commit))
```

Auto-detect: when the user uploads to the bare `/ledgers/{id}/import`
endpoint with a file whose first 3 bytes match the WeChat UTF-8 BOM
**or** whose first non-blank line is one of the three known header
sentinels, the generic handler redirects to the dedicated preview
with `?auto=1`. This keeps the single "Import" button discoverable
while routing to the correct parser.

## Transactions

The `src/db/pool.rs` already provides `PgPool`. Each `commit`
handler opens a single `tx.begin().await?`, batches inserts, and
commits atomically. The existing `check_posting_balance` trigger
enforces the invariant per-row.

## Tests

### Unit (in each parser file)

- `wechat::parse` on `tests/fixtures/wechat_personal_sample.csv`
  returns the expected 2 rows.
- `wechat::parse` on a UTF-8 file with 16 metadata rows finds the
  header at row 17.
- `wechat::parse` on a GB18030 file (identical content, different
  bytes) returns the same rows.
- `alipay_mobile::parse` infers credit on `其他 + 退款` rows when
  the toggle is set.
- `dedup::fingerprint` is stable across reordering.

### Integration (in `tests/integration/`)

- `import_wechat::http_wechat_commit_creates_transactions` —
  POST 5 non-duplicate rows, GET the ledger's transactions list,
  assert 5 rows present.
- `import_wechat::http_wechat_commit_rolls_back_on_bad_row` —
  inject a zero-amount row, assert HTTP 422, assert 0 new
  transactions.
- `import_alipay::http_alipay_mobile_infers_credit_on_refund` —
  upload the mobile sample, toggle include-other, assert row count.
- `import_alipay::http_alipay_web_handles_short_columns` —
  upload the web sample, assert rows present despite missing
  `付款账户`.

### Property

- `prop_dedup_fingerprint_is_order_independent` — for any set of
  `(date, amount, payee)` tuples, the set of fingerprints equals
  the set after `Vec::shuffle`.
- `prop_normalize_payee_idempotent` —
  `normalize(normalize(x)) == normalize(x)` for 1000 random CN +
  EN payee strings.

## References

- jiegec/china_bean_importers README + per-platform files
- yann0917/alipay-wechat-merge README
- Alipay opendocs `03af7h` and `089ccd`
- WeChat Pay download wiki `chapter=9_6&index=8`
- CacinieP/FinancialBeancount (3-tier dedup pattern)
