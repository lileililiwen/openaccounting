# multi-entity Specification

## Purpose
Support businesses operating across multiple entities (subsidiaries, divisions, or legal entities) with consolidated reporting.

## Requirements

### Requirement: Entity Entity

The system MUST maintain an `entities` table:

- `id` UUID PRIMARY KEY
- `name` TEXT NOT NULL
- `legal_name` TEXT (nullable, the registered business name)
- `tax_id` TEXT (nullable, EIN or tax identifier)
- `owner_id` UUID NOT NULL (references users)
- `created_at` TIMESTAMPTZ
- `updated_at` TIMESTAMPTZ

Each entity can have multiple ledgers.

#### Scenario: Entity is created

- **WHEN** a user creates an entity "Acme Holdings" with legal
  name "Acme Holdings LLC" and tax ID "12-3456789"
- **THEN** an entity record is stored and can be associated with
  ledgers.

### Requirement: Entity-Ledger Mapping

The ledgers table MUST have an optional `entity_id` field that
links a ledger to an entity. Standalone ledgers (not part of a
multi-entity structure) have `entity_id=NULL`.

#### Scenario: Ledger is assigned to an entity

- **WHEN** a user assigns the "Operating Account" ledger to
  "Acme Holdings"
- **THEN** the ledger's `entity_id` is set to the entity's ID.

### Requirement: Consolidated Balance Sheet

The system MUST provide a consolidated balance sheet that merges
all ledgers under an entity into a single view. The consolidation
MUST:

1. Sum all asset balances across ledgers.
2. Sum all liability balances across ledgers.
3. Sum all equity balances across ledgers (including retained
   earnings).
4. Eliminate inter-entity transactions (transactions where both
   sides are within the same entity structure).

#### Scenario: Consolidated balance sheet is generated

- **WHEN** a user views the consolidated balance sheet for
  "Acme Holdings" which has 2 ledgers
- **THEN** the report shows combined assets, liabilities, and
  equity across both ledgers.

### Requirement: Inter-Entity Elimination

When generating consolidated reports, transactions between
entities within the same structure MUST be eliminated. An
"Inter-Entity Receivable" and "Inter-Entity Payable" pair
must net to zero.

The elimination MUST be recorded as a journal entry tagged with
`kind='elimination'`.

#### Scenario: Inter-entity transaction is eliminated

- **WHEN** Entity A has a $10,000 receivable from Entity B
- **THEN** the consolidated report shows $0 inter-entity balance,
  and an elimination entry is created.

### Requirement: Entity Switcher UI

The navigation MUST provide an entity switcher when the user
has access to multiple entities. Each entity MUST show its name
and the number of ledgers.

#### Scenario: User switches between entities

- **WHEN** a user has access to 2 entities
- **THEN** the entity switcher shows both entities and allows
  switching between them.
