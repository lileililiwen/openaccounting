# printable-views Specification (delta)

## ADDED Requirements

### Requirement: Hidden Chrome

MUST hide navigation, footer, and primary CTAs when printing.

#### Scenario: Print preview

- **WHEN** the user prints a balance sheet
- **THEN** only the report body and a header with date + ledger name are visible.

### Requirement: Table Layout

MUST expand tables to full width and use a serif body font.

#### Scenario: Layout

- **WHEN** the print preview
- **THEN** tables are full width; font is serif.

### Requirement: Page Breaks

MUST avoid breaking a table row across pages; MUST insert a page break before the totals section.

#### Scenario: Break

- **WHEN** a multi-page report
- **THEN** no row is split; totals are on a new page if needed.

### Requirement: Header

MUST include the ledger name, the report name, and the print date in the header.

#### Scenario: Header

- **WHEN** the printed page
- **THEN** the header is visible.

### Requirement: Print Button

MUST show a 'Print' button on every report page.

#### Scenario: Button

- **WHEN** the user visits /reports/balance-sheet
- **THEN** the button is visible and triggers window.print().
