# Account Management & Opening Balances — Tasks

## 1. Testing

- [x] 1.1 Integration test: editing an account's name/code/description updates the row and writes an audit log entry.
- [x] 1.2 Integration test: changing type/subtype is rejected (400/validation) once the account has postings.
- [x] 1.3 Integration test: changing type/subtype succeeds when the account has no postings.
- [x] 1.4 Integration test: archiving an account flips `is_archived`, hides it from the new-transaction picker, and still appears in reports.
- [x] 1.5 Integration test: activating an archived account restores it in the picker.
- [x] 1.6 Integration test: saving opening balances creates one balanced transaction (Σ debits = Σ credits) dated on the ledger start, with the Opening Balances equity account as the contra side.
- [x] 1.7 Integration test: a second opening-balances save is refused.
- [x] 1.8 Integration test: opening balances with no nonzero targets (all zeros) creates no transaction.

## 2. Implementation

- [x] 2.1 Add `AccountEdit` template struct + `templates/accounts/edit.html` (pre-filled, type/subtype selects disabled with a note when locked).
- [x] 2.2 Handler: `edit_page` (loads account, balance-sheet accounts not needed) + `update` (validate name; lock type/subtype when postings exist; audit log).
- [x] 2.3 Handler: `toggle_archive` POST + audit log.
- [x] 2.4 Routes: `/accounts/{account_id}/edit` GET+POST, `/accounts/{account_id}/toggle-archive` POST.
- [x] 2.5 `accounts/list.html`: add per-row Edit + Archive/Activate links (desktop + mobile) and an "Opening balances" button in the header.
- [x] 2.6 `OpeningBalancesPage` template struct + `templates/accounts/opening_balances.html` (date input, per-account current vs target).
- [x] 2.7 Handler: `opening_balances_page` (balance-sheet accounts + current balances) and `opening_balances_create` (compute deltas, build legs, find/create Opening Balances equity account, PostingService::create, one-time guard).
- [x] 2.8 Routes: `/ledgers/{id}/opening-balances` GET+POST.

## 3. Documentation

- [x] 3.1 Add account management + opening balances to the README features list.
