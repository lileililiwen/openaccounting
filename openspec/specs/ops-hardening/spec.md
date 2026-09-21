# ops-hardening Specification

## Purpose
TBD - created by archiving change ops-hardening. Update Purpose after archive.
## Requirements
### Requirement: Disclosure Process

SECURITY.md SHALL name an operative security contact and SLA (acknowledge 48h, assess 1 week, fix 30 days critical). Placeholder contacts SHALL fail CI.

#### Scenario: Placeholder blocked

- **WHEN** SECURITY.md contains INSERT or TODO contact markers
- **THEN** the docs-lint test fails.

### Requirement: Tested Backups With RTO RPO

Scheduled backups SHALL support filesystem and S3 targets with retention enforcement, restore verification jobs, and a documented RTO/RPO. SQLite deployments SHALL have an equivalent file-snapshot procedure.

#### Scenario: Restore drill passes

- **WHEN** the scheduled backup plus restore-verification job run against fixtures
- **THEN** the double-entry invariant holds on the restored copy and document count matches.

### Requirement: PITR Guide

Docs SHALL give a WAL-archive PITR procedure with tested commands and a recovery-time table for fixture sizes.

#### Scenario: Point-in-time restore documented

- **WHEN** a user follows the PITR doc on a fixture database
- **THEN** each command resolves to an existing script or binary flag.

### Requirement: Tracing and Rate Limits

The binary SHALL emit OTel traces when enabled with redacted spans and SHALL enforce per-route rate limits returning 429 with Retry-After.

#### Scenario: Rate limit headers present

- **WHEN** a client exceeds the login budget
- **THEN** the response is 429 with a Retry-After header and no account lockout side effect.

### Requirement: SBOM Attestation

Tagged releases SHALL publish an SBOM plus provenance attestation alongside cosign signatures.

#### Scenario: Release artifacts complete

- **WHEN** a tag build finishes
- **THEN** the release contains binary, SBOM, provenance, signatures, and checksums or the job fails.

