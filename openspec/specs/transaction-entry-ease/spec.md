# transaction-entry-ease Specification

## Purpose
TBD - created by archiving change a12-transaction-entry-ease. Update Purpose after archive.
## Requirements
### Requirement: Balancing Line

MUST designate one posting line on the transaction editor as the balancing line: its amount is computed live so that Σ debits equals Σ credits, and its direction flips to the opposite of the net of the other lines. Editing any other line SHALL immediately recompute the balancing line. The balancing line SHALL be visually marked as auto-computed.

#### Scenario: Two-leg entry balances itself

- **WHEN** the user enters a 5.50 debit on the first line of a fresh transaction
- **THEN** the balancing line shows a 5.50 credit and the indicator reads balanced.

#### Scenario: Direction flips with the net

- **WHEN** the user enters a 12.00 debit and a 5.50 credit on the fixed lines
- **THEN** the balancing line flips to debit with 6.50 so debits equal credits.

#### Scenario: Balancer is visible

- **WHEN** the user views the posting editor
- **THEN** the balancing line carries an "auto" marker so it is not mistaken for user input.

### Requirement: Balancing Line Promotion

MUST convert the balancing line to a fixed line when the user types an amount or changes the direction on it, and MUST then designate the next empty line (or a new blank line) as the balancing line. The conversion SHALL NOT silently overwrite the user's typed value.

#### Scenario: User takes over the balancer

- **WHEN** the user types an amount into the auto-computed line
- **THEN** that line becomes a fixed line with the typed value, a blank line is appended, and the blank line becomes the new balancer.

### Requirement: Balance Status Indicator

MUST render, adjacent to the save actions, a persistent status line that reads "ready to save" (positive state) only when the transaction balances with at least one amount entered, and otherwise shows the exact remaining amount and the side it belongs on (for example "needs 5.50 on the credit side"). The indicator SHALL NOT claim a balanced state before any amount is entered.

#### Scenario: Ready state

- **WHEN** the postings balance and at least one amount is entered
- **THEN** the status reads "✓ balanced — ready to save".

#### Scenario: Shortfall shown

- **WHEN** the postings are out of balance by 5.50
- **THEN** the status reads "needs 5.50 on the credit side" (or the matching side for a debit shortfall).

#### Scenario: No false positive

- **WHEN** the editor is empty
- **THEN** the status does not claim "ready to save".

### Requirement: Simple Entry Mode

MUST present a "Simple" entry mode on the new-transaction form as the default view, alongside an "Advanced" mode. Simple mode SHALL collect an amount, a date, a description, and two account choices (or an expense/income/transfer selector with a category and a payment account), and SHALL build the two balanced posting lines before submit without exposing Debit/Credit rows. Switching between Simple and Advanced SHALL preserve the data already entered.

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

### Requirement: Advanced Editor Retained

SHALL keep the multi-line posting editor (add line, split, manual direction) for complex transactions, with the balancing-line behaviour active in that view too.

#### Scenario: Split still works

- **WHEN** the user uses the advanced editor to build a three-leg split
- **THEN** the balancing line keeps the transaction balanced as the other lines are edited.

### Requirement: Inline Document Attach

MUST allow the user to select one or more document files on the new-transaction form; submitting the form SHALL create the transaction and attach the files to it in the same request. Attached documents SHALL remain linked to the transaction and be subject to the existing document authorization rules. The transaction SHALL be created even when no document is selected.

#### Scenario: Receipt attached with the entry

- **WHEN** the user attaches a receipt image and saves a new transaction
- **THEN** the transaction is created and the receipt appears in its document list.

#### Scenario: No document

- **WHEN** the user saves a transaction without selecting a file
- **THEN** the transaction is created normally with no documents.

#### Scenario: Document is proof only

- **WHEN** an uploaded document is stored
- **THEN** no category or amount is inferred from it; the postings are exactly what the user entered.

