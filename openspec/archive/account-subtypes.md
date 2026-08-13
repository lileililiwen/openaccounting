# account-subtypes Specification

## Purpose
Classify accounts into subtypes for proper financial reporting. Enables classified balance sheets (Current vs Non-Current) and multi-step income statements (Gross Profit, Operating Income).

## Requirements

### Requirement: Account Subtype Field

Every account MUST have a `subtype` field that further classifies
its parent `type`. The subtype MUST be one of the predefined values
for its type:

**ASSET subtypes:**
- `CURRENT_ASSET` — Cash, receivables, inventory, prepaid expenses
- `FIXED_ASSET` — Property, equipment, vehicles (non-current)
- `INTANGIBLE_ASSET` — Patents, goodwill, trademarks
- `OTHER_ASSET` — Long-term investments, deposits

**LIABILITY subtypes:**
- `CURRENT_LIABILITY` — Accounts payable, accrued liabilities, short-term debt
- `LONG_TERM_LIABILITY` — Loans, bonds, leases > 12 months

**EQUITY subtypes:**
- `EQUITY` — Owner's capital, contributed surplus
- `RETAINED_EARNINGS` — Accumulated profit/loss from prior years
- `DRAWING` — Owner withdrawals

**INCOME subtypes:**
- `OPERATING_INCOME` — Core revenue from business operations
- `NON_OPERATING_INCOME` — Interest, gains on asset sales, royalties

**EXPENSE subtypes:**
- `COST_OF_GOODS_SOLD` — Direct costs of products/services sold
- `OPERATING_EXPENSE` — Rent, salaries, utilities, marketing
- `NON_OPERATING_EXPENSE` — Interest expense, losses
- `TAX_EXPENSE` — Income tax

The `subtype` column MUST have a CHECK constraint that only allows
subtypes valid for the account's `type`.

#### Scenario: Account is created with a valid subtype

- **WHEN** a user creates an account with `type=ASSET` and
  `subtype=CURRENT_ASSET`
- **THEN** the account is stored with both type and subtype, and
  appears under "Current Assets" on the balance sheet.

#### Scenario: Invalid subtype for type is rejected

- **WHEN** a user creates an account with `type=ASSET` and
  `subtype=OPERATING_EXPENSE`
- **THEN** the database rejects the insert with a CHECK constraint
  violation, and the handler returns `400 Bad Request`.

### Requirement: Classified Balance Sheet

The balance sheet report MUST group accounts by subtype within
each type. The balance sheet MUST display, at minimum:

**Assets section:**
- Current Assets (subtotal)
- Fixed Assets (subtotal)
- Intangible Assets (subtotal, if non-zero)
- Other Assets (subtotal, if non-zero)
- **Total Assets**

**Liabilities section:**
- Current Liabilities (subtotal)
- Long-Term Liabilities (subtotal)
- **Total Liabilities**

**Equity section:**
- Owner's Equity
- Retained Earnings
- **Total Equity**

The accounting equation `Total Assets = Total Liabilities + Total
Equity` MUST be verified and displayed.

#### Scenario: Balance sheet shows classified sections

- **WHEN** a user views the balance sheet for a ledger with
  accounts across multiple subtypes
- **THEN** the report displays assets grouped into Current and
  Fixed sections with subtotals, and liabilities split into
  Current and Long-Term sections.

### Requirement: Multi-Step Income Statement

The income statement MUST display a multi-step format:

1. **Revenue** (Operating Income subtotal)
2. **Cost of Goods Sold** (subtotal)
3. **Gross Profit** (Revenue - COGS)
4. **Operating Expenses** (Operating Expense subtotal)
5. **Operating Income** (Gross Profit - Operating Expenses)
6. **Non-Operating Income/Expense** (net)
7. **Income Before Tax**
8. **Tax Expense**
9. **Net Income**

Non-operating and tax sections MUST only appear if the ledger has
accounts with those subtypes.

#### Scenario: Income statement shows gross profit

- **WHEN** a user views the income statement for a ledger with
  both revenue and COGS accounts
- **THEN** the report displays a Gross Profit line computed as
  Revenue minus Cost of Goods Sold.

### Requirement: Subtype on Default Chart of Accounts

The default chart of accounts seeded on ledger creation MUST
include the `subtype` for each account. Accounts that are
automatically created MUST have appropriate subtypes.

#### Scenario: New ledger has accounts with subtypes

- **WHEN** a user creates a new ledger
- **THEN** all 17+ default accounts are created with correct
  subtypes (e.g., "Cash on Hand" → CURRENT_ASSET, "Bank Account"
  → CURRENT_ASSET, "Accounts Payable" → CURRENT_LIABILITY).

### Requirement: Account List Shows Subtype

The chart of accounts view MUST display the subtype for each
account, either as a column or as a visual grouping indicator.

#### Scenario: User sees subtypes in chart of accounts

- **WHEN** a user views the chart of accounts
- **THEN** each account shows its subtype (e.g., "Current Asset",
  "Operating Expense") in addition to its type.
