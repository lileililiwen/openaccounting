# Account Management & Opening Balances

## Why

Accounts can only be created and listed. A daily-use bookkeeper
regularly needs to:

- **Rename / re-code / re-describe** an account they created (e.g.
  "Coffee" → "Coffee & Tea"), or fix a typo. Today the only option is
  to leave the wrong name forever.
- **Archive** accounts that are no longer used (closed bank account,
  retired category) so they stop cluttering the entry pickers, without
  losing their history from reports.
- **Set opening balances** when starting the books — a ledger starts
  empty, but a real business starts with money already in the bank,
  an outstanding invoice, or a loan. Without an opening-balance entry
  there is no way to bring the starting position into the books.

## What Changes

- An **edit account** page: change name, code, and description at any
  time; change type/subtype only while the account has no postings.
- **Archive / activate** an account from the chart of accounts list.
  Archived accounts are hidden from entry pickers (already how the
  pickers filter) and stay in reports; the list shows an "archived"
  badge.
- An **opening balances** page: enter the starting balance for each
  balance-sheet account (asset, liability, equity). Saving posts one
  balanced `standard` transaction on the ledger's start date against
  the "Opening Balances" equity account. Posting a second time is
  refused.

## Capabilities

- `account-edit`: edit an account's name/code/description (and
  type/subtype while it has no postings).
- `account-archive`: archive and reactivate accounts.
- `opening-balances`: record starting balances as a balanced entry
  against the Opening Balances equity account.

## Non-Goals

- Sub-account / parent-account editing (the schema has `parent_id`
  but no UI; out of scope).
- Re-opening or editing an existing opening-balances entry in place
  (users can reverse it, matching the app's reversal model).
- Multi-currency opening balances (per-ledger base currency only).
