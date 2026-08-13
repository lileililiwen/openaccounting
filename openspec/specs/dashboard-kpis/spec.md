# dashboard-kpis Specification

## Purpose
Display key financial metrics and health indicators on the dashboard to give users an at-a-glance view of their business financial health.

## Requirements

### Requirement: Cash Runway

The dashboard MUST display "cash runway" — the number of months
the current cash balance can cover operating expenses. The
calculation MUST be:

```
cash_runway = bank_balance / avg_monthly_operating_expenses
```

Where `avg_monthly_operating_expenses` is the average of the
last 3 months' total expenses (excluding transfers and one-time
items).

#### Scenario: Cash runway is displayed

- **WHEN** a user views the dashboard
- **THEN** the cash runway widget shows "X.X months" with a
  color indicator: green (>6 months), amber (3-6 months),
  red (<3 months).

### Requirement: Month-over-Month Comparison

The dashboard MUST show month-over-month (MoM) change for:

- Revenue (total income postings)
- Expenses (total expense postings)
- Net income (revenue - expenses)
- Bank balance

Each metric MUST show the percentage change from the previous
month with a directional indicator (↑ or ↓).

#### Scenario: MoM comparison is displayed

- **WHEN** a user views the dashboard
- **THEN** each metric shows the current month value and the
  percentage change from the previous month with a directional
  arrow.

### Requirement: Accounts Receivable Summary

The dashboard MUST display:

- Total AR outstanding (sum of open invoices)
- Overdue AR (sum of invoices past due date)
- Number of open invoices
- Number of overdue invoices

#### Scenario: AR summary is displayed

- **WHEN** a user views the dashboard
- **THEN** the AR widget shows total outstanding, overdue amount,
  and counts.

### Requirement: Accounts Payable Summary

The dashboard MUST display:

- Total AP outstanding (sum of open bills)
- Upcoming AP (bills due in the next 7 days)
- Number of open bills
- Number of overdue bills

#### Scenario: AP summary is displayed

- **WHEN** a user views the dashboard
- **THEN** the AP widget shows total outstanding, upcoming amount,
  and counts.

### Requirement: Top Expense Categories

The dashboard MUST show a horizontal bar chart of the top 5
expense categories for the current month, with amounts and
percentages.

#### Scenario: Top expenses chart is displayed

- **WHEN** a user views the dashboard
- **THEN** a bar chart shows the 5 largest expense categories
  with amounts and percentages of total expenses.

### Requirement: Quick Actions

The dashboard MUST include quick-action buttons for common tasks:

- New Transaction
- Upload Document
- View Reports

#### Scenario: Quick action buttons are available

- **WHEN** a user views the dashboard
- **THEN** quick action buttons are visible and link to the
  respective pages.
