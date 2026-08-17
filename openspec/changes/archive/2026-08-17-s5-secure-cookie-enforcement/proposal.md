# Enforce Secure Session Cookie in Production

## Why

`SessionManagerLayer` is configured with `with_secure(false)` regardless
of environment (`src/lib.rs:118`). The auth spec (`openspec/specs/auth/spec.md:80`)
notes "production deployments MUST set it to true" but the binary never
enforces this. A misconfigured reverse proxy or a developer who forgets to
flip a flag ships an insecure cookie.

## What Changes

- Add `APP_ENV` config (`production` / `development` / `test`).
- `AppConfig` MUST reject starting with `APP_ENV=production` if `APP_HOST`
  is not on a TLS-terminating interface or if `--allow-insecure-cookies`
  is not set.
- In production, `with_secure(true)` is mandatory.
- Update README to remove the `with_secure(false)` override.

## Capabilities

### New Capabilities

- `secure-cookie-enforcement`: Cookie `Secure` flag tied to environment.

## Impact

**Modified files:**
- `src/config.rs` — `app_env` field.
- `src/lib.rs` — branching on `app_env`.
- `openspec/specs/auth/spec.md` — delta.
- `README.md` — clarify the prod requirement.
