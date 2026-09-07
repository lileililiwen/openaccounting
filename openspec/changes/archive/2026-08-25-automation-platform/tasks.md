# 1. Testing

- [x] 1.1 Unit: job claim uses SKIP LOCKED — two concurrent claims, one winner (spawn two pools in test).
- [x] 1.2 Unit: backoff schedule 30s/2m/10m/1h/6h for attempts 1..5; dead after max.
- [x] 1.3 Integration: monthly template due twice → exactly one posted transaction per due date (scheduler + manual route both).
- [x] 1.4 Integration: invoice 8 days overdue → reminder events only at offsets 1 and 7.
- [x] 1.5 HTTP: subscription CRUD owner-only; viewer 403; non-HTTPS target rejected.
- [x] 1.6 Integration: `invoice.paid` event → delivery signed with subscription secret; HMAC verifies over raw body.
- [x] 1.7 Integration: receiver 500×2 then 200 → three attempt rows, status delivered.
- [x] 1.8 Unit: email renderer produces locale-correct digest from fixture ledger; missing SMTP marks channel skipped.
- [x] 1.9 Property: 200 random event/subscription matrices enqueue exactly the filtered set.

# 2. Implementation

- [x] 2.1 Migration: `jobs`, `webhook_subscriptions`, `webhook_deliveries`; unique index for template idempotency.
- [x] 2.2 `src/jobs/`: queue, runner, backoff; spawn in worker bootstrap.
- [x] 2.3 Template-run job handler; rewire `/templates/process_due`.
- [x] 2.4 Invoice reminder job + per-ledger toggle.
- [x] 2.5 `src/notifications/email.rs` (lettre) + `http.rs` channel; preference wiring on `/account/notifications`.
- [x] 2.6 Weekly digest job + templates.
- [x] 2.7 Event emission points in posting/invoice/budget services.
- [x] 2.8 `src/handlers/webhooks_out.rs`: subscriptions UI + replay action; purge job for old deliveries.

# 3. Validation

- [x] 3.1 `openspec validate automation-platform`.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets --features test-support`.
- [x] 3.3 `cargo test --features test-support`.
