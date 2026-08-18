# keyboard-shortcuts Specification

## Purpose
TBD - created by archiving change u1-keyboard-shortcuts. Update Purpose after archive.
## Requirements
### Requirement: Shortcuts Defined

MUST define and document the following shortcuts: `g l` (ledgers), `g t` (transactions), `g a` (accounts), `g r` (reports), `c` (create on list), `?` (help), `Esc` (close overlay).

#### Scenario: Help overlay

- **WHEN** the user presses `?`
- **THEN** the help overlay appears listing all shortcuts.

### Requirement: No Conflict

MUST NOT capture keys when the user is typing in an input or textarea.

#### Scenario: In input

- **WHEN** the user types `g` in an input
- **THEN** the shortcut does not fire.

### Requirement: Disabled on Auth Pages

MUST NOT register shortcuts on `/login` and `/register`.

#### Scenario: Login

- **WHEN** the user is on /login and presses `g l`
- **THEN** nothing happens.

