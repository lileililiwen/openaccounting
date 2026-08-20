# Invoicing Upgrade

## Why

Invoices today are a single line: pick a contact, type, dates, and one
total amount. A freelancer or startup building a real invoice needs
line items ("Consulting, 10h × $150"), a per-invoice view showing what
was billed, what has been paid, and what is still owed, a way to print
the invoice for the customer, and a clear overdue flag when payment is
late. There is also no detail page at all — the list is the only view.

## What Changes

- **Line items**: the invoice form accepts multiple lines (description,
  quantity, unit price, amount). The invoice total is computed from the
  lines; a new `invoice_lines` table stores them.
- **Invoice detail page** (`/invoices/{id}`): shows the contact, dates,
  kind, each line, totals, amount paid, outstanding balance, and status,
  plus a print button. Actions to mark the invoice paid or void it.
- **Overdue flag**: on the list and detail page, an open invoice past
  its due date with an unpaid balance is shown as overdue (computed, not
  stored).
- **Payment shortcut**: the payments form can be pre-opened for a
  specific invoice (`/payments/new?invoice_id=…`).

## Capabilities

- `invoice-line-items`: multi-line invoices with computed totals.
- `invoice-detail`: a printable per-invoice view with status and actions.
- `invoice-overdue`: computed overdue flag on list and detail.

## Non-Goals

- Emailing invoices or a payment link for the customer (future).
- A binary PDF generator (the print stylesheet already produces a
  print-ready page the browser can save as PDF).
- Invoice branding/templates and per-entity invoice sequences.
