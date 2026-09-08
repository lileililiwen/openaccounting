# Backup and Restore Drill

## Overview

This document defines the procedure for creating, restoring,
and verifying backups of OpenAccounting data. It covers both
the PostgreSQL database and the document storage directory.

## What to Back Up

### 1. PostgreSQL Database

The primary data store. Contains:
- Ledgers, accounts, transactions, postings
- Users, sessions, audit trail
- Tags, document references

### 2. Document Storage

Uploaded files stored in `DOCUMENTS_DIR`. Referenced by
transaction document attachments.

## Backup Procedure

### Database Backup

```bash
# Full dump (custom format for compression + parallel restore)
pg_dump -Fc -v -f backup_$(date +%Y%m%d_%H%M%S).dump \
  openaccounting

# Or plain SQL dump
pg_dump -v -f backup_$(date +%Y%m%d_%H%M%S).sql \
  openaccounting
```

### Document Storage Backup

```bash
# Create a tarball of the documents directory
tar czf docs_backup_$(date +%Y%m%d_%H%M%S).tar.gz \
  -C /var/lib/openaccounting documents
```

### Verify Backups

```bash
# Verify the dump is readable
pg_restore -l backup_*.dump | head -20

# Verify the tarball integrity
tar tzf docs_backup_*.tar.gz | head -20
```

## Restore Drill

This procedure should be run against a disposable PostgreSQL
instance and temporary document storage.

### Prerequisites

- A running PostgreSQL instance (can be Docker)
- Empty target database
- Empty document storage directory

### Step 1: Create Test Database

```bash
createdb openaccounting_restore_test
```

### Step 2: Restore Database

```bash
pg_restore -v -d openaccounting_restore_test \
  backup_YYYYMMDD_HHMMSS.dump
```

### Step 3: Restore Document Storage

```bash
mkdir -p /tmp/oa_restore_test/documents
tar xzf docs_backup_YYYYMMDD_HHMMSS.tar.gz \
  -C /tmp/oa_restore_test
```

### Step 4: Verify Data Integrity

```bash
# Check table counts
psql openaccounting_restore_test -c "
  SELECT
    (SELECT COUNT(*) FROM ledgers) AS ledgers,
    (SELECT COUNT(*) FROM accounts) AS accounts,
    (SELECT COUNT(*) FROM transactions) AS transactions,
    (SELECT COUNT(*) FROM postings) AS postings;
"

# Verify the double-entry invariant
psql openaccounting_restore_test -c "
  SELECT
    SUM(CASE WHEN direction='DEBIT' THEN amount ELSE 0 END) AS total_debit,
    SUM(CASE WHEN direction='CREDIT' THEN amount ELSE 0 END) AS total_credit
  FROM postings;
"
# total_debit must equal total_credit

# Verify document storage files exist
ls /tmp/oa_restore_test/documents/ | head -10
```

### Step 5: Verify Application Compatibility

```bash
# Set DATABASE_URL to the restored database
DATABASE_URL=postgres://localhost/openaccounting_restore_test \
  APP_ENV=test \
  APP_SECRET=test-secret-for-restore-drill-only-not-production \
  cargo run -- healthz
```

### Step 6: Clean Up

```bash
dropdb openaccounting_restore_test
rm -rf /tmp/oa_restore_test
rm backup_*.dump docs_backup_*.tar.gz
```

## Evidence Recording

After each restore drill, record:

- Date and time
- Backup file names and sizes
- Restore duration
- Data integrity check results
- Any discrepancies found
- Sign-off by the person performing the drill

## Schedule

Restore drills should be performed:
- After any migration schema change
- Before major releases
- At least once per quarter
