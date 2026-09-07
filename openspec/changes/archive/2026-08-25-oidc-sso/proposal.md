# OIDC Single Sign-On

## Why

Self-hosted deployments increasingly sit behind an identity provider.
Firefly III supports LDAP and OIDC; Ghostfolio has experimental OIDC;
Kill Bill/Kaui plugs into LDAP/AD. openaccounting has exactly one
factor: local password (+ optional TOTP). Teams running Keycloak,
Authentik, Auth0, or Entra ID must create and deprovision users by
hand — the #1 adoption blocker reported for self-hosted finance tools
in org settings.

## What Changes

- OIDC Authorization Code flow (with PKCE) against a single
  admin-configured provider: discovery URL, client id/secret,
  scopes, claim mapping.
- JIT account linking by verified email; existing users link on first
  login, new users are provisioned per admin policy
  (`auto | invite-only`).
- Local-password login MAY be disabled instance-wide for SSO-only
  deployments (admin toggle).
- Group/role claim → ledger role mapping on assignment.

## Capabilities

### New Capabilities

- `oidc-sso`: OIDC login, account linking, provisioning policy,
  SSO-only mode.

## Impact

**New files:** `src/auth/oidc.rs`, `src/handlers/auth_oidc.rs`,
`migrations/00xx_oidc_identities.sql`.
**Modified:** login page (SSO button), auth handlers (link flow),
admin console (provider config + policy), session creation path.
