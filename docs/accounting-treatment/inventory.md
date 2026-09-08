# Inventory Treatment Record

**Workflow:** FIFO inventory cost tracking and COGS recognition
**Jurisdiction:** generic
**Scope:** FIFO inventory cost tracking and COGS recognition

## Recognition timing

COGS is recognized at the time of sale. Inventory is reduced
and COGS is increased when a sale transaction is posted.

## Posting treatment

Purchase: Dr Inventory (ASSET), Cr AP/Cash.
Sale: Dr AR/Cash, Cr Revenue (INCOME).
COGS: Dr COGS (EXPENSE), Cr Inventory (ASSET).

## Rounding

Unit costs are computed to 4 decimal places. Total COGS is
rounded to 2 decimal places (base-currency precision).

## Reversal / void

Inventory transactions follow the standard append-only model.

## Report inclusion

Inventory balance appears on the balance sheet under Current
Assets. COGS appears on the income statement as a separate
section.

## Assumptions

- FIFO (First-In, First-Out) cost flow assumption only.
- Single-currency only; no multi-currency inventory.
- No weighted average or LIFO methods.
- No physical count reconciliation.

## Non-compliance disclaimer

Inventory valuation is FIFO only and NOT a substitute for
professional inventory accounting. Other methods (LIFO,
weighted average) are not supported. Physical count
adjustments require manual journal entries.
