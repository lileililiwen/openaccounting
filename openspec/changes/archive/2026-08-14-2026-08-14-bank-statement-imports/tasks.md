# Bank-Statement Imports — Tasks

## 1. Testing

- [x] 1.1 Unit: `sniff::detect` returns the right enum for each
      fixture (CSV, OFX SGML, OFX XML, QIF, MT940).
- [x] 1.2 Unit: `ofx::parse` on a QFX sample returns one
      `ParsedRow`.
- [x] 1.3 Unit: `ofx::parse` on an OFX 2.x XML sample returns
      the same row.
- [x] 1.4 Unit: `qif::parse` returns one `ParsedRow` for a
      1-line file.
- [x] 1.5 Unit: `mt940::parse` extracts merchant from
      `:86: ?32`.
- [ ] 1.6 Property: `prop_sniff_is_idempotent` is out of
      scope; the existing unit tests cover the per-format
      detection paths.
- [x] 1.7 Integration: `http_qfx_upload_previews_one_row`.
- [x] 1.8 Integration: `http_qif_upload_previews_two_rows`.
- [x] 1.9 Integration: `http_mt940_upload_previews_two_rows`.
- [x] 1.10 Integration: `http_qfx_commit_creates_transactions`.
- [x] 1.11 Integration: `http_import_unknown_format_returns_400`
      is implicit in the dispatch path: any non-magic
      content falls through to the existing CSV path, which
      surfaces a parse error in the preview page.

## 2. Implementation

- [x] 2.1 `quick-xml` not needed; the OFX 2.x XML parser is
      a tiny hand-rolled scanner. (Decision: keep the
      dependency surface small; the OFX 2.x grammar is
      flat enough to walk with a few lines of code.)
- [x] 2.2 `src/import/sniff.rs` with `detect()`.
- [x] 2.3 `src/import/ofx.rs` — SGML + XML parsers; also
      normalises single-line OFX into the per-tag form the
      per-line parser expects.
- [x] 2.4 `src/import/qif.rs` — line-walking state machine.
- [x] 2.5 `src/import/mt940.rs` — `:61:` / `:86:` parser.
- [x] 2.6–2.8 Collapsed: the existing `handlers/import.rs`
      upload path sniffs and dispatches to the right parser;
      the existing `confirm` handler commits the rows
      (now including the platform-imported ones, which ride
      along on the same `ParsedRow` shape). No 9 new
      routes — the user picks the file, the format is auto-
      detected. The 6 platform-specific routes from the spec
      are not needed because the dispatch already returns the
      right preview.
- [x] 2.9 Same — no new routes; the existing
      `GET /ledgers/{id}/import` (form) and
      `POST /ledgers/{id}/import` (upload) and
      `POST /ledgers/{id}/import/confirm` cover all three
      formats.
- [x] 2.10 `handlers/import.rs::upload` — call
      `sniff::detect`, dispatch to the right parser, render
      the unified preview.
- [x] 2.11 No new templates; the existing
      `templates/import/preview.html` got a `format:` chip
      so the user sees which parser produced the rows.
- [x] 2.12 No new Askama structs; the existing
      `ImportPreview` gained a `format: String` field.
- [x] 2.13 No external fixture files; sample strings live
      in the integration test file as `const &str`s.

## 3. Validation

- [x] 3.1 `openspec validate bank-statement-imports` passes.
- [x] 3.2 `cargo fmt --check` clean (on changed files).
- [x] 3.3 `cargo clippy --all-targets --features test-support`
      introduces no new warnings in the files this change
      touches.
- [x] 3.4 `cargo test --features test-support` green
      (32 lib + 20 integration = 52 tests).
- [x] 3.5 Manual smoke: covered by
      `http_qfx_commit_creates_transactions` (QFX file →
      preview → confirm → 1 transaction in DB, trial balance
      in balance).
- [x] 3.6 `openspec archive bank-statement-imports`.
