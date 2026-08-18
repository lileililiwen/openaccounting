# notification-preferences Specification

## Purpose
TBD - created by archiving change u6-notification-preferences. Update Purpose after archive.
## Requirements
### Requirement: Channels and Events

MUST expose a grid of (channel, event) toggles; defaults: in-app all ON, email weekly_summary only, push off.

#### Scenario: Defaults

- **WHEN** a fresh user visits /account/notifications
- **THEN** in-app is on for every event; push is off; email is on for weekly_summary only.

### Requirement: Toggles Persist

MUST persist toggles per user.

#### Scenario: Toggle

- **WHEN** the user toggles push for budget_overrun
- **THEN** the preference is saved; a subsequent push fires.

### Requirement: Respect

MUST honor the preference in `notifications::send`.

#### Scenario: Respected

- **WHEN** the user has push off
- **THEN** no push is sent even if a budget overruns.

### Requirement: Audit

MUST log when a notification was suppressed because of preferences.

#### Scenario: Suppressed log

- **WHEN** a push would have fired
- **THEN** a `notifications_suppressed_total{event=budget_overrun}` metric increments.

