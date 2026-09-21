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

## RTO and RPO

- **RPO (recovery point objective): ≤ 24 hours.** The default
  schedule (`BACKUP_CRON`, daily 02:00) plus WAL archiving (below)
  bounds data loss to the last backup plus archived WAL.
- **RTO (recovery time objective): ≤ 1 hour for databases ≤ 1 GB.**
  The drill in `tests/integration/ops_hardening.rs`
  (`backup_destroy_restore_drill_meets_documented_rto`) restores a
  fixture in seconds; operators must run the drill above on their
  own data and record the actual duration under Evidence Recording.
- A nightly `restore_verify` job restores the latest successful
  backup into a scratch database and checks the double-entry
  invariant plus document counts; results appear as
  `kind = 'verification'` rows on the admin schedule page.

## Scheduled-backup targets

The backup worker (`src/workers/backup.rs`) writes one
`openaccounting_<timestamp>.tar.gz` per run containing the database
payload plus `documents.tar`, then enforces retention.

```bash
# Filesystem target (default): BACKUP_DIR, keep BACKUP_KEEP runs
BACKUP_CRON="0 0 2 * * * *" BACKUP_KEEP=7 BACKUP_DIR=./data/backups

# S3 target: same tarball via the Storage trait (requires
# STORAGE_BACKEND=s3 with bucket/region/credentials set)
BACKUP_TARGET=s3 BACKUP_S3_PREFIX=backups BACKUP_KEEP=14
```

Retention deletes tarballs older than the newest `BACKUP_KEEP` and
mirrors the deletion in the `backups`/`backup_runs` tables, for
both targets.

## SQLite deployments

When `DATABASE_URL` is a `sqlite://` URL, the database payload is
a file snapshot (`db.sqlite` in the tarball) instead of `pg_dump`
output. Restore copies it back over the target path:

```bash
# Manual snapshot restore for SQLite deployments
tar -xzf openaccounting_<timestamp>.tar.gz -C /tmp/oa_sqlite_restore
cp /tmp/oa_sqlite_restore/db.sqlite /var/lib/openaccounting/openaccounting.db
tar -xf /tmp/oa_sqlite_restore/documents.tar -C /var/lib/openaccounting/documents
```

## Point-in-time recovery (PostgreSQL)

Scheduled dumps bound loss to 24 h. For finer granularity, archive
WAL segments and keep base backups.

### 1. Enable WAL archiving

```bash
# postgresql.conf (reload with SELECT pg_reload_conf())
# wal_level = replica
# archive_mode = on
# archive_command = 'cp %p /var/lib/openaccounting/wal/%f'
mkdir -p /var/lib/openaccounting/wal
psql -c "SELECT pg_reload_conf()"
```

### 2. Take a base backup

```bash
pg_basebackup -D /var/lib/openaccounting/base -Ft -z -P
ls /var/lib/openaccounting/base
```

### 3. Recover to a point in time

```bash
# Stop the app, then prepare the recovery target
rm -rf /var/lib/openaccounting/data.recover
tar -xzf /var/lib/openaccounting/base/base.tar.gz -C /var/lib/openaccounting/data.recover
cp /var/lib/openaccounting/data.recover/postgresql.conf /tmp/oa_pg.conf.bak
cat >> /var/lib/openaccounting/data.recover/postgresql.conf <<EOF
restore_command = 'cp /var/lib/openaccounting/wal/%f %p'
recovery_target_time = '2026-09-20 02:00:00+00'
EOF
touch /var/lib/openaccounting/data.recover/recovery.signal
# Start postgres on data.recover; it replays WAL to the target,
# then promotes. Verify with:
psql -c "SELECT pg_is_in_recovery()"
```

### Recovery-time targets

Measure your own rows with the drill above; budget against:

| Fixture size | Base restore | WAL replay (24 h) | Total target |
| --- | --- | --- | --- |
| ≤ 100 MB | < 5 min | < 5 min | < 15 min |
| ≤ 1 GB | < 20 min | < 20 min | < 1 h (RTO) |
| > 1 GB | measure | measure | negotiate RTO |

If a drill exceeds the RTO, shorten the backup interval
(`BACKUP_CRON`), move PostgreSQL to faster disks, or lower the
RPO with more frequent base backups — then re-drill.
