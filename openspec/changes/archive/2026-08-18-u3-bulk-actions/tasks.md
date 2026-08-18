## 1. Testing

- [x] 1.1 HTTP: `http_bulk_tag` — adds the tag to every selected row; idempotent (re-running doesn't duplicate).
- [x] 1.2 HTTP: `http_bulk_untag` — removes the tag from every selected row.
- [x] 1.3 HTTP: `http_bulk_contact` — sets `contact_id` on every selected row.
- [x] 1.4 HTTP: `http_bulk_delete_uses_reversals` — three originals preserved; three reversing transactions created.
- [x] 1.5 HTTP: `http_bulk_limit_422` — 501 rows returns 422.

## 2. Implementation

- [x] 2.1 `src/handlers/transactions_bulk.rs` — single `POST /ledgers/{id}/transactions/bulk` route with four actions (`tag`, `untag`, `contact`, `delete`). `MAX_BULK = 500` enforced.
- [x] 2.2 `templates/partials/_bulk_bar.html` — fixed-bottom bar with action picker + value input. Wired into `templates/transactions/list.html` with per-row checkboxes and a header select-all.
- [x] 2.3 Audit log row for each bulk action — `audit_entries` row written with action (`bulk_tag` / `bulk_untag` / `bulk_contact` / `bulk_delete`), selected count, and the side-effect count.

## 3. Validation

- [x] 3.1 `openspec validate u3-bulk-actions`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support`.
- [x] 3.5 `openspec archive u3-bulk-actions`.

### Implementation notes

- The bulk bar submits a single comma-separated `txn_ids`
  field (the same workaround as `u5-dashboard-widgets` and
  `u4-csv-import-wizard`) because serde_urlencoded collapses
  repeated keys.
- Bulk delete is non-destructive: every selected row gets a
  companion row in `transactions` with `kind = 'reversing'`
  and the postings' debit/credit flipped. The originals are
  preserved so the audit trail stays intact.
- Tag attachment is idempotent via `INSERT ... ON CONFLICT DO
  NOTHING`, so users can re-submit without growing the table.
- The `payee` / `reference` columns are nullable; the SELECT
  in `delete_action` decodes them as `Option<String>` to avoid
  the 500 we'd otherwise hit when seed rows omit them.