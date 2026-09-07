# Getting Paid: Quotes, Recurring Invoices, Share Links, E-Invoicing

## Why

Invoicing exists but stops at "create, mark paid, void"
(`src/handlers/invoices.rs`). The commercial baseline (research top-30,
items 8–10, 14–16) expects: estimates/quotes that convert to invoices
(all nine products), recurring invoices (all nine), a client-facing
payment path — payment links or portal (eight of nine) — and automatic
overdue reminders. Separately, statutory e-invoicing is arriving by
mandate: Germany (B2B e-invoicing since 2025), Belgium PEPPOL 2026,
France 2026-2027; Invoice Ninja's PEPPOL/ZUGFeRD/FatturaPA support is
its sharpest differentiator among OSS, and Zoho/Holded compete on it.
openaccounting has neither quotes nor any structured invoice output.

## What Changes

- Estimates/quotes: draft documents with line items, client-facing
  accept/decline link, one-click convert-to-invoice.
- Recurring invoices driven by the scheduler (`automation-platform`
  change): template → auto-issue on schedule.
- Public share links per invoice (tokenized URL): read-only view with
  "mark as paid" instructions and optional payment-reference QR;
  no login required, revocable.
- Overdue reminder events consumed from `invoice.overdue` (scheduler).
- Factur-X (ZUGFeRD 2.x) XML embedded in the printable invoice PDF and
  standalone UBL (CII/UBL 2.1) export per invoice — the data model and
  export first; PEPPOL transport later.

## Capabilities

### New Capabilities

- `invoicing-completeness`: quotes→invoice conversion, recurring
  issuance, public share links, overdue reminders.
- `e-invoicing-facturx`: structured e-invoice embedding/export.

## Impact

**New files:** `src/handlers/estimates.rs`, `src/handlers/invoice_share.rs`,
`src/einvoice/{facturx,ubl}.rs`, `migrations/00xx_estimates.sql`,
`00xx_invoice_shares.sql`.
**Modified:** invoices handler/templates (share button, recurrence),
printable views (embedded XML), scheduler job kinds.
