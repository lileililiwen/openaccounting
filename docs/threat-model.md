# Threat model

What OpenAccounting defends against, what it does not, and how logs
and reports avoid leaking secrets. Reviewed alongside `SECURITY.md`.

## Assets

- General-ledger data (balances, counterparties, amounts).
- Credentials: password hashes, TOTP secrets, API tokens, sessions.
- Uploaded documents (invoices, receipts, ID-adjacent paperwork).
- The `APP_SECRET` (session signing, TOTP encryption) and S3 keys.

## Trust boundaries

- Internet → reverse proxy (TLS termination, not in this binary).
- Reverse proxy → app (`APP_HOST`/`APP_PORT`, plain HTTP).
- App → PostgreSQL (`DATABASE_URL`) and document/S3 storage.
- Browser ↔ app: signed cookies, CSRF tokens, bearer API tokens.

## Threats and mitigations

| Threat | Mitigation |
| --- | --- |
| Credential stuffing on `/login` | Per-account + per-IP throttles (429, generic error page); global auth rate-limit middleware with `Retry-After`. |
| Session theft | Signed cookies, `Secure` in production, idle + absolute timeouts. |
| CSRF on state-changing web routes | Per-session CSRF tokens enforced by middleware. |
| Cross-user data access | Ledger-membership checks on every route; `auditor`/`viewer` read-only roles. |
| SQL injection | Parameterised `sqlx` queries everywhere; no string-built SQL except generated `CREATE DATABASE` names (UUID, no user input). |
| Secret exfiltration via logs | Log-redaction policy below; `APP_SECRET`, tokens, and passwords never logged. |
| Malicious uploads | Upload validation (type sniffing, size cap); files served as attachments, never executed. |
| Dependency compromise | `cargo audit` in CI, pinned actions, cosign-signed reproducible releases with SBOM. |

## Out of scope (non-goals)

- Multi-region HA/failover; single-tenant backup/restore only.
- Formal SOC 2/ISO certification; controls are documented, audit is explicit non-audit.
- Endpoint and network security of the operator's host (see
  `docs/production-deployment.md` for the baseline).

## Log-redaction policy

1. NEVER log `APP_SECRET`, API tokens (`oa_live_…`), passwords,
   TOTP secrets, S3 credentials, or session cookie values.
2. Tracing spans record user/ledger IDs, never emails, names, or
   amounts. IDs above the low-cardinality set are hashed before
   export (see `src/observability/`).
3. Error responses carry generic messages; details go to server
   logs with IDs only.
4. Backup tooling redacts `DATABASE_URL` (password) from every
   error string before writing the `backup_runs` error column.
5. Report a suspected secret leak as a vulnerability per
   `SECURITY.md`, not as a public issue.
