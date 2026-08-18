# investment-lots Specification (delta)

## ADDED Requirements

### Requirement: Lot Creation

MUST create one lot per buy transaction on an account flagged as `type='Investment'`.

#### Scenario: Buy 10 shares @ 50

- **WHEN** the user posts a buy txn to a brokerage account
- **THEN** one lot is created with qty=10 and unit_cost=50.

### Requirement: FIFO Disposal

MUST apply FIFO when a sell transaction is posted on the same account; the disposal record links to the oldest open lot(s).

#### Scenario: Sell 4 of 10

- **WHEN** the user sells 4 shares after buying 10 then 5
- **THEN** 4 shares are matched to the oldest lot; cost basis = 4 × 50 = 200; the realized gain is proceeds - 200.

### Requirement: Holdings Report

MUST expose a holdings report per investment account showing remaining qty, cost basis, and the last manual or external market value (entered by the user — no auto-quote in v1).

#### Scenario: Holdings

- **WHEN** the user opens /reports/holdings
- **THEN** a per-account summary is rendered.

### Requirement: Realized Gains Report

MUST expose a realized-gains report for any period.

#### Scenario: Year report

- **WHEN** the user opens /reports/realized-gains?year=2025
- **THEN** all disposals in 2025 are listed with proceeds, basis, and gain.

### Requirement: Manual Lot Adjustment

MUST allow manual lot creation (e.g. inherited shares) without a buy transaction.

#### Scenario: Manual lot

- **WHEN** the user creates a lot with qty=100, unit_cost=0
- **THEN** the lot is created and appears in holdings.
