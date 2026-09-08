# Design

## Decisions

### Separate development convenience from production safety

Development Compose may retain simple local credentials, but production mode SHALL reject known placeholders, require an explicit environment marker, and require a strong secret. The documentation will make the boundary obvious.

### Make recovery a tested procedure

Backup creation is not sufficient evidence of recoverability. The change will define a restore drill against a disposable PostgreSQL instance and temporary document storage, including checksum/integrity checks and migration compatibility.

### Add maintainership surfaces

`SECURITY.md`, `CONTRIBUTING.md`, `CODEOWNERS`, and a changelog/release policy provide responsible disclosure, review ownership, contribution expectations, and user-visible change tracking.

### Risks and mitigations

- Fail-closed production checks may break existing deployments; mitigate with a migration note listing required variables and an explicit development mode.
- Restore drills can expose sensitive fixtures; mitigate by using synthetic test data and disposable infrastructure.
- Dependency scanners can produce noise; mitigate with a reviewed baseline and time-bounded documented exceptions.

## Verification

Run configuration negative tests, dependency/license scans, CI workflow checks, and a documented backup/restore drill. Confirm production mode cannot start with Compose placeholder credentials.
