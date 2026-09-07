# payee-learning Specification

## Purpose
TBD - created by archiving change data-interchange. Update Purpose after archive.
## Requirements
### Requirement: Alias Store

Confirmed reconciliation matches and manual payee edits SHALL update
`payee_aliases (ledger_id, alias_normalized, canonical_payee,
account_id, hit_count, last_used_at)` where `alias_normalized` is
lowercased with whitespace/punctuation collapsed. Updates are
idempotent per alias; `hit_count` increments reinforce.

#### Scenario: Confirmation teaches the system

- **WHEN** "ACME GmbH" from a feed line is matched to vendor
  "Acme Handels GmbH" → Office Supplies expense twice
- **THEN** the alias row exists with `hit_count=2` pointing at that
  contact and account.

### Requirement: Suggestions in Entry and Import

The transaction-entry payee field SHALL query suggestions by prefix +
fuzzy match over aliases, returning canonical payee, suggested
account, and confidence = `hit_count` decayed by recency. Statement
import preview SHALL pre-fill account/category from aliases above a
confidence threshold and mark them "suggested".

#### Scenario: Autocomplete fills the account

- **WHEN** a user types "acme" having 2 prior confirmations
- **THEN** the suggestion list shows "Acme Handels GmbH" first with the
  learned expense account pre-selected.

#### Scenario: Low confidence never auto-applies

- **WHEN** an alias has one stale hit below threshold
- **THEN** import shows it as a suggestion only, requiring confirmation.

### Requirement: Retroactive Rule Application

From the transactions list, users SHALL be able to select rows and
apply reconciliation rules + payee aliases retroactively
(`POST /ledgers/{id}/rules/apply` with transaction ids). The action
SHALL show a dry-run diff (old → new account/category) before writing,
and MUST refuse on closed periods.

#### Scenario: Recategorize a year of subscriptions

- **WHEN** 40 selected transactions match a rule and the dry-run is
  confirmed
- **THEN** all 40 are updated in one audited operation; any falling in
  a closed period abort the whole batch with 409.

