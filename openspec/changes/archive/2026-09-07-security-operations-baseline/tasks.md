# Tasks

## 1. Testing

- [x] 1.1 Add configuration tests for placeholder secrets, missing `APP_ENV`, insecure cookies, and invalid production database settings.
- [x] 1.2 Add CI dependency/license scan and a synthetic vulnerable-dependency failure test or documented dry run.
- [x] 1.3 Create synthetic backup fixtures and a restore-drill checklist covering database and document storage.
- [x] 1.4 Add a health/metrics smoke check to the deployment verification procedure.

## 2. Implementation

- [x] 2.1 Add `SECURITY.md`, `CONTRIBUTING.md`, `CODEOWNERS`, and a changelog/release policy.
- [x] 2.2 Make production/staging reject known Compose fallback credentials and placeholder secrets.
- [x] 2.3 Add dependency vulnerability and license scanning with an exception ownership format.
- [x] 2.4 Document production configuration, TLS boundary, migrations, logging, metrics, rollback, and upgrades.
- [x] 2.5 Document and automate the backup/restore drill for PostgreSQL and document storage.

## 3. Verification

- [x] 3.1 Run all configuration negative tests.
- [x] 3.2 Run dependency and license scans.
- [x] 3.3 Execute the restore drill against disposable infrastructure and record evidence.
- [x] 3.4 Review the deployment guide from a clean checkout and remove undocumented assumptions.
