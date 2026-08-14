# Bank-Statement Imports — Tasks

## 1. Testing

- [ ] 1.1 Unit: `sniff::detect` returns the right enum for each
      fixture (CSV, OFX SGML, OFX XML, QIF, MT940).
- [ ] 1.2 Unit: `ofx::parse` on a QFX sample returns one
      `ParsedRow`.
- [ ] 1.3 Unit: `ofx::parse` on an OFX 2.x XML sample returns
      the same row.
- [ ] 1.4 Unit: `qif::parse` returns one `ParsedRow` for a
      1-line file.
- [ ] 1.5 Unit: `mt940::parse` extracts merchant from
      `:86: ?32`.
- [ ] 1.6 Property: `prop_sniff_is_idempotent` for 1000
      random prefixes.
- [ ] 1.7 Integration: `http_ofx_commit_creates_transactions`.
- [ ] 1.8 Integration: `http_qif_commit_creates_transactions`.
- [ ] 1.9 Integration: `http_mt940_commit_creates_transactions`.
- [ ] 1.10 Integration: `http_import_redirects_to_ofx_route`.
- [ ] 1.11 Integration: `http_import_unknown_format_returns_400`.

## 2. Implementation

- [ ] 2.1 Add `quick-xml = "0.36"` to `Cargo.toml`.
- [ ] 2.2 `src/import/sniff.rs` with `detect()`.
- [ ] 2.3 `src/import/ofx.rs` — SGML + XML parsers.
- [ ] 2.4 `src/import/qif.rs` — line-walking state machine.
- [ ] 2.5 `src/import/mt940.rs` — `:61:` / `:86:` parser.
- [ ] 2.6 `src/handlers/import_ofx.rs` — `upload_page`,
      `preview`, `commit`.
- [ ] 2.7 `src/handlers/import_qif.rs` — same trio.
- [ ] 2.8 `src/handlers/import_mt940.rs` — same trio.
- [ ] 2.9 `src/main.rs` — 9 new routes.
- [ ] 2.10 `src/handlers/import.rs::upload` — call
      `sniff::detect`, 303 to the right route.
- [ ] 2.11 Templates:
      `templates/import/{ofx,qif,mt940}_{upload,preview}.html`
      (6 files).
- [ ] 2.12 `src/templates/import_{ofx,qif,mt940}.rs` Askama
      structs.
- [ ] 2.13 Fixtures under `tests/fixtures/`.

## 3. Validation

- [ ] 3.1 `openspec validate bank-statement-imports` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: upload a real QFX file from a bank →
      preview renders rows → commit → trial balance still in
      balance.
- [ ] 3.6 `openspec archive bank-statement-imports`.
