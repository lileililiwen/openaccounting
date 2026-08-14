# expense-reimbursement Specification (delta)

## ADDED Requirements

### Requirement: Policy Engine

A ledger MAY have zero or more `reimbursement_policies`. Each
policy has:

- `id`, `ledger_id`, `name`, `kind`
  (`category_cap | receipt_required | per_diem`),
- `config_json` (per-kind configuration, see below),
- `severity` (`hard | soft`, default `hard`),
- `is_active` (boolean),
- `created_at`.

A policy is **active** when `is_active=true`. Only active
policies are evaluated.

#### Scenario: Policy listing

- **WHEN** the ledger owner visits
  `GET /ledgers/{id}/policies`
- **THEN** the page shows every policy (active and inactive)
  with name, kind, severity, and a toggle switch.

### Requirement: Category Cap Policy

A `category_cap` policy's `config_json` is
`{"category":"<enum>","max_per_day":<decimal>,"currency":"<code>"}`.
The policy fires when a line item's category matches AND the
sum of that category's lines for the same `txn_date` exceeds
`max_per_day`. Excess is the violation amount.

#### Scenario: Cap exceeded

- **WHEN** a `category_cap` policy with
  `category=meals, max_per_day=200` is active
- **AND WHEN** the user adds two lines on 2026-08-14 with
  category `meals`, amounts 150 and 100
- **THEN** adding the second line surfaces a `soft` warning
  ("meals cap exceeded by 50.00"); submitting the claim
  surfaces a `hard` violation and the submit handler returns
  `400 Bad Request` with body `Policy violation: meals cap
  (200.00/day) exceeded on 2026-08-14 by 50.00`.

### Requirement: Receipt Required Policy

A `receipt_required` policy's `config_json` is
`{"min_amount":<decimal>}`. The policy fires when a line
item's `amount >= min_amount` AND the line has no
`receipt_document_id`. The line is in violation.

#### Scenario: Receipt missing on a large line

- **WHEN** a `receipt_required` policy with `min_amount=50` is
  active
- **AND WHEN** the user adds a line `amount=120.00` without
  a receipt
- **THEN** the submit handler returns `400 Bad Request` with
  body `Policy violation: receipt required for line item
  amount 120.00 (threshold 50.00). Upload a receipt first.`.

### Requirement: Per-Diem Policy

A `per_diem` policy's `config_json` is
`{"destination":"<string>","daily_rate":<decimal>,"currency":"<code>"}`.
The policy fires when a line item's `category=lodging` AND
the line's `description` contains the destination string
(case-insensitive). The line's "allowed" amount is
`daily_rate * nights`, where `nights` is derived from the
difference between this line's `txn_date` and the previous
line of the same category in the same claim. Excess is the
violation.

#### Scenario: Per-diem over

- **WHEN** a `per_diem` policy for Berlin with
  `daily_rate=150, currency=EUR` is active on a CNY ledger
- **AND WHEN** the user adds two consecutive lodging lines
  in the same claim: night 1 `2026-08-14, amount=200 EUR`,
  night 2 `2026-08-15, amount=120 EUR`
- **THEN** the violation for night 1 is `200 - 150 = 50 EUR`;
  for night 2 (single-night, allowed = 150) the violation is
  `120 - 150 = -30` (under — no violation, soft "under
  per-diem" warning only).

### Requirement: Evaluation Timing

Policies are evaluated at three points:

1. **Line add** — soft-severity policies return warnings in
   the `add_line` response (no blocking).
2. **Claim submit** — hard-severity violations block submit
   with `400 Bad Request` and a per-violation list.
3. **Claim approve** — re-evaluate; if a policy was activated
   after submit, the approval is blocked with the same 400
   response. If a policy was deactivated after submit, soft
   warnings are cleared.

#### Scenario: Activated-after-submit blocks approval

- **WHEN** a claim was submitted cleanly and later a
  `category_cap` policy is activated whose threshold is
  exceeded by an existing line
- **AND WHEN** an approver calls `/approve`
- **THEN** the response is `400 Bad Request` with body
  `Cannot approve: policy violation(s) since submit: <list>.`
  and the claim remains in `submitted` status.

### Requirement: Policy Audit

Creating, toggling, or deleting a policy writes an audit row
(`policy.create`, `policy.toggle`, `policy.delete`) with the
`config_json` in metadata.
