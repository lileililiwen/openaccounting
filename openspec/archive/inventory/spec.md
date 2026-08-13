# inventory Specification

## Purpose
Track inventory quantities and costs for product-based businesses, with FIFO cost flow and stock valuation reporting.

## Requirements

### Requirement: Inventory Item Entity

The system MUST maintain an `inventory_items` table:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `name` TEXT NOT NULL
- `sku` TEXT (nullable, unique per ledger)
- `description` TEXT
- `asset_account_id` UUID NOT NULL (Inventory Asset account)
- `cogs_account_id` UUID NOT NULL (Cost of Goods Sold account)
- `income_account_id` UUID NOT NULL (Sales Revenue account)
- `quantity_on_hand` INTEGER NOT NULL DEFAULT 0
- `unit_cost` NUMERIC(20,4) NOT NULL DEFAULT 0
- `is_active` BOOLEAN NOT NULL DEFAULT TRUE
- `created_at` TIMESTAMPTZ
- `updated_at` TIMESTAMPTZ

The `quantity_on_hand` and `unit_cost` MUST be automatically
updated when inventory transactions are recorded.

#### Scenario: Inventory item is created

- **WHEN** a user creates an inventory item "Widget" with SKU
  "WDG-001", asset account, COGS account, and income account
- **THEN** the item is stored with `quantity_on_hand=0` and
  `unit_cost=0`.

### Requirement: Inventory Purchase

When a purchase transaction is created with an inventory item,
the system MUST:

1. Increase `quantity_on_hand` by the purchased quantity.
2. Update `unit_cost` using FIFO (first-in, first-out) cost
   flow.
3. Create the standard journal entries (debit Inventory Asset,
   credit Accounts Payable/Cash).

#### Scenario: Inventory is purchased

- **WHEN** a user records a purchase of 100 Widgets at $10 each
- **THEN** `quantity_on_hand` increases by 100, `unit_cost` is
  set to $10.00, and the journal entry is created.

### Requirement: Inventory Sale

When a sale transaction is created with an inventory item, the
system MUST:

1. Decrease `quantity_on_hand` by the sold quantity.
2. Create a Cost of Goods Sold entry using the FIFO cost:
   - Debit COGS (quantity × unit cost)
   - Credit Inventory Asset (quantity × unit cost)
3. Create the standard sale journal entries.

#### Scenario: Inventory is sold

- **WHEN** a user records a sale of 50 Widgets at $25 each
  (unit cost $10)
- **THEN** `quantity_on_hand` decreases by 50, and two journal
  entries are created:
  - Debit Accounts Receivable $1,250, Credit Sales Revenue $1,250
  - Debit COGS $500, Credit Inventory Asset $500

### Requirement: Inventory Valuation Report

The system MUST provide an inventory valuation report showing:

- Item name, SKU
- Quantity on hand
- Unit cost (FIFO)
- Total value (quantity × unit cost)
- Total inventory value (sum across all items)

#### Scenario: User views inventory valuation

- **WHEN** a user navigates to the inventory valuation report
- **THEN** the report shows all active inventory items with
  quantities, unit costs, and total values.

### Requirement: Inventory Adjustment

The system MUST support manual inventory adjustments (for
shrinkage, damage, etc.). An adjustment MUST:

1. Change `quantity_on_hand` by the adjustment quantity.
2. Create a journal entry for the cost difference.
3. Record the adjustment in the audit log.

#### Scenario: Inventory shrinkage is recorded

- **WHEN** a user adjusts Widget quantity from 100 to 95
  (5 units lost)
- **THEN** `quantity_on_hand` decreases by 5, and a journal
  entry is created:
  - Debit COGS $50 (5 × $10 unit cost)
  - Credit Inventory Asset $50
