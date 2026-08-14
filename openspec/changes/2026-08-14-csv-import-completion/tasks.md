# CSV Import Completion — Tasks

## 1. Testing

- [ ] 1.1 Integration: `http_csv_import_creates_n_transactions`
      — 12 rows → 12 transactions, each with 2 postings.
- [ ] 1.2 Integration: `http_csv_import_rolls_back_on_bad_date`
      — 1 bad + 9 good → 0 committed, 422.
- [ ] 1.3 Integration: `http_csv_import_rejects_both_debit_and_credit`
      — 422.
- [ ] 1.4 Integration: `http_csv_import_rejects_negative_amount`
      — 422.
- [ ] 1.5 Integration: `http_csv_import_skip_duplicates` —
      dedup fingerprint excludes the second row.
- [ ] 1.6 Integration: `http_csv_import_preview_shows_500_rows`
      — assert preview HTML count.
- [ ] 1.7 Integration: `http_csv_import_unresolved_account_falls_back`
      — empty `account` column → uses `default_account_id`.
- [ ] 1.8 Property: `prop_csv_import_round_trips_balanced_books`
      — random 100-row batches leave trial balance intact.

## 2. Implementation

- [ ] 2.1 Refactor `confirm` handler in
      `src/handlers/import.rs` to read the multipart form,
      re-parse the uploaded file from the temp file, and call
      `insert_one` per row.
- [ ] 2.2 Raise the preview cap from 10 to 10,000 in
      `src/handlers/import.rs::upload`.
- [ ] 2.3 Add `insert_one` helper (private to the handler
      module) per the design doc.
- [ ] 2.4 Extend `ImportConfirmForm` to carry
      `default_account_id` and a stable upload token (the
      multipart field `temp_path` written by axum's
      tempfile).
- [ ] 2.5 Update `templates/import/preview.html`:
      - per-row account selector (dropdown of all ledger EXPENSE
        accounts),
      - yellow background on duplicate rows,
      - "default account" picker at the top,
      - "Showing the first 10,000 rows" warning when truncated.
- [ ] 2.6 Add `import.generic.commit.success` and
      `import.generic.commit.failed` to the audit
      `event_type` whitelist.
- [ ] 2.7 Add `tests/integration/csv_import.rs` (8 tests above).

## 3. Validation

- [ ] 3.1 `openspec validate csv-import-completion` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: upload a 25-row CSV via the UI,
      preview shows all 25, commit, assert 25 transactions in
      the list and trial balance still in balance.
- [ ] 3.6 `openspec archive csv-import-completion`.
