# Proposal: Professional period close and financial controls

## Why

Closing entries (`0005_*`) and the append-only toggle (`0044_*`) exist, but there is no hard-close with override audit, no maker-checker separation of duties, and only owner/editor/viewer roles. Invoice numbers reuse generic transaction numbers with no gap detection. Auditors cannot accept this for professional books: closed periods must block writes, reopens must be logged with reason, large journals need a second approver, and invoice sequences must be gapless with void reasons.

## What Changes

- Hard-close periods per ledger with effective date; writes to closed dates are rejected.
- Reopen/override flow requiring admin role plus mandatory reason, fully audit-logged.
- Maker-checker: journals above a ledger threshold enter pending state until a different user approves.
- New ledger roles `accountant` (write, no close) and `auditor` (read + export only).
- Gapless per-ledger-per-year invoice numbering with void reason tracking.

## Capabilities

### New Capabilities
- `period-hard-close`: hard close, reopen override, maker-checker, accountant/auditor roles, gapless invoice numbering.

### Modified Capabilities
- `transaction-edit-void`: reopens and edits in closed periods are blocked except via override flow.
- `transaction-numbering`: invoice sequence is tracked separately from transaction numbers with gap detection.
- `role-enforcement`: adds `accountant` and `auditor` ledger roles to the existing owner/editor/viewer rules.
- `audit-chain`: close, reopen, override, and approval events are hash-chained audit rows.

## Impact

Affected: `src/handlers/closing.rs`, `src/handlers/sharing.rs`, `src/domain/transaction.rs`, invoice handlers, `migrations/00xx_*` (`closed_periods`, `journal_approvals`, `invoice_sequences`), close/reopen templates. Unaffected: report math, FX, OCR, bank-feed providers, PWA shell.
