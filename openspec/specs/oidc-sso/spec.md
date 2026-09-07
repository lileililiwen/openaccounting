# oidc-sso Specification

## Purpose
TBD - created by archiving change oidc-sso. Update Purpose after archive.
## Requirements
### Requirement: Provider Configuration

Admins SHALL configure exactly one OIDC provider in the admin console
(issuer/discovery URL, client id, client secret, scopes, claim paths).
Secrets SHALL be stored encrypted with the existing AES-GCM key
infrastructure and never rendered back. When unconfigured, the login
page MUST show no SSO button and behave as today.

#### Scenario: Unconfigured instance unchanged

- **WHEN** no provider is configured
- **THEN** `/login` renders only the password form and `/auth/oidc/*`
  routes return 404.

#### Scenario: Secret not readable

- **WHEN** an admin reopens provider settings
- **THEN** the client secret is masked; only replacement is possible.

### Requirement: Authorization Code + PKCE Login

`GET /auth/oidc/login` SHALL start an Authorization Code flow with
S256 PKCE and `state` bound to the session; `GET /auth/oidc/callback`
SHALL validate state, exchange the code server-side, verify the ID
token signature against discovery JWKS, and enforce `nonce`.
Failures at any step SHALL redirect to `/login?error=oidc` without
creating a session.

#### Scenario: Happy-path login

- **WHEN** a user completes the provider round-trip with a valid ID token
- **THEN** a normal tower-sessions session is created for the linked user.

#### Scenario: State mismatch rejected

- **WHEN** the callback arrives with a `state` that was never issued
- **THEN** no session is created and the user lands on `/login?error=oidc`.

### Requirement: Account Linking and Provisioning

Identity linking SHALL match on the verified (`email_verified=true`)
email claim. Existing users link silently on first login and get an
`oidc_identities` row `(issuer, subject, user_id)`. New emails are
provisioned per admin policy: `auto` creates the user; `invite-only`
rejects with a message directing them to an invitation. A subject
already linked to a different user MUST be rejected.

#### Scenario: Existing user links by email

- **WHEN** paul@example.com logs in via OIDC with an existing local account
- **THEN** the accounts link and subsequent OIDC logins skip the form.

#### Scenario: Invite-only blocks strangers

- **WHEN** policy is `invite-only` and an unknown email authenticates
- **THEN** no user is created and the error page explains invitation policy.

### Requirement: SSO-Only Mode

With `SSO_ONLY=true`, password login, registration, and password
change SHALL be disabled (404/hidden), while TOTP enrollment states
are preserved for later. Local recovery: an admin can still set a
one-time reset link for a locked-out user from the admin console.

#### Scenario: Password form gone

- **WHEN** SSO-only is enabled
- **THEN** `/login` shows only the SSO button and `/register` returns 404.

### Requirement: Group-to-Role Mapping

The admin SHALL be able to map IdP groups to ledger roles (`editor`,
`viewer`) used when provisioning assigns a user to ledgers via share
invitations; direct per-ledger invitations MUST always override group
defaults.

#### Scenario: Group grants viewer default

- **WHEN** a provisioned user's token contains group `accounting-ro`
  mapped to viewer and accepts a ledger invitation
- **THEN** their membership role defaults to viewer unless the invite
  explicitly says editor.

