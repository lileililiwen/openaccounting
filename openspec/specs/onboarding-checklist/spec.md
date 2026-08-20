# onboarding-checklist Specification

## Purpose
TBD - created by archiving change 2026-08-20-a16-onboarding-quickstart. Update Purpose after archive.
## Requirements
### Requirement: Setup checklist

MUST provide a ledger setup checklist derived from the ledger's data, with five milestones — opening balances, first transaction, a document, a bank feed, and a collaborator — each with a completion status and a link to the page that completes it.

#### Scenario: Fresh ledger

- **WHEN** a ledger has no activity
- **THEN** all five milestones are shown as pending on the setup page.

#### Scenario: Milestone completion

- **WHEN** the ledger records an opening-balances entry, or its first non-opening transaction, or a document, or a bank feed link, or a member/invitation
- **THEN** the corresponding milestone is marked done.

### Requirement: Dashboard setup card

The dashboard SHALL show a compact setup card with the done/total counts and pending steps while any milestone is incomplete, and SHALL hide it once all milestones are complete.

#### Scenario: Incomplete shows card

- **WHEN** a ledger has pending milestones
- **THEN** the dashboard shows a card with "N of 5" and the pending step links.

#### Scenario: Complete hides card

- **WHEN** all five milestones are done
- **THEN** the dashboard does not show the setup card.

