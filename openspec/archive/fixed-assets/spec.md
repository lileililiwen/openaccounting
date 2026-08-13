# fixed-assets Specification

## Purpose
Track fixed assets (equipment, vehicles, property) with depreciation calculations and net book value reporting.

## Requirements

### Requirement: Fixed Asset Entity

The system MUST maintain a `fixed_assets` table:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `name` TEXT NOT NULL
- `description` TEXT
- `account_id` UUID NOT NULL (the asset account, subtype FIXED_ASSET)
- `purchase_date` DATE NOT NULL
- `purchase_cost` NUMERIC(20,4) NOT NULL
- `salvage_value` NUMERIC(20,4) NOT NULL DEFAULT 0
- `useful_life_years` INTEGER NOT NULL
- `depreciation_method` TEXT NOT NULL CHECK (`depreciation_method` IN ('straight_line', 'declining_balance'))
- `status` TEXT NOT NULL CHECK (`status` IN ('active', 'disposed', 'fully_depreciated'))
- `disposed_date` DATE (nullable)
- `disposed_amount` NUMERIC(20,4) (nullable)
- `created_at` TIMESTAMPTZ
- `updated_at` TIMESTAMPTZ

#### Scenario: Fixed asset is registered

- **WHEN** a user registers a vehicle purchased for $30,000 on
  2026-01-15 with 5-year useful life and $5,000 salvage value
- **THEN** a fixed asset record is created with
  `purchase_cost=30000`, `salvage_value=5000`,
  `useful_life_years=5`.

### Requirement: Depreciation Calculation

The system MUST calculate depreciation for each fixed asset.
For straight-line method:

```
annual_depreciation = (purchase_cost - salvage_value) / useful_life_years
monthly_depreciation = annual_depreciation / 12
```

The system MUST provide a "Calculate Depreciation" action that
generates depreciation entries for a given period.

#### Scenario: Depreciation is calculated for a month

- **WHEN** a user calculates depreciation for January 2026
  for a vehicle with $30,000 cost, $5,000 salvage, 5-year life
- **THEN** a depreciation entry is created:
  - Debit Depreciation Expense $416.67
  - Credit Accumulated Depreciation $416.67

### Requirement: Net Book Value

The system MUST track accumulated depreciation for each fixed
asset and calculate net book value:

```
net_book_value = purchase_cost - accumulated_depreciation
```

The fixed asset list MUST display net book value alongside
purchase cost.

#### Scenario: Net book value is displayed

- **WHEN** a user views the fixed asset register
- **THEN** each asset shows purchase cost, accumulated
  depreciation, and net book value.

### Requirement: Asset Disposal

The system MUST support disposing of a fixed asset. When an
asset is disposed:

1. The user enters the disposal date and amount received.
2. A disposal entry is created:
   - Debit Cash/Receivable (disposal amount)
   - Debit Accumulated Depreciation (total to date)
   - Credit Fixed Asset (purchase cost)
   - Debit/Credit Gain/Loss on Disposal (balancing amount)
3. The asset status changes to 'disposed'.

#### Scenario: Asset is disposed at a loss

- **WHEN** a vehicle with $30,000 cost and $20,000 accumulated
  depreciation is sold for $8,000
- **THEN** a disposal entry is created with a $2,000 loss, and
  the asset status changes to 'disposed'.
