# invoicing-completeness Specification

## Purpose
TBD - created by archiving change ar-getting-paid. Update Purpose after archive.
## Requirements
### Requirement: Estimates and Conversion

Users SHALL create estimates (quotes) with the same line-item model as
invoices, in status `draft | sent | accepted | declined | expired |
converted`. `POST /estimates/{id}/convert` SHALL create an invoice
copying lines, contact, tax rates, and currency; mark the estimate
`converted`; and link both rows. Converting twice MUST return 409.
Accepted/declined state MAY be set through the public share link.

#### Scenario: Quote becomes invoice

- **WHEN** an accepted estimate with two lines is converted
- **THEN** a new invoice exists with identical line amounts and the
  estimate shows `converted` with a link to the invoice.

#### Scenario: Double conversion blocked

- **WHEN** convert is called on an already-converted estimate
- **THEN** the response is 409 and no second invoice is created.

### Requirement: Recurring Invoices

A recurring-invoice template (schedule, line items, contact) SHALL be
processed by the scheduler: at each occurrence it issues one invoice
(numbered by the existing transaction-numbering sequence), posts the
AR leg, and enqueues delivery of the share link if enabled. Occurrence
idempotency SHALL follow the scheduler's `(template_id, due_date)`
rule.

#### Scenario: Monthly subscription invoices itself

- **WHEN** a monthly recurring template is due 2026-09-01 and the
  scheduler runs daily
- **THEN** exactly one invoice dated 2026-09-01 exists for it.

### Requirement: Public Share Links

Each invoice/estimate SHALL support generating a share URL containing
a 256-bit random token (`/share/invoice/{token}`). The page SHALL
render the document read-only in the recipient's locale-neutral form,
show accept/decline for estimates, and work without login. Owners
SHALL be able to revoke a link; revoked tokens return 410. Tokens are
never enumerable and links are not indexed (`noindex`, no analytics).

#### Scenario: Client pays from the link

- **WHEN** a client opens a valid share link for an unpaid invoice
- **THEN** they see totals, due date, and payment instructions without
  authenticating; after the owner marks it paid the same link shows
  `paid`.

#### Scenario: Revoked link dies

- **WHEN** the owner revokes a share link and the client reloads it
- **THEN** the response is 410 Gone.

### Requirement: Overdue Reminders Reach the Client

When the scheduler emits `invoice.overdue` (day 1/7/14), the system
SHALL record the reminder on the invoice timeline and, if the share
link exists and email channel is configured, send the reminder to the
contact's email address. Contacts without email or link are skipped
and marked so.

#### Scenario: Reminder logged even when undeliverable

- **WHEN** an overdue invoice has no share link and no contact email
- **THEN** the timeline records "reminder skipped (no channel)" and no
  retry storm occurs.

---

