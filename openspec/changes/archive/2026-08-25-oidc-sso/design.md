# Design — oidc-sso

## Context

Auth is axum-login + Argon2 + tower-sessions (Postgres). TOTP exists
as a second factor. OIDC must slot in as an *authentication method*
that produces the same session, not a parallel realm.

## Goals / Non-Goals

**Goals:**
- One provider, standard flow, no custom crypto.
- Linking is safe: verified email + unique subject.

**Non-Goals:**
- Multi-provider or per-ledger IdP selection.
- SCIM deprovisioning (admin console covers disable).
- LDAP (different protocol; OIDC frontends like Glauth cover it).

## Decisions

- **`openidconnect` crate for discovery/JWKS/verification.**
  Alternative considered: hand-rolled JWT verify — rejected (key-type
  and alg-confusion risk); `oauth2` crate alone — rejected (no OIDC
  layer, we'd rewrite claims/nonce handling).
- **PKCE S256 even though confidential client.** WHY: free
  hardening; some providers (Entra) encourage it everywhere.
- **Link by verified email, identity by `(issuer, subject)`.**
  WHY: email is what users understand; subject is what security
  requires. Unverified emails never link — prevents account takeover
  via unverified claim.
- **SSO-only as a runtime flag, not a migration.** WHY: reversible;
  local admin path stays for break-glass via one-time reset link.

## Risks / Trade-offs

- Provider outage locks users out → Mitigation: break-glass admin
  reset link; status check on discovery at login with clear error.
- Email reassignment attack (subject reuse across users) → Mitigation:
  reject linking when `(issuer, subject)` maps to another user; log to
  audit chain.
- Clock skew breaks token validation → Mitigation: `openidconnect`
  leeway config, 60 s.

## Migration Plan

1. Migration `oidc_identities` + provider settings storage.
2. Login/callback routes behind existing session creation helper.
3. Admin console section + policy flags.
4. Login template conditional button; SSO-only toggle last.
