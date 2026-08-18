# dark-mode Specification

## Purpose
TBD - created by archiving change u8-dark-mode. Update Purpose after archive.
## Requirements
### Requirement: Default

MUST default to the user's `prefers-color-scheme` (system).

#### Scenario: System dark

- **WHEN** the user's OS is dark
- **THEN** the page renders dark on first visit.

### Requirement: Toggle

MUST offer a toggle that overrides the system preference.

#### Scenario: Toggle

- **WHEN** the user clicks the toggle
- **THEN** the page flips immediately; the preference is saved.

### Requirement: Persistence

MUST persist the user's preference across sessions.

#### Scenario: Persistence

- **WHEN** the user reopens the page next day
- **THEN** the chosen theme is restored.

### Requirement: Coverage

MUST apply to every page.

#### Scenario: Every page

- **WHEN** the user navigates to /reports
- **THEN** the theme is consistent.

