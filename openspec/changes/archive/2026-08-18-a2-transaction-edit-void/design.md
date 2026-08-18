# ## Context

The DB trigger `check_posting_balance` makes a 1-leg posting impossible to
insert directly. Editing an existing transaction would either need to
DELETE + INSERT (loses the original) or UPDATE in place (audit hole). The
reversing-entry pattern is the canonical accounting solution: keep the
original intact, append a new dated transaction that negates it.

## Goals / Non-Goals

**Goals:**
- Non-destructive edits, full audit.
- Works for any transaction, regardless of how it was created.

**Non-Goals:**
- True in-place editing (we explicitly reject this for audit integrity).
- Edit-of-edit: two reversals would cancel, which is allowed.

## Decisions

- A new `transactions.reverses_id UUID REFERENCES transactions(id)` column.
- Reversal creates a new transaction with negated signed_amounts and
  `reverses_id = original.id`.
- Edit creates two transactions (a reversal of the original + the corrected
  new one) in a single `BEGIN ... COMMIT` block.

## Risks / Trade-offs

- Reports must filter pairs out for net views; the GL shows both with a
  reversal marker. Most users will see the net.
- Race condition: two simultaneous edits create two reversals. We lock
  the original row with `SELECT ... FOR UPDATE`.
