## ADDED Requirements

### Requirement: Posting Dimensions

Postings MAY carry an optional cost center and project. Trial balance and P&L SHALL accept dimension filters and slice correctly. Untagged postings SHALL appear under Unassigned.

#### Scenario: Dimension slice filters

- **WHEN** two postings share an account but differ in project P1 vs P2 and P&L filters P1
- **THEN** only the P1 amount appears.

### Requirement: Recurring Journals

Ledger writers SHALL create recurring journal templates with start, frequency, end/occurrence cap, and pause/skip. Due runs SHALL generate preview drafts; posting SHALL be explicit or scheduler-confirmed with idempotency per template period.

#### Scenario: Double-run is idempotent

- **WHEN** the scheduler fires twice for template T period 2026-04
- **THEN** one journal exists for that period.

#### Scenario: Skip excludes a period

- **WHEN** 2026-05 is skipped on template T
- **THEN** no draft is generated for 2026-05 and later periods continue.

### Requirement: Valuation Disclosure

Inventory and depreciation methods SHALL be stored explicitly and disclosed on dependent reports. Changing inventory method with nonzero stock SHALL be rejected.

#### Scenario: Method switch blocked with stock

- **WHEN** a ledger with nonzero inventory switches FIFO to average
- **THEN** the change fails with 409 until stock is zero or a restatement is posted.

### Requirement: FX Override Audit

Every manual FX rate entry or edit SHALL record actor, timestamp, old and new values, and reason. Revaluation SHALL use the audited manual rate for that day.

#### Scenario: Override recorded

- **WHEN** an owner sets EUR→USD 1.09 over feed 1.08 with reason "bank fixing rate"
- **THEN** the audit log shows actor, both values, and the reason.
