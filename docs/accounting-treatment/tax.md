# Tax Treatment Record

**Workflow:** Tax expense recording
**Jurisdiction:** generic (no jurisdiction-specific rules)
**Scope:** Tax expense recording only; no automatic tax calculation

## Recognition timing

Tax expense is recognized when the underlying income or expense
transaction is posted. No deferred tax or temporary difference
tracking is implemented.

## Posting treatment

Tax is recorded as a separate EXPENSE account (subtype
TAX_EXPENSE) with a debit entry. The corresponding credit is
to a LIABILITY account (TAX_PAYABLE) or directly to CASH.

## Rounding

Tax amounts are rounded to 2 decimal places (the base-currency
precision). No per-line rounding adjustments are applied.

## Reversal / void

Tax transactions follow the standard append-only reversal model.
Reversals create offsetting entries; the original is preserved
in the audit log.

## Report inclusion

Tax expense appears in the income statement under "Income Tax
Expense" when the account subtype is TAX_EXPENSE.

## Assumptions

- Tax rates are manually configured per jurisdiction; no
  real-time tax API integration.
- No jurisdiction-specific tax rules are automated.
- Sales tax, VAT, and income tax are all treated the same way
  (manual posting to a tax expense account).

## Non-compliance disclaimer

Passing tests does NOT establish jurisdiction-specific tax
compliance. Consult a qualified tax professional for your
jurisdiction. This software does not file tax returns or
calculate jurisdiction-specific tax obligations.
