# Onboarding Quick-Start

## Why

A brand-new ledger is an empty shell: the default chart of accounts is
seeded, but there is no guidance about what to do next. A person or
startup starting daily bookkeeping often doesn't know that they should
record their starting money, then their first transaction, attach
receipts, connect a bank feed, or share the books. Today they are left
to discover each feature on their own.

## What Changes

- A **setup checklist** computed from the ledger's data (no new
  schema): five milestones, each with a status and a link to the
  page that completes it.
  1. Set opening balances
  2. Record the first transaction
  3. Attach a document (receipt / invoice)
  4. Link a bank account / bank feed
  5. Invite a collaborator
- The **dashboard** shows a compact setup card with "N of 5 steps
  done" and the pending steps, hiding once everything is complete.
- A dedicated **`/ledgers/{id}/setup`** page lists all milestones with
  their completion state and links.

## Capabilities

- `onboarding-checklist`: a data-driven setup checklist with a
  dashboard card and a dedicated setup page.

## Non-Goals

- Multiple chart-of-accounts templates (a future change).
- A multi-step wizard that posts data across pages (the checklist
  links to the existing, working forms).
- Persisting a "dismissed" state for the dashboard card (it simply
  disappears when complete).
