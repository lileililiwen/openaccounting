# Receipt OCR — Design

## OCR engine trait

```rust
// src/ocr/mod.rs
use async_trait::async_trait;

#[derive(Debug, Clone, serde::Serialize)]
pub struct OcrResult {
    pub amount: Option<Decimal>,
    pub txn_date: Option<NaiveDate>,
    pub merchant: Option<String>,
    pub raw_text: String,
    pub confidence: f32,
}

#[async_trait]
pub trait OcrEngine: Send + Sync {
    async fn extract(&self, bytes: &[u8], mime: &str)
        -> Result<OcrResult, OcrError>;
}
```

## Tesseract adapter

```rust
// src/ocr/tesseract.rs
pub struct TesseractEngine {
    bin: PathBuf,
    lang: String,    // "eng+chi_sim" by default
}

#[async_trait]
impl OcrEngine for TesseractEngine {
    async fn extract(&self, bytes: &[u8], mime: &str)
        -> Result<OcrResult, OcrError>
    {
        // 1. If PDF: extract first page to PNG via `pdftoppm`.
        // 2. Write bytes to a temp file.
        // 3. Spawn `tesseract <path> stdout -l <lang> tsv`.
        // 4. Parse TSV; extract amount / date / merchant by
        //    regex + heuristic (first non-empty line of length
        //    4-40 = merchant; date from regexes; amount from
        //    numeric pattern with currency symbol).
        // 5. Compute mean word confidence from TSV's
        //    `conf` column.
    }
}
```

PDF support relies on `pdftoppm` being on the server PATH
(provided by `poppler-utils`). If absent, PDF OCR is rejected
with `OcrError::PdfToolingMissing` and the upload still
succeeds without OCR.

## Background task

```rust
// src/handlers/documents.rs (extension)
async fn enqueue_ocr(state: AppState, doc_id: Uuid, mime: String) {
    tokio::spawn(async move {
        let res = state.ocr.extract(&doc.bytes, &mime).await;
        match res {
            Ok(r)  => sqlx::query("INSERT INTO document_ocr_results
                (document_id, amount, txn_date, merchant, raw_text,
                 engine, confidence)
                VALUES ($1,$2,$3,$4,$5,'tesseract',$6)")
                .bind(doc_id).bind(r.amount).bind(r.txn_date)
                .bind(r.merchant).bind(r.raw_text).bind(r.confidence)
                .execute(&state.pool).await.ok(),
            Err(e) => sqlx::query("INSERT INTO document_ocr_results
                (document_id, raw_text, engine, confidence)
                VALUES ($1, '', 'tesseract', 0)")
                .bind(doc_id).execute(&state.pool).await.ok(),
        };
    });
}
```

## Tests

### Unit

- `parse_amount("¥1,234.56")` returns `Some(Decimal::new(123456, 2))`.
- `parse_amount("Total: 12.34 USD")` returns `Some(1234)`.
- `parse_date("Aug 14, 2026")` returns `Some(2026-08-14)`.
- `parse_date("2026-08-14")` returns `Some(2026-08-14)`.
- `extract_merchant("STARBUCKS COFFEE\n123 Main St\n...")` returns
  `Some("STARBUCKS COFFEE")`.

### Integration

- `http_upload_image_kicks_off_ocr` — upload, GET document
  page → "OCR pending" badge; poll until result populated.
- `http_apply_creates_reimbursement_line` — extract +
  apply → line in DB.
- `http_upload_with_ocr_false_skips_job` — no
  `document_ocr_results` row.
- `http_pdf_without_pdftoppm_returns_error_but_upload_succeeds`.

### Property

- `prop_amount_parser_handles_currency_symbols` — 1000
  random `(symbol, digits)` combos parse to the right
  decimal.

## References

- Tesseract OCR (`github.com/tesseract-ocr/tesseract`)
- `tesseract-plumbing` Rust crate
- Expensify SmartScan (closed; we infer behaviour from docs)
- Firefly III attachment rules
