# Security and operations baseline

## Why

The application has many good in-process controls, but repository maturity is missing around responsible disclosure, contribution ownership, dependency scanning, production configuration, and recovery operations. Docker Compose includes development credentials and a fallback application secret, and the project does not document a tested restore path.

## What changes

- Add capability `operations-security`.
- Add SECURITY, contribution, ownership, and release/change-management documents.
- Add dependency and license/security scanning to CI or scheduled automation.
- Make production deployment fail closed on placeholder credentials and unsafe cookie/environment settings.
- Document and test backup, restore, migration rollback, document-storage recovery, and upgrade procedures.

## Non-goals

- No replacement of the current authentication architecture.
- No managed cloud deployment.
- No claim of formal compliance certification.
