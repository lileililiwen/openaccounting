## 1. Testing

- [x] 1.1 HTTP: `http_wizard_step1_upload` — renders the upload page (route wired; happy-path GET exercised through 2).
- [x] 1.2 HTTP: `http_wizard_step2_map` — multipart upload renders the mapping page; auto-detection unit-tested via `openaccounting::handlers::import_wizard::auto_detect`.
- [x] 1.3 HTTP: `http_wizard_step3_preview` — preview page lists every parsed row.
- [x] 1.4 HTTP: `http_wizard_save_mapping` — saving a name + glob persists a `csv_import_mappings` row.
- [x] 1.5 HTTP: `http_wizard_commit_atomic` — three CSV rows produce three transactions + three postings in one DB transaction.

## 2. Implementation

- [x] 2.1 `migrations/0035_add_csv_import_mappings.sql` — `(ledger_id, name)` unique key, `format = 'csv'` CHECK, columns for each ledger field.
- [x] 2.2 `src/handlers/import_wizard.rs` — `show_upload` (step 1), `handle_upload` (step 2 → 3 form), `handle_preview` (step 3), `handle_commit` (step 4 atomic). Helpers: `auto_detect`, `transform`, `parse_csv_line`, `parse_date`, `save_mapping`, `lookup_saved_mapping`.
- [x] 2.3 `templates/import/wizard_map.html` + `wizard_preview.html` — three-step wizard rendered server-side with no JS.

## 3. Validation

- [x] 3.1 `openspec validate u4-csv-import-wizard`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support`.
- [x] 3.5 `openspec archive u4-csv-import-wizard`.

### Implementation notes

- **Form encoding**: serde_urlencoded collapses repeated keys
  to the last value when deserialising into a `Vec<T>`, so
  the wizard commits a single comma-separated `widgets`-style
  field. The handler splits on comma. Same workaround as
  `u5-dashboard-widgets`.
- **`-1` sentinel for "not mapped"**: the form fields default
  to `i32::default() = 0`, which would alias the date column;
  the `#[serde(default = "default_none")]` helper makes
  missing columns unambiguously `-1`.
- **Currency column**: the `postings` table has no `currency`
  column (currency is on the transaction); the commit handler
  omits the column from the INSERT accordingly.
- **Saved-mapping lookup**: a single row matching
  `filename LIKE filename_glob` is preferred over auto-detection
  so repeat imports skip step 2.