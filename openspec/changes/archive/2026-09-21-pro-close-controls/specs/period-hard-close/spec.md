## ADDED Requirements

### Requirement: Hard Close Watermark

The system SHALL maintain one close watermark per ledger in `closed_periods`. Every write path (transaction create/edit/reversal, import commit, revaluation, amortization post) SHALL reject dates `<= closed_through` with HTTP 409 and message naming the closed date.

#### Scenario: Write into closed period is blocked

- **WHEN** ledger L is closed through 2026-03-31 and an editor posts a transaction dated 2026-03-15
- **THEN** the save fails with 409 and no rows are written.

#### Scenario: Post-close dates still work

- **WHEN** the same ledger posts a transaction dated 2026-04-01
- **THEN** the transaction is accepted normally.

### Requirement: Reopen With Override Audit

Reopening or posting into a closed period SHALL require global admin or ledger owner role plus a non-empty reason (min 10 chars). The override SHALL write a `reopen_events` row and an audit-chain row with actor, timestamp, and reason.

#### Scenario: Override is logged

- **WHEN** an owner reopens 2026-03 with reason "Correcting supplier invoice INV-104 per auditor request"
- **THEN** a reopen event and audit row exist and the correction posts.

#### Scenario: Reasonless override rejected

- **WHEN** an owner attempts reopen with an empty reason
- **THEN** the request fails with 400 and nothing changes.

### Requirement: Maker-Checker Approval

Journals at or above the ledger threshold SHALL be created in `pending` status, excluded from all reports, and SHALL require approval by a different user before posting. The maker SHALL NOT be able to approve their own journal.

#### Scenario: Self-approval blocked

- **WHEN** user A creates a 50,000 journal and then attempts to approve it
- **THEN** approval fails with 403 and the journal stays pending.

#### Scenario: Second user approves

- **WHEN** user B approves user A's pending journal
- **THEN** the journal becomes posted and appears in the trial balance.

### Requirement: Accountant and Auditor Roles

The system SHALL support `accountant` (all editor writes except close/share/override) and `auditor` (read plus CSV/JSON/PDF export, no writes). Close, reopen, and threshold changes SHALL require owner or admin.

#### Scenario: Auditor cannot write

- **WHEN** an auditor attempts to create a transaction
- **THEN** the request fails with 403.

### Requirement: Gapless Invoice Numbering

Invoices SHALL carry `invoices.number` from a per-ledger-per-year gapless sequence, unique per ledger. Voiding SHALL retain the number with a mandatory reason. A gap report SHALL distinguish voids from true missing numbers.

#### Scenario: Void keeps its number

- **WHEN** invoice 2026-0042 is voided with reason "duplicate issue"
- **THEN** number 2026-0042 remains reserved and the gap report lists it as void, not missing.
