## 1. Testing

- [x] 1.1 Unit: rate-limit buckets return 429 with Retry-After at budget plus one.
- [x] 1.2 Unit: S3 backup target writes and prunes per retention fixture.
- [x] 1.3 Integration: restore-verification job checks invariant and document counts.
- [x] 1.4 Integration: SECURITY.md lint fails on placeholder markers.
- [x] 1.5 HTTP: /metrics stays Prometheus-valid with trace IDs enabled.
- [x] 1.6 E2E: backup → destroy → restore drill meets documented RTO on fixtures.

## 2. Implementation

- [x] 2.1 SECURITY.md contact, SLA, threat model, log-redaction policy.
- [x] 2.2 Backup worker: S3 target, retention, verification, SQLite parity.
- [x] 2.3 Docs: RTO/RPO statement and PITR procedure with tested commands.
- [x] 2.4 Observability: OTel opt-in tracing with redaction and sampling.
- [x] 2.5 Middleware: per-route rate limits across auth, API, webhooks, imports.
- [x] 2.6 Release: SBOM plus provenance attestation in release workflow.

## 3. Validation

- [x] 3.1 `openspec validate ops-hardening` passes.
- [ ] 3.2 `cargo fmt --check && cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.3 `cargo test --features test-support` passes including backup and lint tests.
