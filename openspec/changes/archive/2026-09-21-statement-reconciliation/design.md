## Context

`src/handlers/reconciliation.rs` (~510 lines) handles matching and rule application. No session, balance, or lock concept exists. Statement parsers live in `src/import/` for OFX/QIF/MT940/CSV plus WeChat/Alipay handlers.

## Goals / Non-Goals

**Goals:**
- Every reconciled statement ties out to an external closing balance.
- Cleared state is per transaction leg, reversible only with reason.

**Non-Goals:**
- Fully automatic reconciliation without user confirm (auto-clear stays opt-in per session).
- Building a certified EU open-banking connector in this change (trait shape + one documented mapping only).

## Decisions

- **New `rec_sessions(account_id, stmt_close_date, stmt_close_balance, status)` plus `rec_lines(session_id, txn_id, cleared)` tables.** WHY: separates the external statement assertion from internal cleared flags; re-running a period never rewrites history.
- **Finish requires difference == 0 exactly (Decimal comparison).** WHY: accountants reconcile to the cent; tolerance windows hide errors and break the audit story.
- **Lock = session status `closed` blocks unclear of its lines except via unreconcile-with-reason.** WHY: reuses the period-hard-close override pattern without coupling the two features.
- **CAMT.053 via existing XML parsing stack, QBO via OFX-variant parser extension.** WHY: no new crates; QBO is OFX with Intuit headers so the OFX path extends cleanly. Alternative considered: new `camt` crate — rejected: adds native deps for one format.
- **EU aggregator as trait documentation + config shape, not a live integration.** WHY: live credentials cannot be tested in CI; trait shape unblocks a follow-up change.

## Risks / Trade-offs

- Large statements (10k+ lines) slow session load → Mitigation: paginate uncleared lines, index on (account_id, date).
- CAMT.053 namespace variants across banks → Mitigation: accept both pain.002-adjacent namespaces found in fixtures; reject unknown with file+line error.
- Plaid vs EU field mapping drift → Mitigation: normalize to (date, amount, payee, reference) at provider boundary with provider tag preserved.
