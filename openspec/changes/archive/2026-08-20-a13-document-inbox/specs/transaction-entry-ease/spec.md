# transaction-entry-ease Specification (delta)

## MODIFIED Requirements

### Requirement: Simple Entry Mode

MUST present the multi-leg posting editor (with the balancing line) as the DEFAULT view of the new-transaction form; the two-leg "Simple" entry mode SHALL remain available as an optional shortcut, alongside an "Advanced" mode. Simple mode SHALL collect an amount, a date, a description, and two account choices (or an expense/income/transfer selector with a category and a payment account), and SHALL build the two balanced posting lines before submit without exposing Debit/Credit rows. Switching between Simple and Advanced SHALL preserve the data already entered.

#### Scenario: Default is multi-leg

- **WHEN** a user opens the new-transaction form
- **THEN** the multi-leg editor is shown with the balancing line active.

#### Scenario: Expense from everyday language

- **WHEN** the user enters Expense, Category = Office Supplies, Amount = 5.50, Paid from = Bank Account
- **THEN** the submitted postings are DEBIT Office Supplies 5.50 and CREDIT Bank Account 5.50.

#### Scenario: Income

- **WHEN** the user enters Income, Category = Sales Revenue, Amount = 120.00, Received into = Bank Account
- **THEN** the submitted postings are DEBIT Bank Account 120.00 and CREDIT Sales Revenue 120.00.

#### Scenario: Transfer

- **WHEN** the user enters Transfer, From = Bank Account, To = Credit Card, Amount = 50.00
- **THEN** the submitted postings are DEBIT Credit Card 50.00 and CREDIT Bank Account 50.00.

#### Scenario: Mode switch preserves data

- **WHEN** the user fills simple fields and switches to Advanced
- **THEN** the two generated lines are visible with their accounts and amounts, and nothing entered is lost.

#### Scenario: Simple stays a shortcut

- **WHEN** a user switches to Simple and back to the editor
- **THEN** no entered data is lost.
