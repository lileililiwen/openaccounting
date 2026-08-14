# expense-reimbursement Specification (delta)

## ADDED Requirements

### Requirement: Policy Engine

A ledger SHALL be able to have zero or more
`reimbursement_policies`. Each policy has:

- `id`, `ledger_id`, `name`, `kind`
  (`category_cap | receipt_required | per_diem`),
- `config_json` (per-kind configuration),
- `severity` (`hard | soft`, default `hard`),
- `is_active` (boolean, default `true`),
- `created_at`.

A policy SHALL be active when `is_active=true`. Only active
policies are evaluated.

#### Scenario: A policy row is created via SQL

- **WHEN** the operator inserts a policy row via
  `INSERT INTO reimbursement_policies (...)`
- **THEN** the row is queryable, the `kind` matches the
  CHECK constraint, and `is_active` defaults to `true`.

### Requirement: Category Cap Policy

A `category_cap` policy's `config_json` SHALL be
`{"category":"<name>","max_per_day":<decimal>,"currency":"<code>"}`.
The policy fires when the sum of that category's lines for
the same `txn_date` exceeds `max_per_day`. One violation
per (category, date) pair SHALL be returned (deduped).

#### Scenario: Cap exceeded

- **WHEN** the ledger has a 100 USD / day cap on
  `Other Expense` and a claim has a 200 USD line on
  `Other Expense` dated today
- **AND WHEN** the user posts `submit`
- **THEN** the response is 400 with body
  `policy violation: [<policy name>] category 'Other
  Expense' cap exceeded (actual 20000¢ > cap 10000¢)`.

### Requirement: Receipt Required Policy

A `receipt_required` policy's `config_json` SHALL be
`{"min_amount":<decimal>}`. The policy fires when a line
item's `amount >= min_amount` AND the line has no receipt
attached. The line is in violation.

#### Scenario: Receipt missing on a large line

- **WHEN** the ledger has a 50 USD receipt-required policy
  AND the user submits a claim with a 100 USD line that has
  no receipt
- **THEN** the response is 400 with body
  `policy violation: [<name>] receipt required for 10000¢
  (min 5000¢)`.

### Requirement: Per-Diem Policy

A `per_diem` policy's `config_json` SHALL be
`{"destination":"<string>","daily_rate":<decimal>,"currency":"<code>"}`.
The policy fires when a lodging line item's amount exceeds
`daily_rate * nights`. Nights SHALL be derived from the
difference between this line's `txn_date` and the previous
line of the same category in the same claim.

#### Scenario: Per-diem over

- **WHEN** a claim has two lodging lines dated 2026-08-13
  and 2026-08-15 (2 nights), and a Berlin per-diem of
  100 EUR / night, but the line amounts are 500 EUR each
- **THEN** the policy fires and the violation amount is
  300 EUR (200 over the 2-night allowance of 200).

### Requirement: Evaluation Timing

Policies SHALL be evaluated at three points:

1. **Line add** — soft-severity policies return warnings
   in the `add_line` response (no blocking).
2. **Claim submit** — hard-severity violations block submit
   with `400 Bad Request` and a per-violation list.
3. **Claim approve** — re-evaluate; if a hard violation
   exists, the approval is blocked with the same 400
   response.

#### Scenario: Activated-after-submit blocks approval

- **WHEN** a claim is submitted while no policies exist
- **AND WHEN** the operator then inserts a hard
  `category_cap` policy
- **AND WHEN** the user posts `approve`
- **THEN** the response is 400 with the cap-violation
  message.

#### Scenario: Soft warning does not block

- **WHEN** the policy is `severity=soft`
- **THEN** the submit and approve proceed normally; the
  page surfaces an advisory warning banner instead of
  returning 4xx.

### Requirement: Policy Audit

Creating, toggling, or deleting a policy MUST write an audit
row (`policy.create`, `policy.toggle`, `policy.delete`)
with the `config_json` in metadata.

#### Scenario: Policy create writes an audit row

- **WHEN** the operator inserts a policy
- **THEN** an `audit_entries` row is written with
  `action='policy.create'`, `entity_type='reimbursement_policy'`.
