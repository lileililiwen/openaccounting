# secure-cookie-enforcement Specification (delta)

## ADDED Requirements

### Requirement: APP_ENV Config

MUST read `APP_ENV` from the environment; allowed values are `production`, `staging`, `development`, `test`. Defaults to `development`. Refuses to start with an unknown value.

#### Scenario: Unknown value

- **WHEN** APP_ENV=`foo`
- **THEN** startup fails with a clear error.

#### Scenario: Default

- **WHEN** APP_ENV not set
- **THEN** treated as `development`.

### Requirement: Secure Cookie Required in Production

MUST set `Secure` on the session cookie when `APP_ENV ∈ {production, staging}`; MUST refuse to start unless `--allow-insecure-cookies` is passed.

#### Scenario: Production start

- **WHEN** APP_ENV=production, APP_HOST=https://…
- **THEN** session cookie is `Secure`.

#### Scenario: Production on HTTP

- **WHEN** APP_ENV=production, APP_HOST=http://…
- **THEN** startup fails with `Refusing to run in production with an insecure cookie`.

#### Scenario: Override

- **WHEN** APP_ENV=production --allow-insecure-cookies
- **THEN** startup proceeds; a loud warning is logged.

#### Scenario: Development

- **WHEN** APP_ENV=development, http://localhost
- **THEN** session cookie is NOT `Secure` (no behavior change for devs).
