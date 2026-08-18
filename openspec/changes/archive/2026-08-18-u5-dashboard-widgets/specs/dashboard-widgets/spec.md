# dashboard-widgets Specification (delta)

## ADDED Requirements

### Requirement: Widget Picker

MUST allow adding, removing, and reordering widgets; the layout is saved per user.

#### Scenario: Add

- **WHEN** the user adds the 'Budget burn' widget
- **THEN** the layout is persisted; the dashboard re-renders with it.

### Requirement: Built-In Widgets

MUST ship these widgets: net-worth chart, top-5 expense categories, budget vs actual, recent transactions, account balances.

#### Scenario: All five present

- **WHEN** a fresh user with no customization
- **THEN** the default layout renders all five widgets.

### Requirement: Persistence

MUST persist the layout per user; reloading the dashboard renders the same widgets in the same order.

#### Scenario: Reload

- **WHEN** the user reloads
- **THEN** the layout is unchanged.

### Requirement: Reset

MUST offer a 'Reset to default' button.

#### Scenario: Reset

- **WHEN** the user clicks reset
- **THEN** the layout returns to default.
