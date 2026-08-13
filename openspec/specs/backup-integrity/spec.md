# backup-integrity Specification

## Purpose
Protect against data loss with automated backups and integrity checks to ensure the ledger data is consistent and reliable.

## Requirements

### Requirement: Automated Daily Backups

The system MUST support automated daily database backups via
a background job or cron task. Backups MUST be stored in a
configurable directory (default: `./backups/`).

Each backup file MUST be named with a timestamp:
`openaccounting_YYYYMMDD_HHMMSS.sql.gz`

#### Scenario: Daily backup is created

- **WHEN** the daily backup job runs
- **THEN** a compressed SQL dump of the database is created in
  the backups directory with the current timestamp in the filename.

### Requirement: Backup Retention Policy

The system MUST retain the last 30 daily backups. Backups older
than 30 days MUST be automatically deleted when a new backup
is created.

#### Scenario: Old backups are pruned

- **WHEN** a new backup is created and there are more than 30
  backups in the directory
- **THEN** the oldest backups beyond the 30-day limit are deleted.

### Requirement: Manual Backup Trigger

The system MUST provide an admin-only endpoint to trigger a
manual backup on demand. The response MUST include the backup
filename and path.

#### Scenario: Admin triggers manual backup

- **WHEN** an admin clicks "Backup Now" on the admin page
- **THEN** a backup is created immediately and the admin sees
  the backup filename in the response.

### Requirement: Data Integrity Checks

The system MUST provide an integrity check that verifies:

1. All transactions are balanced (total debits = total credits
   across all postings).
2. All postings reference valid accounts.
3. All accounts reference valid ledgers.
4. No orphaned records exist (postings without transactions,
   transactions without ledgers, etc.).

The integrity check MUST be accessible via an admin endpoint.

#### Scenario: Integrity check passes

- **WHEN** an admin runs the integrity check
- **THEN** the response shows "All checks passed" with counts
  of verified records (transactions, postings, accounts).

#### Scenario: Integrity check finds an issue

- **WHEN** an integrity check finds a transaction with unbalanced
  postings
- **THEN** the response lists the problematic transaction(s) with
  details (ID, date, description, debit/credit totals).

### Requirement: Backup Download

Admins MUST be able to download backup files from the admin
panel. The endpoint MUST serve the file as a download with
the appropriate Content-Type and Content-Disposition headers.

#### Scenario: Admin downloads backup

- **WHEN** an admin clicks download on a backup file
- **THEN** the `.sql.gz` file is downloaded via the browser.
