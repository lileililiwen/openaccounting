# reconciliation-rules Specification (delta)

## ADDED Requirements

### Requirement: Rule Kinds

A rule SHALL have a `kind` in (`match`, `categorize`, `flag`).

A **match** rule proposes pairing an un-reconciled imported
line with an existing posting. Its action specifies the
pairing criterion: `link_by_amount_date` (amount equal AND date
within ±N days) or `link_by_posting_id` (explicit).

A **categorize** rule proposes a GL account for an un-reconciled
imported line. Its action specifies `gl_account_id`.

A **flag** rule surfaces an anomaly (e.g. unusual amount) for
human review. Its action specifies `reason_text` and a UI
highlight color (`yellow | red | blue`).

#### Scenario: A categorize rule fires

- **WHEN** a rule with `kind=categorize`,
  `predicate = {"payee_glob":"STARBUCKS%"}`,
  `action = {"gl_account_id":<id of "Travel & Meals">}`,
  `is_active=true` exists in the ledger
- **AND WHEN** the user imports a line `{date:"2026-08-14",
  amount:"42.50", payee:"STARBUCKS COFFEE"}`
- **THEN** the rule fires and the reconciliation page shows
  a suggestion badge "Suggested account: Travel & Meals" next
  to the line.

### Requirement: Predicate Language

A predicate SHALL be a JSON object with any of these keys, all
AND-combined:

- `payee_glob`           — SQL `LIKE` pattern matched against
                            `payee`.
- `description_glob`     — same against `description`.
- `amount_cents_eq`      — exact integer-cent match.
- `amount_cents_lt`      — strict less-than integer cents.
- `amount_cents_gt`      — strict greater-than integer cents.
- `date_offset_days_eq`  — days from today (negative for past).
- `currency`             — 3-letter code.

A predicate with no keys SHALL match every line. A predicate
with any key whose value is not the correct type MUST be
rejected at rule creation with `400 Bad Request`.

#### Scenario: Compound predicate

- **WHEN** a rule has
  `predicate = {"payee_glob":"AMZN%", "amount_cents_gt":10000}`
- **AND WHEN** the user imports a line with `payee="AMZN
  Mktp", amount:"42.50"`
- **THEN** the rule does NOT fire (amount too low).
- **WHEN** the user imports another line with `payee="AMZN
  Mktp", amount:"250.00"`
- **THEN** the rule fires.

### Requirement: Priority and Tiebreaking

Each rule SHALL have a `priority` integer (default 100). When
multiple rules match the same line, the lowest `priority`
number MUST win. For categorize, only the winning rule's GL
account is suggested (no chained categorization).

#### Scenario: Higher-priority rule overrides

- **WHEN** two categorize rules match the same line:
  R1 priority=200 suggests "Office Supplies", R2 priority=50
  suggests "Software & SaaS"
- **THEN** R2 wins and "Software & SaaS" is suggested.

### Requirement: Apply Suggestion

`POST /ledgers/{id}/rules/{rule_id}/apply` SHALL accept a
list of `imported_line_id` values and apply the rule's action
to each. For a categorize rule this creates the corresponding
postings (using the same commit flow as
`2026-08-14-csv-import-completion`). For a match rule this
sets `reconciliation_matches.imported_line_id =
posting_id`. For a flag rule this sets a UI annotation.

Apply MUST be atomic per call: any failure rolls back every
change in the call.

#### Scenario: Apply categorize suggestion

- **WHEN** the user accepts the suggestion for line L by
  posting `/rules/{R2}/apply` with `imported_line_id=[L]`
- **THEN** one new transaction is created (with the suggested
  GL account + the ledger's default cash account), L is
  marked `reconciled=true`, and the response is `303 See
  Other` to the reconciliation page.

### Requirement: Rule Audit

Creating, toggling, or deleting a rule MUST write an audit row
(`rule.create`, `rule.toggle`, `rule.delete`) with the
predicate and action JSON in `metadata`.

#### Scenario: Create writes a rule.create audit row

- **WHEN** the user posts a new rule via
  `POST /ledgers/{id}/rules`
- **THEN** an `audit_entries` row is written with
  `action='rule.create'`, `entity_type='reconciliation_rule'`,
  and the predicate + action JSON in `new_value`.
