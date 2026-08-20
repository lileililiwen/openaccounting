# Onboarding Quick-Start — Design

## Approach

A pure read-only feature: a `SetupStatus` computed from the ledger's
existing rows. No migration, no new tables. The status feeds two
surfaces — a compact card on the dashboard (shown only while
incomplete) and a full `/ledgers/{id}/setup` page.

## Decisions

- **Milestones are derived, not stored.** WHY: every milestone maps to
  data that already exists (`transactions`, `documents`,
  `bank_feed_links`, `ledger_members`/`ledger_invitations`), so the
  checklist is always accurate and needs no write path or cleanup.
  Mitigation: each milestone is a single `EXISTS` query over the
  ledger.

- **Five milestones, ordered by workflow.** Opening balances first
  (bring in starting position), then the first transaction, then
  documents, then a bank feed, then collaboration. WHY: this mirrors
  how a real bookkeeper starts. Mitigation: the setup page lists all
  five with links, so users can do them in any order.

- **"First transaction" excludes the opening-balances entry and
  drafts.** WHY: the opening entry is set up elsewhere on the checklist,
  and drafts are scratch work. Mitigation: the query filters
  `kind != 'draft'` and `description != 'Opening balances'`.

- **Collaborator counts any member beyond the owner or any pending
  invitation.** WHY: an invited-but-unaccepted accountant is still a
  useful signal that sharing was started. Mitigation: the query checks
  both `ledger_members` (other users) and `ledger_invitations`.

- **Dashboard card hides when complete.** WHY: once setup is done it is
  noise. Mitigation: the dashboard handler passes `None` when all five
  milestones are done; the setup page always shows the full list so it
  stays reachable.

## Data Flow

1. `compute_setup_status(pool, ledger_id, owner_id)` runs five EXISTS
   queries and builds `SetupStatus { steps, done, total, complete }`.
2. The dashboard handler calls it and renders the card when
   `!complete`; the setup page handler renders the full checklist.
3. Each step's link points at the existing page that completes it.

## Risks / Trade-offs

- A ledger with heavy history could leave a milestone "undone" forever
  (e.g., never linked a bank feed). Mitigation: the dashboard card
  disappears only when ALL five are done; users who don't want bank
  feeds can use the manual/import path and the card eventually clears
  via the first transaction + documents, or simply live with it — the
  card is non-blocking.
- `documents` rows bound before the `ledger_id` column existed may have
  NULL `ledger_id`. Mitigation: the query also counts documents whose
  `transaction_id` belongs to the ledger.
