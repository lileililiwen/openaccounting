# Onboarding Quick-Start — Tasks

## 1. Testing

- [x] 1.1 Integration test: a fresh ledger's setup page shows 5 steps with none done.
- [x] 1.2 Integration test: after recording an opening-balances entry, the opening-balances milestone is done and done count is 1.
- [x] 1.3 Integration test: after recording a non-opening transaction, the first-transaction milestone is done.
- [x] 1.4 Integration test: after uploading a document, the documents milestone is done.
- [x] 1.5 Integration test: after creating a bank feed link, the bank-feed milestone is done.
- [x] 1.6 Integration test: after inviting a collaborator, the collaborator milestone is done.
- [x] 1.7 Integration test: the dashboard shows the setup card while incomplete and hides it once all five are done.

## 2. Implementation

- [x] 2.1 Add `src/handlers/onboarding.rs`: `SetupStep`, `SetupStatus`, `compute_setup_status` (five EXISTS queries), and `setup_page` handler.
- [x] 2.2 Add `SetupPage` template struct (`src/templates/onboarding.rs`) + `templates/onboarding/setup.html` checklist.
- [x] 2.3 Add `setup: Option<SetupStatus>` to `DashboardPage` and render a compact setup card at the top of `templates/dashboard.html`.
- [x] 2.4 Wire the dashboard handler to compute and pass the setup status.
- [x] 2.5 Register `/ledgers/{id}/setup` GET route.

## 3. Documentation

- [x] 3.1 Mention the onboarding checklist in the README features list.
