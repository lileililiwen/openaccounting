# tax-on-transactions Specification (delta)

## ADDED Requirements

### Requirement: Tax rate selection on postings

MUST allow attaching one of the ledger's active tax rates to a posting line when entering a transaction in Advanced mode.

#### Scenario: Select a rate on a line

- **WHEN** a user selects a tax rate on a posting line and saves the transaction
- **THEN** the chosen rate is recorded against that posting.

#### Scenario: No rate selected

- **WHEN** a posting line has no tax rate selected
- **THEN** no tax is recorded for that line.

### Requirement: Tax posting generation

SHALL post an additional leg to the tax rate's account on the same debit/credit side as a taxed line, with amount equal to `round(line amount × rate, 2)`, and SHALL record the linkage (posting, tax rate, base amount, tax amount).

#### Scenario: Tax leg amount

- **WHEN** a transaction is saved with a taxed line of amount 100.00 and rate 10%
- **THEN** a tax leg of 10.00 is posted to the tax rate's account on the same side.

#### Scenario: Linkage recorded

- **WHEN** a transaction with a taxed line is saved
- **THEN** a `posting_taxes` row links that posting to the rate with the base amount and computed tax amount.

#### Scenario: Still balanced

- **WHEN** a transaction includes tax legs
- **THEN** the transaction remains balanced (Σ debits = Σ credits including the tax legs).

### Requirement: Balancing editor behavior

The balancing-line editor SHALL account for tax so entries stay balanced without manual re-entry.

#### Scenario: Balancer includes tax

- **WHEN** a user applies a tax rate to a non-balancing line
- **THEN** the balancing line's amount includes the tax (gross = net + tax) and the entry stays balanced.

#### Scenario: No tax on the balancer

- **WHEN** a line holds the balancing role
- **THEN** its tax selector is disabled so the balancing line cannot carry tax.

#### Scenario: Clearing a rate

- **WHEN** the user clears a tax rate from a line
- **THEN** the tax leg's contribution is removed from the balance computation.

#### Scenario: Submit guarded

- **WHEN** the user submits the form
- **THEN** the entry is only submitted when it balances including tax.

### Requirement: Tax report accuracy

The tax report SHALL aggregate taxable base (net), tax, and gross per tax rate over the requested date range; the CSV export SHALL include the same data.

#### Scenario: Aggregates over a period

- **WHEN** a user views the tax report for a period containing taxed transactions
- **THEN** each rate shows its net base, tax, and gross totals.

#### Scenario: Empty rate still listed

- **WHEN** a rate has no taxed transactions in the period
- **THEN** the report still lists the rate with zero totals.

### Requirement: Drafts retain tax

Tax attribution SHALL survive draft → promote.

#### Scenario: Draft promoted

- **WHEN** a draft is saved with a taxed line and later promoted
- **THEN** the promoted transaction keeps the tax leg and the `posting_taxes` linkage.

### Requirement: Reversal reverses tax

Reversing or editing a transaction SHALL negate tax legs along with the other legs.

#### Scenario: Reverse a taxed transaction

- **WHEN** a user reverses a transaction that contains a tax leg
- **THEN** the reversal posting set includes a negated tax leg.
