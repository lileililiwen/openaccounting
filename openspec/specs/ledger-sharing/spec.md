# ledger-sharing Specification

## Purpose
Enable multiple users to collaborate on the same set of books. Allows business owners to share ledgers with bookkeepers, accountants, and other team members.

## Requirements

### Requirement: Ledger Invitation System

The system MUST support inviting other users to access a ledger.
An invitation MUST include:

- `ledger_id` UUID
- `inviter_id` UUID (the user sending the invitation)
- `invitee_email` TEXT (the email of the user being invited)
- `role` TEXT CHECK (`role` IN ('editor', 'viewer'))
- `status` TEXT CHECK (`status` IN ('pending', 'accepted', 'declined', 'revoked'))
- `created_at` TIMESTAMPTZ
- `expires_at` TIMESTAMPTZ (invitations expire after 7 days)

#### Scenario: Owner invites a bookkeeper

- **WHEN** a ledger owner sends an invitation to
  `bookkeeper@example.com` with role='editor'
- **THEN** an invitation record is created with status='pending'
  and the invitee receives an email notification.

#### Scenario: Invitation expires after 7 days

- **WHEN** an invitation is not accepted within 7 days
- **THEN** the invitation status changes to 'expired' and can
  no longer be accepted.

### Requirement: Ledger Roles

The system MUST support three roles per ledger:

- **owner** — Full access: create, edit, delete, close periods,
  manage sharing. Exactly one owner per ledger (the creator).
- **editor** — Create and edit transactions and accounts. Cannot
  close periods, delete the ledger, or manage sharing.
- **viewer** — Read-only access to all ledger data and reports.
  Cannot create, edit, or delete anything.

#### Scenario: Editor can create transactions

- **WHEN** a user with role='editor' on a ledger creates a
  transaction
- **THEN** the transaction is created successfully.

#### Scenario: Viewer cannot create transactions

- **WHEN** a user with role='viewer' attempts to create a
  transaction
- **THEN** the handler returns `403 Forbidden`.

### Requirement: Invitation Accept/Decline

The system MUST provide a UI for pending invitations. The
invitee MUST be able to:

- Accept the invitation (gains access to the ledger).
- Decline the invitation (no access granted).

After acceptance, the invitee becomes a member of the ledger
with the specified role. The invitation status changes to
'accepted'.

#### Scenario: Invitee accepts invitation

- **WHEN** a user accepts a pending invitation with role='editor'
- **THEN** the invitation status changes to 'accepted', and the
  user can access the ledger with editor permissions.

### Requirement: Member Management

The ledger owner MUST be able to:

- View all members and their roles.
- Change a member's role (e.g., promote viewer to editor).
- Revoke a member's access (removes them from the ledger).

Revoked members lose access immediately. Their existing
transactions are preserved (not deleted).

#### Scenario: Owner revokes access

- **WHEN** a ledger owner revokes a member's access
- **THEN** the member's role is removed, and subsequent attempts
  to access the ledger return `404 Not Found`.

### Requirement: Shared Ledger Navigation

When a user has access to multiple ledgers (their own + shared),
the navigation MUST provide a ledger switcher that lists all
ledgers the user can access. Each ledger MUST show its name and
the user's role.

#### Scenario: User sees shared ledgers in navigation

- **WHEN** a user is an editor on another user's ledger
- **THEN** the ledger switcher shows that ledger with role
  "editor" alongside the user's own ledgers.
