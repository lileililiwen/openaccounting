# budgets Specification

## Purpose
Set spending and revenue targets per account, track actual vs. budget, and alert when thresholds are exceeded.

## Requirements

### Requirement: Budget Entity

The system MUST maintain a `budgets` table:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `account_id` UUID NOT NULL
- `period` TEXT NOT NULL CHECK (`period` IN ('monthly', 'quarterly', 'yearly'))
- `amount` NUMERIC(20,4) NOT NULL
- `start_date` DATE NOT NULL
- `end_date` DATE NOT NULL
- `created_at` TIMESTAMPTZ
- `updated_at` TIMESTAMPTZ

The combination of `account_id`, `period`, `start_date` MUST be
unique per ledger.

#### Scenario: Monthly budget is created

- **WHEN** a user creates a $5,000 monthly budget for the
  Marketing account starting 2026-01-01
- **THEN** a budget record is stored with `period='monthly'`,
  `amount=5000`, `start_date=2026-01-01`, `end_date=2026-01-31`.

### Requirement: Budget vs. Actual Report

The system MUST provide a budget vs. actual report that shows:

- Account name
- Budgeted amount for the period
- Actual amount (sum of postings in the period)
- Variance (budget - actual)
- Variance percentage
- Visual indicator (green if under budget, red if over)

The report MUST support filtering by period (month, quarter,
year) and date range.

#### Scenario: User views budget vs. actual for March

- **WHEN** a user views the budget report for March 2026
- **THEN** each budgeted account shows budget, actual, variance,
  and a color indicator.

### Requirement: Budget Alerts

The system MUST generate notifications when actual spending
exceeds a configurable threshold (default 80%) of the budgeted
amount. Alerts MUST be generated:

1. On ledger access (lazy check).
2. Via a daily background job (if available).

#### Scenario: Budget threshold alert is generated

- **WHEN** Marketing account has a $5,000 monthly budget and
  spending reaches $4,200 (84%)
- **THEN** a notification is created: "Marketing: 84% of monthly
  budget used ($4,200 / $5,000)".

### Requirement: Budget Management UI

The system MUST provide a UI to:

- List all budgets for a ledger.
- Create a new budget for an account.
- Edit an existing budget.
- Delete a budget.
- View the budget vs. actual report.

#### Scenario: User creates a budget

- **WHEN** a user navigates to the budget page and clicks "New
  Budget"
- **THEN** they see a form to select account, period, amount,
  and date range.
