# cash-basis-docs Specification (delta)

## ADDED Requirements

### Requirement: Cash-Basis Documentation

MUST document how cash-basis works: a read-time filter that only counts postings whose peer leg is a cash or bank account; the underlying postings are unchanged.

#### Scenario: Docs exist

- **WHEN** the user opens docs/cash-basis.md
- **THEN** the doc explains the model, lists the supported accounts, and shows an example.

### Requirement: Recognize Revenue Action

MUST provide a one-click action on a DeferredRevenue line that moves the amount to Income on a chosen date, with a reversing entry on the same line for any unearned remainder.

#### Scenario: Recognize 100 of 1200

- **WHEN** the user clicks 'Recognize 100' on a deferred revenue line
- **THEN** a balanced txn is created: Dr DeferredRevenue 100 / Cr Revenue 100.

### Requirement: Recognize Expense Action

MUST provide the symmetric action for PrepaidExpense.

#### Scenario: Recognize 50 of 200 prepaid

- **WHEN** the user clicks 'Recognize 50'
- **THEN** Dr Expense 50 / Cr Prepaid 50.

### Requirement: Schedule

MUST allow the user to schedule monthly recognition; on the chosen day of each month the action is auto-applied.

#### Scenario: Monthly schedule

- **WHEN** the user schedules $100/month recognition starting 2025-01-15
- **THEN** 12 transactions are posted over the year; each dated 2025-MM-15.
