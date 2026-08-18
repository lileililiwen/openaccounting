# role-enforcement Specification (delta)

## ADDED Requirements

### Requirement: Owner Can Write

MUST allow the owner of a ledger to perform every write action.

#### Scenario: Owner writes

- **WHEN** the owner creates a transaction, an account, an invoice, a budget
- **THEN** all return 303.

### Requirement: Editor Can Write

MUST allow an editor of a ledger to perform every write action EXCEPT ledger settings (sharing, delete, basis switch).

#### Scenario: Editor writes

- **WHEN** an editor creates a transaction
- **THEN** 303.

#### Scenario: Editor ledger settings

- **WHEN** an editor tries to invite a new member
- **THEN** 403.

### Requirement: Viewer Cannot Write

MUST reject every write action from a viewer.

#### Scenario: Viewer create

- **WHEN** a viewer POSTs a new transaction
- **THEN** 403.

### Requirement: Sharing/Ownership Cannot Be Changed by Non-Owner

MUST reject sharing changes, role changes, and ledger deletion from anyone other than the owner.

#### Scenario: Editor invites

- **WHEN** an editor invites a new member
- **THEN** 403.
