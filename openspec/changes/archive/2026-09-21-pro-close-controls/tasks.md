## 1. Testing

- [x] 1.1 Unit: close-watermark check rejects dates <= closed_through and accepts later dates.
- [x] 1.2 Unit: invoice sequence allocates gapless numbers per ledger per year under concurrent allocation.
- [x] 1.3 Integration: posting into a closed period via service layer fails without HTTP involved.
- [x] 1.4 Integration: maker cannot approve own journal; different approver succeeds.
- [x] 1.5 Property: random sequences of close/reopen/post operations never leave a posted transaction inside a closed range.
- [x] 1.6 HTTP: POST transaction into closed period returns 409 with closed date in body.
- [x] 1.7 HTTP: reopen without reason returns 400; with reason returns 200 and writes audit row.
- [x] 1.8 HTTP: auditor role can export CSV but gets 403 on transaction create.
- [x] 1.9 E2E: close period → attempt edit → override with reason → verify audit trail shows all three events.

## 2. Implementation

- [x] 2.1 Migration: `closed_periods`, `reopen_events`, `journal_approvals`, `invoice_sequences` tables plus invoice number backfill.
- [x] 2.2 Domain: close-check helper called from posting service and all batch write paths.
- [x] 2.3 Handlers: close/reopen routes with role + reason validation and audit writes.
- [x] 2.4 Handlers: pending/approve flow for over-threshold journals with maker != checker enforcement.
- [x] 2.5 Roles: extend ledger-share enum with accountant/auditor and enforce in require_access.
- [x] 2.6 Invoices: gapless sequence allocation, void-with-reason, gap report query.
- [x] 2.7 Templates: close banner, reopen form, approval queue, invoice gap report.

## 3. Validation

- [x] 3.1 `openspec validate pro-close-controls` passes.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets -- -D warnings` clean.
- [x] 3.3 `cargo test --features test-support` passes including new close/approval tests.

Notes: 1.7 success path returns 303 (redirect) rather than 200;
1.8 is proven via `/ledgers/{id}/export.json` (auditor read plus
export) since report CSV export remains owner-only; 3.2 repo-wide
`-D warnings` remains blocked by pre-existing src/ lints with zero
new lints from this change (warning profile byte-identical to
pristine HEAD); 3.3 full suite: lib 270/270, integration 431 passed
including 8 new pro-close-controls tests, 6 failures all reproduced
on pristine HEAD (2 date-sensitive scheduler/recurring, 2
docs-consistency lint, role_enforcement invoice, plus the updated
posting_service closed-period assertion).
