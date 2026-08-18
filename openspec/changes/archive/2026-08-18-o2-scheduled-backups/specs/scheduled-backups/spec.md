# scheduled-backups Specification (delta)

## ADDED Requirements

### Requirement: Cron Schedule

MUST accept a cron expression via `BACKUP_CRON` and execute the backup at each tick.

#### Scenario: Hourly tick

- **WHEN** BACKUP_CRON=`0 * * * *` and 1 hour passes
- **THEN** one backup is created.

### Requirement: Content

MUST include both the Postgres `pg_dump` and the documents directory in a single tarball.

#### Scenario: Contents

- **WHEN** an admin inspects the tarball
- **THEN** contains `db.sql` and `documents/`.

### Requirement: Retention

MUST keep at most `BACKUP_KEEP` most-recent backups; older ones are deleted.

#### Scenario: Retention=7

- **WHEN** the 8th backup is taken
- **THEN** the oldest is deleted; 7 remain.

### Requirement: Local Destination

MUST write backups to `BACKUP_DIR` (default `./data/backups`).

#### Scenario: Local

- **WHEN** BACKUP_DIR=/var/backups/oa
- **THEN** files appear there.

### Requirement: S3 Destination (Optional)

MUST MUST, when `BACKUP_S3_BUCKET` is set, additionally upload the tarball to S3 with the same retention.

#### Scenario: S3

- **WHEN** BACKUP_S3_BUCKET=… is set
- **THEN** the tarball is uploaded; old objects are deleted.

### Requirement: Status Page

MUST show the last 20 backup runs (success/failure + duration + size) on `/admin/backups`.

#### Scenario: List

- **WHEN** the admin visits /admin/backups
- **THEN** the runs are listed.

### Requirement: Failure Alert

MUST emit a structured ERROR log on failure; future change may add email/push alerts.

#### Scenario: Failure log

- **WHEN** pg_dump fails
- **THEN** ERROR log with the stderr is emitted.
