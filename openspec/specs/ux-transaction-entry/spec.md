# ux-transaction-entry Specification

## Purpose
The "New transaction" form is the core data-entry surface of the app. A novice user must be able to record a meaningful transaction without accounting knowledge, and the form MUST NOT silently accept entries that are balanced but economically nonsensical. Found by a novice-user form submission test on 2026-08-19; evidence in `docs/ux-novice-form-test/*.png`.

## Requirements

### Requirement: Honest Balance Indicator

MUST NOT display a "balanced" success state until at least one posting line carries an amount; before any amount is entered the indicator SHALL show a neutral placeholder (e.g. "—" or "net 0.00").

#### Scenario: Empty form

- **WHEN** a user opens the new-transaction form with no amounts entered
- **THEN** the indicator does not claim "✓ balanced" (observed green "✓ balanced" on an empty form, see `21_txn_new_form.png`).

#### Scenario: Unbalanced entry

- **WHEN** a user enters a single 5.50 debit with no matching credit
- **THEN** the indicator shows the imbalance clearly (observed "net 5.50", see `23_save_unbalanced.png`).

### Requirement: Same-Account Leg Warning

MUST warn the user when the same account appears on both the debit and credit side of a transaction; SHOULD refuse to save such a transaction or require explicit confirmation.

#### Scenario: Self-referential legs

- **WHEN** a novice selects "Cash on Hand" for both the debit and credit line of a 5.50 transaction
- **THEN** the form explains that the entry moves money within the same account and asks the user to choose different accounts (observed: the app silently saved a `Cash on Hand → Cash on Hand` transaction, see `24_save_both_debit.png`).

### Requirement: Future-Date Confirmation

SHOULD require confirmation for transaction dates far in the future (e.g. more than 30 days ahead), since such dates are almost always entry errors.

#### Scenario: Far-future date

- **WHEN** a user records a transaction dated `2030-01-01` in 2026
- **THEN** the app asks "You're recording a transaction dated 2030-01-01 — is that intentional?" before saving (observed: saved silently, see `24_save_both_debit.png`).

### Requirement: Grouped Account Picker

SHALL present the account dropdown grouped by account type (`ASSET`, `EXPENSE`, `INCOME`, …) and SHOULD provide incremental search once the account list exceeds ~10 entries.

#### Scenario: Default chart of accounts

- **WHEN** a user opens the account dropdown on the transaction form
- **THEN** the 19 default accounts render in `<optgroup>`s by type instead of one flat, alphabetically unhelpful list (see `21_txn_new_form.png`).

### Requirement: Plain-Language Debit/Credit Help

SHOULD explain the debit/credit model in non-accounting terms and SHOULD offer a "simple entry" mode that builds the two legs for the user.

#### Scenario: Novice records a coffee expense

- **WHEN** a user wants to record spending 5.50 on coffee
- **THEN** the form offers "Spend 5.50 from Bank Account on Office Supplies" style guidance or a one-click pattern that creates the expense + cash legs, instead of leaving two empty Debit/Credit rows (see `21_txn_new_form.png`).

#### Scenario: Help affordance

- **WHEN** a user hovers or clicks a "?" next to "Postings (must balance)"
- **THEN** a tooltip explains "Every transaction has two sides: where the money went (expense/asset) and where it came from (cash/bank)."

### Requirement: Save-as-Draft Secondary Action

MUST render "Save as draft" visually subordinate to the primary "Save transaction" action (e.g. a link or outline button), so the primary action is unambiguous.

#### Scenario: Both actions visible

- **WHEN** a user views the bottom of the transaction form
- **THEN** "Save transaction" is the dominant CTA and "Save as draft" reads as a secondary option (observed: two equal-weight buttons, see `21_txn_new_form.png`).

## Out of Scope
- Changing the double-entry data model or the server-side balance invariant.
- The ledger, account, and template forms (covered by other specs / future work).
