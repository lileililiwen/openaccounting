# Proposal: Full statement reconciliation workflow

## Why

Reconciliation today is a line-level match helper plus categorize/flag rules (`0011_*`, `reconciliation-rules`, payee learning). There is no statement-level workflow: no opening/closing balances, no cleared-vs-uncleared tracking, no difference-must-be-zero gate, no lock after reconcile. Imports cover OFX/QIF/MT940/CSV/WeChat/Alipay but not CAMT.053 or QBO, and bank feeds are Plaid-only. Professional books require provable statement agreement per account per period.

## What Changes

- Statement reconciliation sessions per account: opening balance, statement closing balance/date, cleared marking, computed difference, zero-gate to finish, lock afterwards.
- Unreconcile with reason (audit-logged) for corrections.
- CAMT.053 and QBO import support in the statement importer.
- Provider abstraction second implementation path documented for EU open banking (GoCardless/Enable Banking shape) behind the existing Provider trait.

## Capabilities

### New Capabilities
- `statement-reconciliation`: statement sessions, cleared tracking, zero-gate close, lock, unreconcile audit.

### Modified Capabilities
- `reconciliation-rules`: rules MAY auto-suggest cleared candidates inside a session; auto-clear requires explicit user confirm.
- `data-import`: statement importer accepts CAMT.053 and QBO in addition to existing formats.
- `bank-feeds`: provider trait gains a second documented integration shape for EU open-banking aggregators.

## Impact

Affected: `src/handlers/reconciliation.rs`, `src/domain/reconciliation_rules.rs`, `src/import/*`, bank-feed provider impls, reconcile templates. Unaffected: posting invariant, reports math, auth, notifications.
