# Cash-Basis Reporting in OpenAccounting

OpenAccounting supports two reporting bases, configured per
ledger:

| Basis    | What counts as income / expense                                  |
|----------|-----------------------------------------------------------------|
| `accrual`| The date of the posting. (Default.)                             |
| `cash`   | Only postings whose peer leg is a cash or bank account.         |

The basis is set at ledger creation and can be flipped by the
ledger owner later. Reports (`/ledgers/{id}/reports`) accept a
`?basis=...` override; absent that, the ledger's stored basis is
used.

## What cash basis does NOT do

The cash-basis toggle is a **read-time filter** on reports. It
does **not** rewrite, re-date, or delete postings. The underlying
double-entry data is unchanged, so flipping back to accrual shows
the full history.

What this means for the common "I got a $1,200 invoice for a
year of service, what do I book?" question:

* **Wrong on accrual, wrong on cash:** book the full $1,200 to
  revenue the day you receive it.
* **Right on accrual, partially right on cash:** book the $1,200
  to a DeferredRevenue account on receipt, then recognize $100 /
  month into Income. The cash filter then ignores the original
  $1,200 entry (no cash leg) and counts each $100 recognition
  (peer leg is the bank account you booked the $1,200 into).

## Recognized accounts

The convention is:

* **DeferredRevenue** — `subtype = 'CurrentLiability'`, e.g.
  code `2400`. Holds revenue you have invoiced / received but
  have not yet earned.
* **PrepaidExpense** — `subtype = 'CurrentAsset'`, e.g.
  code `1400`. Holds expenses paid in advance (annual SaaS
  subscriptions, insurance, rent).

Both are present in the seeded chart of accounts when the
subtype `CurrentLiability` / `CurrentAsset` is matched — you do
not need extra migrations. Add the accounts in your chart of
accounts UI if your default COA skipped them.

## Recognizing revenue: one click

1. Book the initial receipt as
   `Dr Bank 1,200 / Cr DeferredRevenue 1,200` on receipt.
2. Open the deferred-revenue line. The "Recognize" action
   creates a `transaction_templates` row with two postings
   (`Dr DeferredRevenue` / `Cr Revenue`) and a recurring
   frequency (default monthly).
3. The recurring worker (`process_due` in `templates.rs`)
   posts one balanced transaction per period on the schedule.

The total amount and the per-period amount are independent:

* Total amount = how much you deferred initially.
* Per-period amount = how much to recognize each month.

The schedule advances `next_date` by the chosen frequency and
keeps firing until the template is deactivated.

## Recognizing prepaid expense: symmetric

Same flow, opposite sign. The button writes
`Dr Expense / Cr PrepaidExpense` instead.

## Example: 12 months × $100 SaaS subscription, received Jan 1

```text
Jan 1   Dr Bank 1,200  Cr DeferredRevenue 1,200
Feb 1   Dr DeferredRevenue 100  Cr Sales Revenue 100   ← recognition
Mar 1   Dr DeferredRevenue 100  Cr Sales Revenue 100
…
Dec 1   Dr DeferredRevenue 100  Cr Sales Revenue 100
```

On a cash basis the Jan 1 booking is **excluded** from the P&L
(no cash leg in the underlying posting chain is relevant — the
cash leg is the bank account, but the counter-leg is a liability,
not revenue). Each recognition is **included** because the peer
leg is the bank account.

## See also

- `migrations/0020_add_ledger_basis.sql` — original toggle.
- `migrations/0009_add_recurring_transactions.sql` — template
  infrastructure reused for recognition schedules.
- `src/reports/income_statement.rs` — implementation of the
  cash-basis filter.
- `openspec/changes/a9-cash-basis-docs/specs/cash-basis-docs/spec.md`
- `openspec/specs/reports/spec.md` — basis semantics for reports.