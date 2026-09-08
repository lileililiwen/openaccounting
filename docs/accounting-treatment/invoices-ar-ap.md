# Invoices / AR/AP Treatment Record

**Workflow:** Invoice creation, AR aging, AP aging, payment matching
**Jurisdiction:** generic
**Scope:** Invoice creation, AR aging, AP aging, payment matching

## Recognition timing

Revenue is recognized when the invoice is posted (accrual basis).
Accounts receivable are created at invoice time and reduced when
payment is received. Accounts payable follow the mirror pattern.

## Posting treatment

Invoice creation debits AR (ASSET) and credits Revenue (INCOME).
Payment receipt debits Cash (ASSET) and credits AR (ASSET).
AP invoice debits Expense/Inventory and credits AP (LIABILITY).
AP payment debits AP (LIABILITY) and credits Cash (ASSET).

## Rounding

All amounts use 2-decimal-place precision matching the
base currency.

## Reversal / void

Invoice reversals follow the standard append-only model.
Voiding an invoice creates an offsetting entry; the original
is preserved in the audit log.

## Report inclusion

AR aging reports group outstanding receivables by age bucket
(current, 30, 60, 90+ days). AP aging follows the same pattern
for payables.

## Assumptions

- No automated payment reminders.
- Aging buckets are fixed: current, 30, 60, 90+ days.
- No multi-currency invoice support in aging reports.

## Non-compliance disclaimer

Invoice aging reports are informational only and NOT a
substitute for professional accounting advice. Revenue
recognition timing may differ under your accounting standards
(e.g., ASC 606, IFRS 15). Consult your auditor.
