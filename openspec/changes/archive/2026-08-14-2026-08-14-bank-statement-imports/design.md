# Bank-Statement Imports — Design

## Format detection (`src/import/sniff.rs`)

```rust
pub enum DetectedFormat { Csv, OfxSgml, OfxXml, Qif, Mt940 }

pub fn detect(bytes: &[u8]) -> Option<DetectedFormat> {
    let first_line = bytes.iter()
        .take_while(|&&b| b != b'\n')
        .copied().collect::<Vec<u8>>();
    let first = String::from_utf8_lossy(&first_line);
    if first.starts_with("OFXHEADER:") { return Some(OfxSgml); }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]);
    if head.contains("<?xml") && head.contains("<OFX>") {
        return Some(OfxXml);
    }
    if first.starts_with("!Type:") { return Some(Qif); }
    let txt = String::from_utf8_lossy(bytes);
    if txt.contains(":20:") && txt.contains(":25:")
       && (txt.contains(":60F:") || txt.contains(":60M:")) {
        return Some(Mt940);
    }
    None
}
```

## OFX parser

OFX SGML has no proper XML escaping and uses CRLF-less
header/body separation. The body starts after the first blank
line. We use `quick-xml` with the `sgml` workaround
(`reader.config().trim_text(true); allow_doctype_yes()`),
or fall back to a hand-written SGML scanner for the simpler
QFX flavor (no nested tags).

For 2.x XML we use `quick-xml` natively.

A unified AST `Vec<OfxTxn>` feeds `ParsedRow`.

## QIF parser

Hand-written state machine walking lines:
- `!Type:…` → start of file; store type (Bank/Card/etc.).
- `D`, `T`, `P`, `M`, `N`, `L` → set fields on the current
  transaction.
- `^` → push the current transaction into the output vec;
  reset.

The state machine is straightforward and tested with the
fixture.

## MT940 parser

Hand-written. Each `:61:` opens a transaction; the
immediately following `:86:` is the narrative. Field
positions are fixed by SWIFT spec. Amount sign comes from the
D/C flag in `:61:`. Bank reference comes from the substring
between `N` and `//` in `:61:`.

We also extract the `:60F:` (opening) and `:62F:` (closing)
balance lines and surface them as informational footer text
in the preview, NOT as postings.

## Routes

```
GET  /ledgers/{id}/import/ofx     → upload page
POST /ledgers/{id}/import/ofx     → preview
POST /ledgers/{id}/import/ofx/commit
GET  /ledgers/{id}/import/qif     → upload page
POST /ledgers/{id}/import/qif     → preview
POST /ledgers/{id}/import/qif/commit
GET  /ledgers/{id}/import/mt940   → upload page
POST /ledgers/{id}/import/mt940   → preview
POST /ledgers/{id}/import/mt940/commit
```

The generic `POST /ledgers/{id}/import` calls `detect()` and
either 303s to the platform-specific route or shows the
existing CSV preview.

## Tests

### Unit

- `sniff::detect` returns the right enum for each fixture.
- `ofx::parse` on a QFX sample returns one `ParsedRow`.
- `ofx::parse` on an OFX 2.x XML sample returns the same row.
- `qif::parse` returns one `ParsedRow` for a 1-line file.
- `mt940::parse` extracts a transaction with merchant from
  `:86: ?32`.

### Integration

- `http_ofx_commit_creates_transactions` — 5 rows → 5 txns.
- `http_qif_commit_creates_transactions`.
- `http_mt940_commit_creates_transactions`.
- `http_import_redirects_to_ofx_route` — upload a QFX file to
  generic endpoint → 303 to `/import/ofx`.
- `http_import_unknown_format_returns_400`.

### Property

- `prop_sniff_is_idempotent` — re-sniffing the first 4 KB of
  a 100 MB file yields the same result as sniffing the whole
  file.

## References

- OFX 2.x schema: `financialdataexchange.org`
- QIF Intuit spec (publicly mirrored)
- MT940 SWIFT standard (SCR2.0 / MT940 handbook)
- Firefly III Data Importer
  (`github.com/firefly-iii/data-importer`)
- GnuCash AqBanking
