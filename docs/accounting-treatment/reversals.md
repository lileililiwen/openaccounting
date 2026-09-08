# Reversals Treatment Record

**Workflow:** Transaction voiding and reversal with audit trail
**Jurisdiction:** generic
**Scope:** Transaction voiding and reversal with audit trail

## Recognition timing

Reversals are applied immediately. A reversal creates a new
transaction with the opposite postings, dated on the reversal
date (not the original transaction date).

## Posting treatment

A reversal creates a new transaction with all postings
inverted (DEBIT becomes CREDIT and vice versa). The original
transaction is preserved unchanged. Both appear in the
general ledger and trial balance.

## Rounding

Reversal amounts match the original transaction exactly;
no additional rounding is applied.

## Reversal / void

Reversals are themselves append-only: they cannot be deleted
or modified after creation. The original transaction is
preserved in the audit log with a hash chain entry.

## Report inclusion

Reversals affect reports as normal transactions. The trial
balance reflects the net effect (original + reversal = zero).
Reports can be filtered to exclude reversed transactions
using the kind column.

## Assumptions

- Reversals create new offsetting transactions.
- Original transactions are preserved in the audit log.
- No partial reversals (full transaction reversal only).

## Non-compliance disclaimer

Reversal behavior follows the append-only model and is NOT
a replacement for formal audit controls. Consult your
auditor to confirm this meets your jurisdiction's
record-keeping requirements.
