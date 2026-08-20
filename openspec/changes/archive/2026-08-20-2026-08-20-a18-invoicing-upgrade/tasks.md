# Invoicing Upgrade — Tasks

## 1. Testing

- [x] 1.1 Integration test: creating an invoice with two lines stores the invoice (total = sum), both lines, and links them.
- [x] 1.2 Integration test: creating an invoice with no lines (or all-zero amounts) is rejected.
- [x] 1.3 Integration test: the detail page shows lines, totals, amount paid, and outstanding.
- [x] 1.4 Integration test: an open invoice past due shows "overdue" on the list and detail; a non-due invoice does not.
- [x] 1.5 Integration test: marking an invoice paid sets status paid; voiding sets void (audit logged).
- [x] 1.6 Integration test: the payments new page with `?invoice_id=` preselects the invoice (and the fixed `invoice_number` query works).
- [x] 1.7 Integration test: the invoice list links each invoice to its detail page.

## 2. Implementation

- [x] 2.1 Migration `0049_add_invoice_lines` (+ `.down.sql`), applied.
- [x] 2.2 Rewrite `invoices::create` to parse `lines[N][description|quantity|unit_price|amount]`, sum totals, and insert invoice + lines atomically.
- [x] 2.3 Add line rows + JS (add/remove row, qty×price autofill, running total) to `invoices/new.html`.
- [x] 2.4 Add `invoices::show` handler + `templates/invoices/show.html` (lines, totals, status, overdue, print button, actions) + `/invoices/{id}` route.
- [x] 2.5 Add `mark_paid` and `void` POST handlers + routes + audit logs.
- [x] 2.6 Compute overdue on the list query and link rows to the detail page.
- [x] 2.7 Fix the payments `SELECT number` bug → `invoice_number`; accept `?invoice_id=` on the payments new page to preselect.
- [x] 2.8 Add the necessary template structs (`InvoiceShow`, line rows) and keep `InvoiceNew` form data preserved on error.

## 3. Documentation

- [x] 3.1 Mention invoice line items and the detail page in the README.
