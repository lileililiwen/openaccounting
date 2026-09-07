# notification-channels Specification

## Purpose
TBD - created by archiving change automation-platform. Update Purpose after archive.
## Requirements
### Requirement: Channel Registry

Notifications SHALL be delivered through a channel registry with at
least: `push` (existing), `email` (SMTP via new optional config:
`SMTP_URL`, `SMTP_FROM`), and `http` (generic POST to a user-supplied
URL, ntfy/Gotify-compatible body). A channel that is unconfigured MUST
be skipped silently for delivery purposes while remaining selectable
in preferences as "not configured".

#### Scenario: Email channel sends when configured

- **WHEN** SMTP is configured and a budget threshold alert fires for a
  user with email enabled
- **THEN** one email is sent and a delivery record with status `sent`
  is stored.

#### Scenario: Unconfigured channel does not block others

- **WHEN** a user enables push + email but SMTP is unset
- **THEN** push still delivers and the event records email as `skipped`.

### Requirement: Per-User Preferences

The existing `/account/notifications` page SHALL let each user choose,
per event class (`budget_alert`, `invoice_overdue`, `weekly_digest`,
`reimbursement_decision`), which channels receive it. Defaults: email
on for all classes when SMTP is configured; digest off.

#### Scenario: Digest opt-out

- **WHEN** a user disables `weekly_digest` for all channels
- **THEN** the weekly digest job skips them entirely.

### Requirement: Weekly Digest

The scheduler SHALL send an opted-in user a weekly summary: income,
expenses, net, top 3 expense categories, overdue invoice count, and
budget alerts — rendered from server templates in the user's locale.

#### Scenario: Digest content matches ledger data

- **WHEN** the digest job runs for a user owning one ledger
- **THEN** the figures equal the income statement totals for the ISO
  week covered.

