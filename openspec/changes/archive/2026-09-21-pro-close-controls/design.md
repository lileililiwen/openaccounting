## Context

Current close is a one-time transfer to retained earnings plus an optional append-only flag. There is no period table, no override path, and no approval state on journals. Roles are coarse. Invoice identity piggybacks on `YYYY-NNNNNN` transaction numbers with no void audit.

## Goals / Non-Goals

**Goals:**
- Closed periods reject all writes (transactions, edits, reversals, imports, revaluations).
- Every exception is attributable: who, when, reason.
- Large journals need a second pair of eyes.

**Non-Goals:**
- Jurisdiction-specific close checklists (deferred to compliance-exports).
- Payroll approval chains (no payroll engine exists).

## Decisions

- **New `closed_periods(ledger_id, closed_through, closed_by, closed_at)` table.** WHY: a single date watermark is queryable in every write path with one indexed lookup, unlike scanning closing transactions.
- **Override = new audit row + `reopen_events` record with mandatory reason, admin-only.** WHY: keeps the ledger immutable while making exceptions visible; matches auditor expectations of logged reopens.
- **Maker-checker via `journal_approvals(txn_id, maker, checker, status)` for amounts ≥ ledger threshold (default 10,000 base).** WHY: state machine on the transaction avoids a parallel pending-ledger; checker must differ from maker enforced in handler.
- **Roles extend the existing ledger-share enum, not a new RBAC framework.** WHY: smallest change consistent with `role-enforcement`; `accountant` = editor minus close/share, `auditor` = viewer plus export.
- **Separate `invoice_sequences(ledger_id, year, last_no)` with `invoices.number` NOT NULL UNIQUE per ledger.** WHY: transaction numbers can have gaps from drafts/deletes; invoice law in most jurisdictions cannot.

## Risks / Trade-offs

- Write-path hot spot on close check → Mitigation: single indexed `closed_periods` lookup cached per request.
- Maker-checker friction for solo users → Mitigation: threshold is per-ledger configurable, default-off for personal ledgers.
- Voids break gaplessness perception → Mitigation: voids keep their number with reason; gap report shows voids vs true gaps.
