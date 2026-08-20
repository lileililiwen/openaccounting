# Account Management & Opening Balances — Design

## Approach

Three independent, small features on the existing `accounts` table
(no schema migration required — `is_archived`, `description`, `code`
already exist).

1. **Edit**: a GET form (`/accounts/{id}/edit`) re-uses the new-account
   form layout, pre-filled. The POST handler updates name/code/
   description always; type/subtype only when the account has no
   postings.
2. **Archive/activate**: a single POST toggle on `is_archived`.
   Archived accounts are already excluded from entry pickers by the
   existing `is_archived = FALSE` filters and are already included in
   reports (report queries load all accounts), so no report logic
   changes.
3. **Opening balances**: a page listing balance-sheet accounts
   (ASSET / LIABILITY / EQUITY). Saving computes
   `delta = target − current balance` per account, builds one leg per
   non-zero delta in the account's normal direction, and posts the
   balancing total to the ledger's "Opening Balances" equity account
   as a `standard` transaction dated on the ledger's start date.

## Decisions

- **Type/subtype locked once an account has postings.** WHY: reclassifying
  an account with history silently rewrites the meaning of past reports.
  Mitigation: the edit page disables those selects and shows an
  explanatory note when the account has postings; name/code/description
  stay editable.

- **Archiving is soft and always reversible.** WHY: an archived account
  with history must still appear in reports and the general ledger;
  hiding it from pickers is the only behavior change. Mitigation: the
  list shows an "archived" badge; an "Activate" action restores it.
  No guard on accounts with postings — history is preserved either way.

- **Opening balances = one balanced transaction, posted once.** WHY: it
  uses the app's own double-entry engine (PostingService), is visible
  and reversible like any other entry, and needs no new tables. The
  contra side is the seeded "Opening Balances" equity account (created
  on the fly if a ledger somehow lacks it). Mitigation: a second save is
  refused with a clear message; fixing means reversing the entry.

- **Delta-based math, not absolute.** WHY: a user who already recorded
  some activity can still bring each account to its correct starting
  figure (the difference is posted). Mitigation: the form pre-fills each
  input with the account's current balance so an untouched ledger shows
  zeros.

- **No new schema.** WHY: all required columns exist. Risk: none
  material.

## Data Flow

1. `accounts/list.html` gains per-row Edit + Archive/Activate links and
   an "Opening balances" button in the header.
2. Edit page submits name/code/description/type/subtype; the handler
   checks `COUNT(postings WHERE account_id = $1) == 0` before allowing
   type/subtype changes.
3. Opening-balances page submits a date + one amount per account. The
   handler builds debit/credit legs in normal direction, appends the
   Opening Balances contra leg, and calls
   `PostingService::create` with `description = "Opening balances"`.
4. The list page keeps showing live balances, so the effect of the
   opening entry is immediately visible.

## Risks / Trade-offs

- A user could set an opening balance that contradicts existing
  activity (e.g. target below current for a bank account). Mitigation:
  the delta is posted as-is; the page shows current balances next to
  each input so the impact is visible before saving.
- Two ledgers both wanting a "first day" date — default is the ledger's
  creation date, and the date is user-editable. Mitigation: document in
  the form.
