## 1. Testing

- [x] 1.1 HTTP: `http_api_token_required` — 401 without bearer, 401 with wrong token, 200 with valid token.
- [x] 1.2 HTTP: `http_api_create_ledger_and_transaction` — balanced transaction returns 201; subsequent GETs return 200.
- [x] 1.3 HTTP: `http_api_create_transaction_unbalanced_returns_problem_details` — 400 with `application/problem+json` body.
- [x] 1.4 HTTP: `http_api_idempotency_replay_returns_same` — second POST with same `Idempotency-Key` returns the cached 201.
- [ ] 1.5 HTTP: `http_api_rate_limit_429` — deferred. Rate limiter requires a separate `governor` dep + middleware; not in scope for this iteration. The 60 req/min default is documented in the spec; future change will add it.
- [x] 1.6 HTTP: `http_api_reports_trial_balance_200` — 200 with `data` array.
- [x] 1.7 HTTP: `http_api_revoke_token_revokes` — revoked token returns 401.

## 2. Implementation

- [x] 2.1 `migrations/0039_add_api_tokens.sql` — `api_tokens(id, user_id, name, token_prefix UNIQUE, token_hash, created_at, last_used_at, revoked_at)`. (The proposal referenced `0031` but that number was already taken by `0031_add_user_theme.sql`; bumped to `0039`.)
- [x] 2.2 `src/auth/api_token.rs` — `issue_token`, `verify_token`, `revoke_token`, `list_tokens`. Token shape: `oa_live_<48-hex>`; first 12 hex chars stored for lookup, full secret Argon2id-hashed. 3 unit tests.
- [x] 2.3 `src/api/{mod,ledgers,accounts,transactions,reports,problem}.rs`. RFC 7807 problem-details for every 4xx/5xx; `Idempotency-Key` replay on `POST /transactions`. Ledger-scoped ownership enforced via `ledger_owned_by`. Reports: trial-balance, balance-sheet, income-statement, cash-flow (stub), general-ledger.
- [x] 2.4 `src/lib.rs` — `pub mod api` + merge into the protected router via `nest("/api/v1", api_routes_with_bearer)`. Nest scoping keeps the bearer middleware from bleeding into `/metrics` and other unrelated routes.
- [x] 2.5 `src/handlers/account_api_tokens.rs` + `templates/api_tokens/`. Plaintext shown once on create; list/revoke via standard HTML forms.
- [ ] 2.6 Rate limiter (`governor`) — deferred (see 1.5).
- [ ] 2.7 OpenAPI generation — deferred. No `utoipa` dep added; OpenAPI spec can be hand-written or generated in a follow-up change.

## 3. Validation

- [x] 3.1 `openspec validate a1-rest-api` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings.
- [x] 3.4 `cargo test --features test-support --test integration` — 191 / 191 pass (full suite).
- [ ] 3.5 `openspec archive a1-rest-api`.
