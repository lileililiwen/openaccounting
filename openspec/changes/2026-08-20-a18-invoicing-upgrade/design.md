# Invoicing Upgrade — Design

## Approach

Add an `invoice_lines` table and rewrite the create flow to build
invoices from lines. Add a detail page, print view, and computed
overdue status. No changes to the payments engine beyond fixing a
latent query bug and accepting an `invoice_id` query param.

## Decisions

- **Line amounts are explicit, not derived.** The form has quantity and
  unit price, but the server uses the submitted per-line `amount`
  (defaulting to `quantity × unit_price` via JS). WHY: invoices
  sometimes have flat-rate lines; storing the amount avoids
  recomputation drift. Mitigation: JS auto-fills `amount` from
  qty × price and the server validates `amount > 0`.

- **Total is computed, not typed.** The `invoices.total` column is set
  to the sum of line amounts and no longer accepts a typed total.
  WHY: prevents a total that disagrees with the lines. Mitigation: the
  existing column stays; only the input changes.

- **Detail page uses the existing print stack.** The app already has a
  print button partial and `@media print` CSS; the detail page reuses
  them. WHY: no new dependency. Risk: the printed page includes the
  sidebar — the existing print CSS hides `.no-print`; the detail layout
  uses the standard main + sidebar, which the print CSS already handles.
  Mitigation: verified visually.

- **Overdue is computed, not stored.** `open` invoices with
  `due_date < today` and `amount_paid < total` render as overdue.
  WHY: avoids background jobs flipping status and keeps the value
  always correct. Mitigation: the list and detail queries compute it;
  no migration needed.

- **Mark-paid / void are simple POSTs.** Mark paid sets `status='paid'`
  (and `amount_paid = total`); void sets `status='void'`. WHY: a
  lightweight audit-friendly action for the bookkeeper. Mitigation:
  audit-logged.

## Schema Change

Migration `0049_add_invoice_lines.sql`:

```sql
CREATE TABLE IF NOT EXISTS invoice_lines (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id  UUID NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    quantity    NUMERIC(12,4) NOT NULL DEFAULT 1,
    unit_price  NUMERIC(20,4) NOT NULL DEFAULT 0,
    amount      NUMERIC(20,4) NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_invoice_lines_invoice ON invoice_lines(invoice_id);
```

## Data Flow

1. `new.html` renders a dynamic line editor (JS adds rows, computes
   amount and total).
2. `invoices::create` parses `lines[N][…]`, sums amounts, inserts the
   invoice and its lines in one DB transaction.
3. `invoices::show` loads the invoice, contact, lines, and payment
   history, computes overdue, and renders the detail page.
4. The list page computes overdue per row and links each invoice to its
   detail page.

## Risks / Trade-offs

- Existing invoices have no lines. Mitigation: the detail page shows
  "No line items" and still renders the stored total; the list is
  unaffected.
- The payments new page had a latent `SELECT number` bug against the
  invoices table. Mitigation: fixed to `invoice_number` as part of this
  change (it blocks the payment shortcut).
