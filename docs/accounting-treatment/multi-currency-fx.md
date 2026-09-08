# Multi-Currency / FX Treatment Record

**Workflow:** Currency conversion and realized/unrealized FX gains/losses
**Jurisdiction:** generic
**Scope:** Currency conversion and realized/unrealized FX gain/loss

## Recognition timing

Realized FX gains/losses are recognized at the time of
settlement (when a foreign-currency transaction is paid or
received in the base currency). Unrealized FX gains/losses
are computed at period-end for open foreign-currency balances.

## Posting treatment

FX gains post to an INCOME account (subtype NON_OPERATING_INCOME).
FX losses post to an EXPENSE account (subtype NON_OPERATING_EXPENSE).
The gain/loss amount is the difference between the historical
rate at transaction time and the settlement/period-end rate,
multiplied by the foreign-currency amount.

## Rounding

Base-currency amounts are rounded to 2 decimal places.
Exchange rates are stored at full decimal precision.

## Reversal / void

FX adjustments follow the standard append-only reversal model.

## Report inclusion

Realized FX gains/losses appear in the income statement under
"Non-Operating Income (Expense)." Unrealized gains/losses
appear in the balance sheet as adjustments to foreign-currency
asset/liability balances.

## Assumptions

- Exchange rates are manually entered; no live rate feed.
- Rounding to 2 decimal places for base-currency amounts.
- No hedge accounting or forward contract support.

## Non-compliance disclaimer

FX calculations are simplified and may NOT match your
accounting software's treatment. Verify with your auditor.
Real exchange-rate behavior involves bid-ask spreads,
settlement dates, and regulatory requirements not covered
by this software.
