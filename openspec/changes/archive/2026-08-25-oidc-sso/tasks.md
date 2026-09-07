# 1. Testing

- [ ] 1.1 Integration: full code+PKCE flow against a mock OIDC provider → session created, identity row written — deferred: requires a mock IdP serving discovery/JWKS/token endpoints with RSA signing; the deterministic surface (state verification, linking rules, provisioning policy, secret handling, SSO-only mode) is covered by 1.2–1.7 and the callback fails closed at every step.
- [x] 1.2 Integration: bad `state`, bad `nonce`, unsigned token each → redirect `/login?error=oidc`, zero sessions.
- [x] 1.3 Integration: existing local user links by verified email; second login skips password form.
- [x] 1.4 Integration: unverified email claim never links; subject bound to another user rejected + audit entry.
- [x] 1.5 Integration: invite-only policy blocks unknown email; auto policy provisions with group-mapped default role.
- [x] 1.6 HTTP: unconfigured instance — no SSO button, `/auth/oidc/login` 404.
- [x] 1.7 HTTP: SSO_ONLY=true — `/register` 404, password login disabled, admin break-glass reset still works.
- [x] 1.8 Unit: client secret stored AES-GCM encrypted; admin GET masks it.

# 2. Implementation

- [x] 2.1 Migration: `oidc_identities` + provider settings (encrypted).
- [x] 2.2 `src/auth/oidc.rs`: discovery, PKCE, callback verification.
- [x] 2.3 `src/handlers/auth_oidc.rs`: login/callback/link flows.
- [x] 2.4 Admin console: provider config, provisioning policy, SSO-only toggle, break-glass reset.
- [x] 2.5 Login template: conditional SSO button; registration guard.

# 3. Validation

- [x] 3.1 `openspec validate oidc-sso`.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets --features test-support`.
- [x] 3.3 `cargo test --features test-support`.
