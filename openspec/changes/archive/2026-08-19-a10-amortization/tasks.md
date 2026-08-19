## 1. Testing

- [x] 1.1 HTTP: `http_create_amortization_schedule`.
- [x] 1.2 Worker: `amortization_worker_posts_one_period`.
- [x] 1.3 Worker: `amortization_worker_idempotent`.
- [x] 1.4 HTTP: `http_amortization_progress_report`.
- [x] 1.5 HTTP: `http_amortization_skip_period`.
- [x] 1.6 HTTP: `http_amortization_viewer_cannot_create`.

## 2. Implementation

- [x] 2.1 `migrations/0046_add_amortization_schedules.sql`.
- [x] 2.2 `src/handlers/amortization.rs` (new + list + skip).
- [x] 2.3 `src/workers/amortization.rs` (sweep + post_one + idempotency).
- [x] 2.4 `src/reports/amortization.rs` (progress listing).
- [x] 2.5 `src/lib.rs` — routes wired.
- [x] 2.6 `templates/amortization/{new,report}.html`.
- [x] 2.7 `transactions_kind_check` widened to allow `'amortization'`.

## 3. Validation

- [x] 3.1 `openspec validate a10-amortization`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --features test-support` clean for new files.
- [x] 3.4 `cargo test --features test-support --test integration -- amortization` (6/6 pass).
- [ ] 3.5 `openspec archive a10-amortization`.
