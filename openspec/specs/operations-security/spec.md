# operations-security Specification

## Purpose

Ensures production safety through configuration validation (rejecting placeholder secrets and default credentials), security documentation (SECURITY.md, CONTRIBUTING.md, CODEOWNERS), dependency scanning, and tested backup/restore procedures.
## Requirements
### Requirement: Responsible disclosure and ownership

The repository SHALL publish security-reporting instructions, contribution guidance, code ownership, and a release/change policy.

#### Scenario: Security issue reporter

- **WHEN** a user finds a vulnerability
- **THEN** the repository provides a private reporting path, supported-version expectations, and a response timeline without requiring public disclosure first.

### Requirement: Production configuration fails closed

Production and staging SHALL reject known placeholder database credentials, placeholder application secrets, missing explicit environment selection, and insecure session-cookie configuration.

#### Scenario: Placeholder production secret

- **WHEN** the application starts in production with the Compose fallback secret
- **THEN** startup fails with an actionable configuration error before serving requests.

### Requirement: Dependency risk is continuously visible

The project SHALL run dependency vulnerability and license-policy checks in CI or on a scheduled workflow and SHALL record reviewed exceptions with expiry or owner.

#### Scenario: New vulnerable dependency

- **WHEN** a dependency introduces a known high-severity vulnerability
- **THEN** the configured check fails or creates a visible, owned exception rather than silently passing.

### Requirement: Backup and restore are verified

The project SHALL document and periodically exercise restoration of PostgreSQL data, document files, and application migrations into a disposable environment.

#### Scenario: Restore drill

- **WHEN** a restore drill runs from a generated backup
- **THEN** the restored instance passes migration, audit-chain, document-integrity, and representative report checks.

### Requirement: Production deployment is documented

The project SHALL document secret provisioning, TLS/reverse-proxy expectations, database migrations, health checks, log/metric collection, rollback, and upgrade sequencing.

#### Scenario: Fresh production deployment

- **WHEN** an operator follows the deployment guide
- **THEN** the application starts without development placeholders and exposes the documented health/metrics behavior.

